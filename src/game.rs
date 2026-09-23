use crate::background::Background;
use crate::camera::Camera;
use crate::entities::{Bullet, Entity, Kind, Pickup};
use crate::level::{Color, Level, Spawn, SpawnKind, TILE, Tile};
use crate::physics::{Body, T, solid_at};
use crate::player::{MAX_HP, Player};
use crate::render::{Framebuffer, Rgb, Screen, hex};
use crate::sprites::*;

pub const VIEW_W: usize = 112;
pub const VIEW_H: usize = 64;

/// Ticks a hint stays in the hint row after the hero leaves its globe.
const HINT_TICKS: u32 = 180;
/// A bomb crate hurts a hero closer than this (px, per axis).
const BOMB_RADIUS: f32 = 12.0;
/// Entities further than this outside the view stay frozen.
const ACTIVE_MARGIN: f32 = 24.0;

pub const HUD_BG: Rgb = hex(0x0c0e14);
const HUD_LABEL: Rgb = hex(0x8b93a7);
const HUD_OFF: Rgb = hex(0x3a3f50);
const HUD_ACCENT: Rgb = hex(0xe8a33d);

#[derive(Default, Clone, Copy)]
pub struct Controls {
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub fire: bool,
}

/// Park–Miller LCG: small, seedable, reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// Uniform in [0, 1).
    pub fn next(&mut self) -> f32 {
        self.0 = self.0 * 16807 % 2147483647;
        self.0 as f32 / 2147483647.0
    }
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    c: Rgb,
}

struct Banner {
    text: &'static str,
    ticks: u32,
    keep_score: bool,
    color: Rgb,
}

impl Banner {
    fn died() -> Banner {
        Banner { text: "YOU DIED", ticks: 110, keep_score: false, color: hex(0xff4d4d) }
    }
}

pub struct Game {
    start: Level,
    map: Level,
    bg: Background,
    cam: Camera,
    p: Player,
    entities: Vec<Entity>,
    bullets: Vec<Bullet>,
    parts: Vec<Particle>,
    score: u32,
    banner: Option<Banner>,
    /// Text in the hint row and its remaining ticks.
    hint: Option<(String, u32)>,
    secrets_found: u32,
    secrets_total: u32,
    frame: u32,
    rng: Rng,
}

/// 0..=9, fixed per crate position: decides what the crate holds.
fn crate_roll(tx: i32, ty: i32) -> u32 {
    ((tx as u32).wrapping_mul(73856093) ^ (ty as u32).wrapping_mul(19349663)) % 10
}

fn burst(parts: &mut Vec<Particle>, rng: &mut Rng, x: f32, y: f32, n: usize, spread: f32) {
    for _ in 0..n {
        parts.push(Particle {
            x,
            y,
            vx: (rng.next() - 0.5) * spread,
            vy: -rng.next() * spread * 0.8,
            life: 20.0 + rng.next() * 20.0,
            c: if rng.next() < 0.5 { SPARK_1 } else { SPARK_2 },
        });
    }
}

impl Game {
    pub fn new(level: Level) -> Self {
        let bg = Background::new(level.width_px() as usize, level.height_px() as usize, VIEW_W, VIEW_H);
        let cam = Camera::new(level.width_px(), level.height_px());
        let secrets_total = level.spawns.iter().filter(|s| s.kind == SpawnKind::Secret).count() as u32;
        let mut g = Game {
            map: level.clone(),
            start: level,
            bg,
            cam,
            p: Player::spawn((0, 0)),
            entities: Vec::new(),
            bullets: Vec::new(),
            parts: Vec::new(),
            score: 0,
            banner: None,
            hint: None,
            secrets_found: 0,
            secrets_total,
            frame: 0,
            rng: Rng::new(7),
        };
        g.reset(false);
        g
    }

    fn reset(&mut self, keep_score: bool) {
        self.map = self.start.clone();
        self.p = Player::spawn(self.map.player);
        self.entities = self.map.spawns.iter().map(Entity::spawn).collect();
        self.cam.snap(self.p.body.center(), self.p.face);
        self.bullets.clear();
        self.parts.clear();
        self.banner = None;
        self.hint = None;
        self.secrets_found = 0;
        if !keep_score {
            self.score = 0;
        }
    }

    pub fn update(&mut self, c: Controls) {
        self.frame += 1;
        if let Some(b) = &mut self.banner {
            b.ticks = b.ticks.saturating_sub(1);
            if b.ticks == 0 {
                let keep = b.keep_score;
                self.reset(keep);
            }
            return;
        }

        if let Some(b) = self.p.update(&self.map, c) {
            self.bullets.push(b);
        }
        self.cam.follow(self.p.body.center(), self.p.face);

        // Exit
        let b = self.p.body;
        for ty in (b.y / T).floor() as i32..=((b.y + b.h - 0.01) / T).floor() as i32 {
            for tx in (b.x / T).floor() as i32..=((b.x + b.w - 0.01) / T).floor() as i32 {
                if self.map.get(tx, ty) == Some(Tile::Exit) && self.banner.is_none() {
                    self.score += 1000;
                    self.banner = Some(Banner { text: "LEVEL CLEAR", ticks: 150, keep_score: true, color: EXIT });
                }
            }
        }

        // Touching things: pickups, switches, hint globes, secret areas
        if let Some((_, n)) = &mut self.hint {
            *n = n.saturating_sub(1);
        }
        self.hint = self.hint.take().filter(|(_, n)| *n > 0);
        let mut pressed = Vec::new();
        let (p, score, parts, rng) = (&mut self.p, &mut self.score, &mut self.parts, &mut self.rng);
        let (hint, secrets, hints) = (&mut self.hint, &mut self.secrets_found, &self.map.hints);
        self.entities.retain_mut(|e| {
            if !p.body.overlaps(&e.body) {
                return true;
            }
            let (x, y) = e.body.center();
            match &mut e.kind {
                Kind::Pickup(Pickup::Key(c)) => {
                    p.keys = p.keys.with(*c);
                    *score += 250;
                    burst(parts, rng, x, y, 10, 1.6);
                    false
                }
                Kind::Pickup(Pickup::Health) => {
                    p.hp = (p.hp + 2).min(MAX_HP);
                    false
                }
                Kind::Pickup(Pickup::Gem) => {
                    *score += 100;
                    false
                }
                Kind::Switch { on } => {
                    if !*on {
                        *on = true;
                        pressed.push(((e.body.x / T) as usize, (e.body.y / T) as usize));
                        burst(parts, rng, x, y, 6, 1.4);
                    }
                    true
                }
                Kind::Hint(i) => {
                    match hint {
                        Some((text, n)) if *text == hints[*i] => *n = HINT_TICKS,
                        _ => *hint = Some((hints[*i].clone(), HINT_TICKS)),
                    }
                    true
                }
                Kind::Secret { found } => {
                    if !*found {
                        *found = true;
                        *secrets += 1;
                        *hint = Some(("SECRET AREA FOUND!".into(), HINT_TICKS));
                        burst(parts, rng, x, y, 14, 2.0);
                    }
                    true
                }
                Kind::Bot(_) => true,
            }
        });
        for switch in pressed {
            let gates: Vec<_> = self.map.links.iter().filter(|l| l.switch == switch).map(|l| l.gate).collect();
            for (x, y) in gates {
                self.map.open_gate(x, y);
            }
        }

        // Bots, only near the view
        let (cx, cy) = self.cam.origin();
        let active = Body {
            x: cx as f32 - ACTIVE_MARGIN,
            y: cy as f32 - ACTIVE_MARGIN,
            w: VIEW_W as f32 + 2.0 * ACTIVE_MARGIN,
            h: VIEW_H as f32 + 2.0 * ACTIVE_MARGIN,
        };
        let (map, p) = (&self.map, &mut self.p);
        for e in self.entities.iter_mut().filter(|e| e.body.overlaps(&active)) {
            let Kind::Bot(bot) = &mut e.kind else { continue };
            bot.update(&mut e.body, map, p.keys);
            if p.invulnerable == 0 && p.body.overlaps(&e.body) && p.hurt(e.body.x) {
                self.banner = Some(Banner::died());
            }
        }

        // Bullets
        let mut broken = Vec::new();
        for b in &mut self.bullets {
            b.x += b.vx;
            b.life -= 1;
            if solid_at(&self.map, self.p.keys, b.x, b.y) {
                b.life = 0;
                burst(&mut self.parts, &mut self.rng, b.x, b.y, 3, 1.0);
                broken.push(((b.x / T).floor() as i32, (b.y / T).floor() as i32));
                continue;
            }
            for e in &mut self.entities {
                let eb = e.body;
                let Kind::Bot(bot) = &mut e.kind else { continue };
                if bot.hp > 0 && b.x >= eb.x && b.x <= eb.x + eb.w && b.y >= eb.y - 1.0 && b.y <= eb.y + eb.h {
                    b.life = 0;
                    bot.hp -= 1;
                    bot.hit = 6;
                    if bot.hp <= 0 {
                        self.score += 500;
                        burst(&mut self.parts, &mut self.rng, eb.x + 2.0, eb.y + 2.0, 16, 2.2);
                    }
                    break;
                }
            }
        }
        for (tx, ty) in broken {
            self.break_tile(tx, ty);
        }
        self.bullets.retain(|b| b.life > 0);
        self.entities.retain(|e| !e.is_dead());

        for q in &mut self.parts {
            q.x += q.vx;
            q.y += q.vy;
            q.vy += 0.12;
            q.life -= 1.0;
        }
        self.parts.retain(|q| q.life > 0.0);
    }

    /// A shot hit the solid tile (tx, ty): crates and cracked walls break. A crate drops a gem
    /// (50%), health (30%) or a bomb (20%), fixed per crate position so levels stay repeatable.
    fn break_tile(&mut self, tx: i32, ty: i32) {
        let tile = self.map.get(tx, ty);
        if !matches!(tile, Some(Tile::Crate | Tile::Cracked)) {
            return;
        }
        self.map.set(tx, ty, Tile::Empty);
        let (cx, cy) = ((tx * TILE + 2) as f32, (ty * TILE + 2) as f32);
        burst(&mut self.parts, &mut self.rng, cx, cy, 10, 1.8);
        if tile != Some(Tile::Crate) {
            return;
        }
        let (x, y) = (tx as usize, ty as usize);
        match crate_roll(tx, ty) {
            0..=4 => self.entities.push(Entity::spawn(&Spawn { kind: SpawnKind::Gem, x, y })),
            5..=7 => self.entities.push(Entity::spawn(&Spawn { kind: SpawnKind::Health, x, y })),
            _ => {
                burst(&mut self.parts, &mut self.rng, cx, cy, 24, 3.5);
                let (px, py) = self.p.body.center();
                if (px - cx).abs() < BOMB_RADIUS && (py - cy).abs() < BOMB_RADIUS && self.p.invulnerable == 0 && self.p.hurt(cx) {
                    self.banner = Some(Banner::died());
                }
            }
        }
    }

    fn brick(&self, tx: i32, ty: i32, lx: i32, ly: i32) -> Rgb {
        if ly == 3 || lx == if ty % 2 == 1 { 1 } else { 3 } {
            return MORTAR;
        }
        if ly == 0 && self.map.get(tx, ty - 1).is_some_and(|a| !matches!(a, Tile::Brick | Tile::Cracked)) {
            return BRICK_TOP;
        }
        let h = ((tx as u32).wrapping_mul(73856093) ^ (ty as u32).wrapping_mul(19349663)) % 7;
        if h < 3 { BRICK_A } else if h < 5 { BRICK_B } else { BRICK_C }
    }

    fn tile_pixel(&self, t: Tile, tx: i32, ty: i32, lx: i32, ly: i32) -> Option<Rgb> {
        let edge = lx == 0 || lx == 3;
        match t {
            Tile::Brick => Some(self.brick(tx, ty, lx, ly)),
            Tile::Cracked => Some(if [(1, 0), (2, 1), (1, 2), (2, 3)].contains(&(lx, ly)) { CRACK } else { self.brick(tx, ty, lx, ly) }),
            Tile::Gate => (lx % 2 == 0).then_some(BARS),
            Tile::Crate => Some(if edge || ly == 0 || ly == 3 {
                CRATE_EDGE
            } else if lx == ly || lx + ly == 3 {
                CRATE_X
            } else {
                CRATE_FACE
            }),
            Tile::Girder => match ly {
                0 => Some(GIRDER_1),
                1 => Some(if lx == 2 { GIRDER_HOLE } else { GIRDER_2 }),
                _ => None,
            },
            Tile::Door(c) if self.p.keys.has(c) => edge.then_some(DOOR_FRAME),
            Tile::Door(c) => {
                let (light, dark) = door_colors(c);
                Some(if edge {
                    DOOR_FRAME
                } else if (lx + ly + (self.frame >> 3) as i32) % 4 == 0 {
                    dark
                } else {
                    light
                })
            }
            Tile::Exit => {
                let top = self.map.get(tx, ty - 1) != Some(Tile::Exit);
                let on = (self.frame >> 4).is_multiple_of(2);
                Some(if (edge || (top && ly == 0)) && on { EXIT } else { EXIT_DARK })
            }
            _ => None,
        }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        let (cam_x, cam_y) = self.cam.origin();
        self.bg.draw(fb, (cam_x, cam_y));

        let (tx0, ty0) = (cam_x / TILE, cam_y / TILE);
        let tx1 = ((cam_x + VIEW_W as i32 - 1) / TILE).min(self.map.w as i32 - 1);
        let ty1 = ((cam_y + VIEW_H as i32 - 1) / TILE).min(self.map.h as i32 - 1);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let Some(t) = self.map.get(tx, ty) else { continue };
                let (x, y) = (tx * TILE - cam_x, ty * TILE - cam_y);
                match t {
                    Tile::Empty => {}
                    _ => {
                        for ly in 0..TILE {
                            for lx in 0..TILE {
                                if let Some(c) = self.tile_pixel(t, tx, ty, lx, ly) {
                                    fb.put(x + lx, y + ly, c);
                                }
                            }
                        }
                    }
                }
            }
        }

        let view = Body { x: cam_x as f32 - 8.0, y: cam_y as f32 - 8.0, w: VIEW_W as f32 + 16.0, h: VIEW_H as f32 + 16.0 };
        for e in self.entities.iter().filter(|e| e.body.overlaps(&view)) {
            e.draw(fb, (cam_x, cam_y), self.frame);
        }
        self.p.draw(fb, (cam_x, cam_y), self.frame);

        for b in &self.bullets {
            let (bx, by) = ((b.x - cam_x as f32).round() as i32, (b.y - cam_y as f32).round() as i32);
            fb.put(bx, by, BULLET);
            fb.put(bx - b.vx.signum() as i32, by, FLASH);
        }
        for q in &self.parts {
            fb.put((q.x - cam_x as f32).round() as i32, (q.y - cam_y as f32).round() as i32, q.c);
        }
    }

    /// HUD on the top row, controls + fps on the bottom row, banner in the middle.
    pub fn hud(&self, s: &mut Screen, fps: u32) {
        let (top, bottom) = (0, s.rows - 1);
        s.fill_row(top, HUD_BG);
        s.fill_row(bottom, HUD_BG);

        let mut c = s.text(1, top, "HEALTH ", HUD_LABEL, HUD_BG);
        for i in 0..MAX_HP {
            c = s.text(c, top, "█ ", if i < self.p.hp { HEAL } else { HUD_OFF }, HUD_BG);
        }
        c = s.text(c, top, "  SCORE ", HUD_LABEL, HUD_BG);
        c = s.text(c, top, &format!("{:06}", self.score), HUD_ACCENT, HUD_BG);
        c = s.text(c, top, "   KEYS ", HUD_LABEL, HUD_BG);
        for color in Color::ALL {
            let has = self.p.keys.has(color);
            c = s.text(c, top, if has { "█ " } else { "· " }, if has { key_color(color) } else { HUD_OFF }, HUD_BG);
        }
        if self.secrets_total > 0 {
            c = s.text(c, top, "  SECRETS ", HUD_LABEL, HUD_BG);
            s.text(c, top, &format!("{}/{}", self.secrets_found, self.secrets_total), HUD_ACCENT, HUD_BG);
        }
        let name = format!("{}  ", self.map.name);
        s.text(s.cols - name.chars().count(), top, &name, HUD_LABEL, HUD_BG);

        match &self.hint {
            Some((text, _)) => s.text(1, bottom, text, HUD_ACCENT, HUD_BG),
            None => s.text(0, bottom, " ← → move   Z jump   X fire   Esc quit", HUD_LABEL, HUD_BG),
        };
        let fps = format!("{fps} fps ");
        s.text(s.cols - fps.len(), bottom, &fps, hex(0x4d5468), HUD_BG);

        if let Some(b) = &self.banner {
            let text = format!("  {}  ", b.text);
            let col = (s.cols - text.len()) / 2;
            s.text(col, s.rows / 2 - 1, &text, b.color, HUD_BG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Keys;

    fn game() -> Game {
        Game::new(Level::parse(include_str!("../levels/level1.txt")).unwrap())
    }

    fn is_bot(e: &Entity) -> bool {
        matches!(e.kind, Kind::Bot(_))
    }

    fn bot_count(g: &Game) -> usize {
        g.entities.iter().filter(|e| is_bot(e)).count()
    }

    fn clear_bots(g: &mut Game) {
        g.entities.retain(|e| !is_bot(e));
    }

    fn keep_first_bot(g: &mut Game) {
        let mut seen = false;
        g.entities.retain(|e| !is_bot(e) || !std::mem::replace(&mut seen, true));
    }

    fn first_bot(g: &mut Game) -> &mut Entity {
        g.entities.iter_mut().find(|e| is_bot(e)).unwrap()
    }

    fn bot_hp(e: &Entity) -> Option<i32> {
        match &e.kind {
            Kind::Bot(b) => Some(b.hp),
            _ => None,
        }
    }

    fn run(g: &mut Game, c: Controls, ticks: usize) {
        for _ in 0..ticks {
            g.update(c);
        }
    }

    const YELLOW: Keys = Keys(1 << 3);
    const IDLE: Controls = Controls { left: false, right: false, jump: false, fire: false };

    #[test]
    fn player_stands_still_on_the_floor() {
        let mut g = game();
        run(&mut g, IDLE, 30);
        let y = g.p.body.y;
        assert!(g.p.on_ground);
        for _ in 0..120 {
            g.update(IDLE);
            assert_eq!(g.p.body.y, y);
            assert!(g.p.on_ground);
        }
        assert_eq!(y + g.p.body.h, 14.0 * T, "feet on top of floor row 14");
    }

    #[test]
    fn walls_stop_the_player() {
        let mut g = game();
        run(&mut g, Controls { left: true, ..IDLE }, 120);
        assert_eq!(g.p.body.x, T, "stopped at left wall");
        // Brick block at tiles 20..=23 on rows 12-13 blocks walking right.
        run(&mut g, Controls { right: true, ..IDLE }, 300);
        assert_eq!(g.p.body.x + g.p.body.w, 20.0 * T);
    }

    #[test]
    fn fits_under_low_girder_after_landing_from_a_jump() {
        let mut g = game();
        clear_bots(&mut g);
        run(&mut g, IDLE, 30);
        run(&mut g, Controls { jump: true, ..IDLE }, 5);
        run(&mut g, IDLE, 60);
        assert_eq!(g.p.body.y + g.p.body.h, 14.0 * T, "landed flush on the floor");
        // Girder at tiles 8..=14 on row 11 leaves exactly 2 tiles of headroom.
        run(&mut g, Controls { right: true, ..IDLE }, 200);
        assert_eq!(g.p.body.x + g.p.body.w, 20.0 * T, "walked under the girder up to the brick block");
    }

    #[test]
    fn jump_reaches_three_tiles_and_release_cuts_it() {
        let mut g = game();
        run(&mut g, IDLE, 30);
        let floor = g.p.body.y;
        let mut top = floor;
        for _ in 0..60 {
            g.update(Controls { jump: true, ..IDLE });
            top = top.min(g.p.body.y);
        }
        assert!(floor - top >= 3.0 * T, "full jump {}", floor - top);

        run(&mut g, IDLE, 60);
        let mut short = floor;
        g.update(Controls { jump: true, ..IDLE });
        for _ in 0..60 {
            g.update(IDLE);
            short = short.min(g.p.body.y);
        }
        assert!(floor - short < floor - top, "short hop should be lower");
    }

    #[test]
    fn bot_dies_after_two_hits() {
        let mut g = game();
        run(&mut g, IDLE, 30);
        let (px, py) = (g.p.body.x, g.p.body.y);
        keep_first_bot(&mut g);
        first_bot(&mut g).body = Body { x: px + 24.0, y: py + 3.0, w: 5.0, h: 5.0 };
        let mut saw_one_hit = false;
        for _ in 0..120 {
            g.update(Controls { fire: true, ..IDLE });
            saw_one_hit |= g.entities.iter().any(|e| bot_hp(e) == Some(1));
        }
        assert!(saw_one_hit);
        assert_eq!(bot_count(&g), 0);
        assert_eq!(g.score, 500);
    }

    #[test]
    fn touching_a_bot_hurts_and_dying_restarts() {
        let mut g = game();
        run(&mut g, IDLE, 30);
        keep_first_bot(&mut g);
        let (x, y) = (g.p.body.x, g.p.body.y + 3.0);
        (first_bot(&mut g).body.x, first_bot(&mut g).body.y) = (x, y);
        g.update(IDLE);
        assert_eq!(g.p.hp, MAX_HP - 1);
        assert!(g.p.invulnerable > 0);

        g.p.hp = 1;
        g.p.invulnerable = 0;
        let (x, y) = (g.p.body.x, g.p.body.y + 3.0);
        (first_bot(&mut g).body.x, first_bot(&mut g).body.y) = (x, y);
        g.update(IDLE);
        assert_eq!(g.banner.as_ref().map(|b| b.text), Some("YOU DIED"));
        run(&mut g, IDLE, 200);
        assert!(g.banner.is_none());
        assert_eq!(g.p.hp, MAX_HP);
        assert_eq!(bot_count(&g), 6);
    }

    #[test]
    fn door_needs_key_and_exit_clears_level() {
        let mut g = game();
        let door = (78.0 * T + 1.0, 12.0 * T);
        assert!(solid_at(&g.map, Keys::default(), door.0, door.1));
        assert!(!solid_at(&g.map, YELLOW, door.0, door.1));

        // Put the player right before the door, holding the key, and walk to the exit.
        clear_bots(&mut g);
        g.p.keys = YELLOW;
        g.p.body.x = 76.0 * T;
        g.p.body.y = 12.0 * T;
        let mut cleared = false;
        for _ in 0..200 {
            g.update(Controls { right: true, ..IDLE });
            cleared |= g.banner.as_ref().is_some_and(|b| b.text == "LEVEL CLEAR");
        }
        assert!(cleared);
    }

    #[test]
    fn pickups_are_collected() {
        let mut g = game();
        clear_bots(&mut g);
        g.p.hp = 1;
        // Health '+' sits at tile (11, 10), on the girder at row 11.
        g.p.body.x = 11.0 * T;
        g.p.body.y = 10.0 * T - 4.0;
        run(&mut g, IDLE, 20);
        assert_eq!(g.p.hp, 3);
        assert!(!g.entities.iter().any(|e| matches!(e.kind, Kind::Pickup(Pickup::Health))), "health was picked up");
    }

    #[test]
    fn key_and_gem_pickups_apply_their_effects() {
        let mut g = game();
        clear_bots(&mut g);
        let at = |g: &Game, p: Pickup| g.entities.iter().find(|e| matches!(e.kind, Kind::Pickup(k) if k == p)).unwrap().body;
        let gem = at(&g, Pickup::Gem);
        (g.p.body.x, g.p.body.y) = (gem.x, gem.y);
        g.update(IDLE);
        assert_eq!((g.score, g.p.keys), (100, Keys::default()));

        let key = at(&g, Pickup::Key(Color::Yellow));
        (g.p.body.x, g.p.body.y) = (key.x, key.y);
        g.update(IDLE);
        assert_eq!((g.score, g.p.keys), (350, YELLOW));
        assert!(!g.entities.iter().any(|e| matches!(e.kind, Kind::Pickup(Pickup::Key(_)))));
    }

    fn from(text: &str) -> Game {
        Game::new(Level::parse(text).unwrap())
    }

    const RIGHT: Controls = Controls { right: true, left: false, jump: false, fire: false };
    const FIRE: Controls = Controls { fire: true, left: false, right: false, jump: false };

    #[test]
    fn only_the_matching_key_opens_a_door() {
        let level = "##########\n#...R....#\n#P..R...X#\n##########";
        let mut g = from(level);
        g.p.keys = Keys::default().with(Color::Green);
        run(&mut g, RIGHT, 100);
        assert!(g.p.body.x + g.p.body.w <= 4.0 * T, "a green key must not open the red door");

        let mut g = from(level);
        g.p.keys = Keys::default().with(Color::Red);
        run(&mut g, RIGHT, 100);
        assert!(g.p.body.x > 4.0 * T);
        assert_eq!(g.banner.as_ref().map(|b| b.text), Some("LEVEL CLEAR"));
    }

    #[test]
    fn a_switch_opens_its_gate_once_pressed() {
        let mut g = from("##########\n#...!....#\n#PS.!...X#\n##########\n---\nlink 3,3 5,3");
        assert_eq!(g.map.get(4, 2), Some(Tile::Gate));
        run(&mut g, RIGHT, 20);
        assert_eq!([g.map.get(4, 1), g.map.get(4, 2)], [Some(Tile::Empty), Some(Tile::Empty)], "the whole gate opens");
        assert!(g.entities.iter().any(|e| matches!(e.kind, Kind::Switch { on: true })));
    }

    #[test]
    fn shot_cracked_walls_disappear() {
        let mut g = from("###########\n#.........#\n#....%....#\n#P...#...X#\n###########");
        run(&mut g, FIRE, 40);
        assert_eq!(g.map.get(5, 2), Some(Tile::Empty));
        assert_eq!(g.map.get(5, 3), Some(Tile::Brick), "only cracked walls break");
    }

    #[test]
    fn shot_crates_drop_a_gem_health_or_hurt_with_a_bomb() {
        // The crate at (5, 2) rolls a gem, (4, 2) health and (6, 2) a bomb; the bullet flies at row 2.
        let crate_at = |x: usize, hero: usize| {
            let mut row = vec!['.'; 9];
            row[x - 1] = 'C';
            let mut floor = vec!['.'; 9];
            floor[hero - 1] = 'P';
            format!("###########\n#.........#\n#{}#\n#{}#\n###########", row.iter().collect::<String>(), floor.iter().collect::<String>())
        };
        let pickups = |g: &Game, p: Pickup| g.entities.iter().filter(|e| matches!(e.kind, Kind::Pickup(k) if k == p)).count();

        let mut g = from(&crate_at(5, 1));
        run(&mut g, FIRE, 40);
        assert_eq!((g.map.get(5, 2), pickups(&g, Pickup::Gem)), (Some(Tile::Empty), 1));

        let mut g = from(&crate_at(4, 1));
        run(&mut g, FIRE, 40);
        assert_eq!((g.map.get(4, 2), pickups(&g, Pickup::Health)), (Some(Tile::Empty), 1));

        // The bomb goes off next to the hero.
        let mut g = from(&crate_at(6, 4));
        run(&mut g, FIRE, 40);
        assert_eq!((g.map.get(6, 2), g.p.hp), (Some(Tile::Empty), MAX_HP - 1));
    }

    #[test]
    fn hint_globes_show_their_text_for_a_while() {
        let mut g = from("########\n#......#\n#P.?...#\n########\n---\nhint 4,3 HELLO THERE");
        run(&mut g, RIGHT, 12);
        assert_eq!(g.hint.as_ref().map(|h| h.0.as_str()), Some("HELLO THERE"));
        g.hud(&mut Screen::new(112, 34), 60);
        run(&mut g, RIGHT, 30);
        assert!(g.hint.is_some(), "the text stays for a while after leaving the globe");
        run(&mut g, IDLE, HINT_TICKS as usize + 2);
        assert!(g.hint.is_none());
    }

    #[test]
    fn secret_areas_count_once() {
        let mut g = from("########\n#......#\n#P.s...#\n########");
        assert_eq!((g.secrets_found, g.secrets_total), (0, 1));
        run(&mut g, RIGHT, 60);
        run(&mut g, Controls { left: true, ..IDLE }, 60);
        run(&mut g, RIGHT, 60);
        assert_eq!(g.secrets_found, 1);
        assert_eq!(g.hint.as_ref().map(|h| h.0.as_str()), Some("SECRET AREA FOUND!"));
        g.hud(&mut Screen::new(112, 34), 60);
    }

    #[test]
    fn feature_test_levels_play_through() {
        // Keys: collect all four in order and walk out through every door.
        let mut g = from(include_str!("../levels/test/keys.txt"));
        clear_bots(&mut g);
        for _ in 0..1200 {
            g.update(RIGHT);
            if g.banner.is_some() {
                break;
            }
        }
        assert_eq!(g.p.keys, Keys(0b1111));
        assert_eq!(g.banner.as_ref().map(|b| b.text), Some("LEVEL CLEAR"));
    }

    fn big_game() -> Game {
        Game::new(Level::parse(include_str!("../levels/test/big.txt")).unwrap())
    }

    #[test]
    fn big_level_camera_stays_inside_and_renders_everywhere() {
        let mut g = big_game();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        for ty in (0..90).step_by(3) {
            for tx in (0..128).step_by(3) {
                g.p.body.x = tx as f32 * T;
                g.p.body.y = ty as f32 * T;
                g.cam.snap(g.p.body.center(), 1.0);
                let (cx, cy) = g.cam.origin();
                assert!(cx >= 0 && cy >= 0);
                assert!(cx + VIEW_W as i32 <= g.map.width_px() && cy + VIEW_H as i32 <= g.map.height_px());
                g.render(&mut fb);
            }
        }
    }

    #[test]
    fn camera_moves_smoothly_while_playing_the_big_level() {
        let mut g = big_game();
        clear_bots(&mut g);
        let mut rng = Rng::new(3);
        let (mut prev, mut moved_y) = (g.cam.origin(), false);
        // Jump around, then drop through the gap at the top of the tower.
        for i in 0..2400 {
            let c = Controls { right: i % 900 < 600, left: i % 900 >= 600, jump: rng.next() < 0.3, fire: false };
            g.update(c);
            if i == 1200 {
                g.p.body = Body { x: 8.0 * T, y: 8.0 * T, ..g.p.body };
                g.cam.snap(g.p.body.center(), 1.0);
                prev = g.cam.origin();
            }
            let now = g.cam.origin();
            assert!((now.0 - prev.0).abs() <= 4 && (now.1 - prev.1).abs() <= 4, "tick {i}: camera {prev:?} -> {now:?}");
            moved_y |= now.1 != prev.1;
            prev = now;
        }
        assert!(moved_y, "the camera followed the fall");
    }

    #[test]
    fn far_enemies_stay_frozen_until_the_view_gets_near() {
        let mut g = game();
        let xs = |g: &Game| g.entities.iter().filter(|e| is_bot(e)).map(|e| e.body.x).collect::<Vec<_>>();
        let before = xs(&g);
        run(&mut g, IDLE, 60);
        let after = xs(&g);
        for (b, a) in before.iter().zip(&after) {
            if *b > VIEW_W as f32 + ACTIVE_MARGIN {
                assert_eq!(b, a, "far enemy moved");
            }
        }
        assert!(before.iter().zip(&after).any(|(b, a)| b != a), "near enemy patrols");
    }

    #[test]
    fn renders_without_panicking_across_the_level() {
        let mut g = game();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let mut s = Screen::new(112, 34);
        for x in 0..84 {
            g.p.body.x = x as f32 * T;
            g.cam.snap(g.p.body.center(), 1.0);
            g.render(&mut fb);
            s.blit(&fb, 1, 32, HUD_BG);
            g.hud(&mut s, 60);
            s.flush(&mut Vec::new()).unwrap();
        }
    }
}

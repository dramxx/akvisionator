use crate::background::Background;
use crate::level::{Level, TILE, Tile};
use crate::render::{Framebuffer, Rgb, Screen, hex};
use crate::sprites::{self, *};

pub const VIEW_W: usize = 112;
pub const VIEW_H: usize = 64;

// Physics, per 60 Hz tick, in pixels. Jump ≈ 14 px ≈ 3.5 tiles high.
const GRAVITY: f32 = 0.22;
const MAX_FALL: f32 = 3.5;
const JUMP_SPEED: f32 = -2.5;
const JUMP_CUT: f32 = -1.0;
const RUN_SPEED: f32 = 0.9;
const BOT_SPEED: f32 = 0.35;
const BULLET_SPEED: f32 = 3.2;
const FIRE_COOLDOWN: u32 = 12;
const MAX_HP: i32 = 4;
const BOT_HP: i32 = 2;
const INVULNERABLE: u32 = 70;

const T: f32 = TILE as f32;
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

#[derive(Clone, Copy)]
struct Body {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Body {
    fn overlaps(&self, o: &Body) -> bool {
        self.x < o.x + o.w && self.x + self.w > o.x && self.y < o.y + o.h && self.y + self.h > o.y
    }
}

struct Player {
    body: Body,
    vy: f32,
    knockback: f32,
    face: f32,
    dir: i32,
    on_ground: bool,
    hp: i32,
    invulnerable: u32,
    cooldown: u32,
    flash: u32,
    key: bool,
    jump_held: bool,
    dist: f32,
}

struct Enemy {
    body: Body,
    vy: f32,
    dir: f32,
    hp: i32,
    hit: u32,
}

struct Bullet {
    x: f32,
    y: f32,
    vx: f32,
    life: i32,
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

pub struct Game {
    start: Level,
    map: Level,
    bg: Background,
    p: Player,
    enemies: Vec<Enemy>,
    bullets: Vec<Bullet>,
    parts: Vec<Particle>,
    score: u32,
    banner: Option<Banner>,
    frame: u32,
    rng: Rng,
}

fn solid_at(map: &Level, door_open: bool, x: f32, y: f32) -> bool {
    let (tx, ty) = ((x / T).floor() as i32, (y / T).floor() as i32);
    map.get(tx, ty).is_none_or(|t| t.is_solid(door_open))
}

/// Samples the box every 3 px (tiles are 4 px, so nothing slips through) plus its far edges.
fn box_solid(map: &Level, door_open: bool, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (x1, y1) = (x + w - 0.01, y + h - 0.01);
    let mut yy = y;
    loop {
        let py = yy.min(y1);
        let mut xx = x;
        loop {
            let px = xx.min(x1);
            if solid_at(map, door_open, px, py) {
                return true;
            }
            if px >= x1 {
                break;
            }
            xx += 3.0;
        }
        if py >= y1 {
            return false;
        }
        yy += 3.0;
    }
}

/// Moves along one axis in steps of ≤ 0.25 px. Returns true if it hit something.
fn move_body(map: &Level, door_open: bool, b: &mut Body, horizontal: bool, amount: f32) -> bool {
    let mut rem = amount;
    while rem.abs() > 1e-4 {
        let step = rem.signum() * rem.abs().min(0.25);
        let old = *b;
        if horizontal { b.x += step } else { b.y += step }
        if box_solid(map, door_open, b.x, b.y, b.w, b.h) {
            // Snap flush against the tile we hit, so no sub-pixel gap is left behind
            // (a gap under the feet would make the hero too tall for 2-tile passages).
            let (pos, size) = if horizontal { (&mut b.x, b.w) } else { (&mut b.y, b.h) };
            *pos = if step > 0.0 { ((*pos + size) / T).floor() * T - size } else { ((*pos / T).floor() + 1.0) * T };
            if box_solid(map, door_open, b.x, b.y, b.w, b.h) {
                *b = old;
            }
            return true;
        }
        rem -= step;
    }
    false
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
        let bg = Background::new(level.width_px() as usize, VIEW_W, VIEW_H);
        let mut g = Game {
            map: level.clone(),
            start: level,
            bg,
            p: Player::spawn((0, 0)),
            enemies: Vec::new(),
            bullets: Vec::new(),
            parts: Vec::new(),
            score: 0,
            banner: None,
            frame: 0,
            rng: Rng::new(7),
        };
        g.reset(false);
        g
    }

    fn reset(&mut self, keep_score: bool) {
        self.map = self.start.clone();
        self.p = Player::spawn(self.map.player);
        self.enemies = self
            .map
            .enemies
            .iter()
            .map(|&(tx, ty)| Enemy {
                body: Body { x: tx as f32 * T, y: (ty + 1) as f32 * T - 5.0, w: 5.0, h: 5.0 },
                vy: 0.0,
                dir: if tx % 2 == 1 { 1.0 } else { -1.0 },
                hp: BOT_HP,
                hit: 0,
            })
            .collect();
        self.bullets.clear();
        self.parts.clear();
        self.banner = None;
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

        // Player movement
        let p = &mut self.p;
        let dir = c.right as i32 - c.left as i32;
        p.dir = dir;
        if dir != 0 {
            p.face = dir as f32;
        }
        if c.jump && p.on_ground && !p.jump_held {
            p.vy = JUMP_SPEED;
        }
        if !c.jump && p.vy < JUMP_CUT {
            p.vy = JUMP_CUT;
        }
        p.jump_held = c.jump;
        p.vy = (p.vy + GRAVITY).min(MAX_FALL);
        let vx = dir as f32 * RUN_SPEED + p.knockback;
        p.knockback *= 0.85;
        let x0 = p.body.x;
        move_body(&self.map, p.key, &mut p.body, true, vx);
        p.dist += (p.body.x - x0).abs();
        let hit = move_body(&self.map, p.key, &mut p.body, false, p.vy);
        p.on_ground = hit && p.vy > 0.0;
        if hit {
            p.vy = 0.0;
        }
        p.cooldown = p.cooldown.saturating_sub(1);
        p.flash = p.flash.saturating_sub(1);
        p.invulnerable = p.invulnerable.saturating_sub(1);
        if c.fire && p.cooldown == 0 {
            p.cooldown = FIRE_COOLDOWN;
            p.flash = 3;
            let x = if p.face > 0.0 { p.body.x + 5.0 } else { p.body.x - 3.0 };
            self.bullets.push(Bullet { x, y: p.body.y + 3.0, vx: BULLET_SPEED * p.face, life: 60 });
        }

        // Pickups and exit
        let b = p.body;
        for ty in (b.y / T).floor() as i32..=((b.y + b.h - 0.01) / T).floor() as i32 {
            for tx in (b.x / T).floor() as i32..=((b.x + b.w - 0.01) / T).floor() as i32 {
                match self.map.get(tx, ty) {
                    Some(Tile::Key) => {
                        self.p.key = true;
                        self.score += 250;
                        self.map.set(tx, ty, Tile::Empty);
                        burst(&mut self.parts, &mut self.rng, (tx * TILE + 2) as f32, (ty * TILE + 2) as f32, 10, 1.6);
                    }
                    Some(Tile::Health) => {
                        self.p.hp = (self.p.hp + 2).min(MAX_HP);
                        self.map.set(tx, ty, Tile::Empty);
                    }
                    Some(Tile::Gem) => {
                        self.score += 100;
                        self.map.set(tx, ty, Tile::Empty);
                    }
                    Some(Tile::Exit) if self.banner.is_none() => {
                        self.score += 1000;
                        self.banner = Some(Banner { text: "LEVEL CLEAR", ticks: 150, keep_score: true, color: EXIT });
                    }
                    _ => {}
                }
            }
        }

        // Enemies
        let (map, p) = (&self.map, &mut self.p);
        for e in &mut self.enemies {
            e.vy = (e.vy + GRAVITY).min(MAX_FALL);
            if move_body(map, p.key, &mut e.body, false, e.vy) {
                e.vy = 0.0;
            }
            let b = e.body;
            let nx = if e.dir > 0.0 { b.x + b.w + 0.3 } else { b.x - 0.5 };
            if box_solid(map, p.key, nx, b.y, 0.2, b.h - 0.5) || !solid_at(map, p.key, nx, b.y + b.h + 1.0) {
                e.dir = -e.dir;
            } else {
                e.body.x += e.dir * BOT_SPEED;
            }
            e.hit = e.hit.saturating_sub(1);
            if p.invulnerable == 0 && p.body.overlaps(&e.body) {
                p.hp -= 1;
                p.invulnerable = INVULNERABLE;
                p.knockback = if p.body.x < e.body.x { -2.2 } else { 2.2 };
                p.vy = -1.6;
                if p.hp <= 0 {
                    self.banner = Some(Banner { text: "YOU DIED", ticks: 110, keep_score: false, color: hex(0xff4d4d) });
                }
            }
        }

        // Bullets
        for b in &mut self.bullets {
            b.x += b.vx;
            b.life -= 1;
            if solid_at(&self.map, self.p.key, b.x, b.y) {
                b.life = 0;
                burst(&mut self.parts, &mut self.rng, b.x, b.y, 3, 1.0);
                continue;
            }
            for e in &mut self.enemies {
                let eb = e.body;
                if e.hp > 0 && b.x >= eb.x && b.x <= eb.x + eb.w && b.y >= eb.y - 1.0 && b.y <= eb.y + eb.h {
                    b.life = 0;
                    e.hp -= 1;
                    e.hit = 6;
                    if e.hp <= 0 {
                        self.score += 500;
                        burst(&mut self.parts, &mut self.rng, eb.x + 2.0, eb.y + 2.0, 16, 2.2);
                    }
                    break;
                }
            }
        }
        self.bullets.retain(|b| b.life > 0);
        self.enemies.retain(|e| e.hp > 0);

        for q in &mut self.parts {
            q.x += q.vx;
            q.y += q.vy;
            q.vy += 0.12;
            q.life -= 1.0;
        }
        self.parts.retain(|q| q.life > 0.0);
    }

    fn tile_pixel(&self, t: Tile, tx: i32, ty: i32, lx: i32, ly: i32) -> Option<Rgb> {
        let edge = lx == 0 || lx == 3;
        match t {
            Tile::Brick => {
                if ly == 3 || lx == if ty % 2 == 1 { 1 } else { 3 } {
                    return Some(MORTAR);
                }
                if ly == 0 && self.map.get(tx, ty - 1).is_some_and(|a| a != Tile::Brick) {
                    return Some(BRICK_TOP);
                }
                let h = ((tx as u32).wrapping_mul(73856093) ^ (ty as u32).wrapping_mul(19349663)) % 7;
                Some(if h < 3 { BRICK_A } else if h < 5 { BRICK_B } else { BRICK_C })
            }
            Tile::Girder => match ly {
                0 => Some(GIRDER_1),
                1 => Some(if lx == 2 { GIRDER_HOLE } else { GIRDER_2 }),
                _ => None,
            },
            Tile::Door if self.p.key => edge.then_some(DOOR_FRAME),
            Tile::Door => Some(if edge {
                DOOR_FRAME
            } else if (lx + ly + (self.frame >> 3) as i32) % 4 == 0 {
                DOOR_DARK
            } else {
                DOOR
            }),
            Tile::Exit => {
                let top = self.map.get(tx, ty - 1) != Some(Tile::Exit);
                let on = (self.frame >> 4).is_multiple_of(2);
                Some(if (edge || (top && ly == 0)) && on { EXIT } else { EXIT_DARK })
            }
            _ => None,
        }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        let max_cam = (self.map.width_px() - VIEW_W as i32).max(0);
        let cam = ((self.p.body.x + 2.0 - VIEW_W as f32 / 2.0).round() as i32).clamp(0, max_cam);
        self.bg.draw(fb, cam);

        let tx0 = cam / TILE;
        for ty in 0..self.map.h as i32 {
            for tx in tx0..=(tx0 + VIEW_W as i32 / TILE).min(self.map.w as i32 - 1) {
                let Some(t) = self.map.get(tx, ty) else { continue };
                let (x, y) = (tx * TILE - cam, ty * TILE);
                match t {
                    Tile::Empty => {}
                    Tile::Key => {
                        let bob = (self.frame as f32 / 10.0).sin().round() as i32;
                        sprites::draw(fb, &PICK_KEY, x, y + bob, false, pick_pal);
                    }
                    Tile::Health => sprites::draw(fb, &PICK_HEALTH, x, y + 1, false, pick_pal),
                    Tile::Gem => {
                        sprites::draw(fb, &PICK_GEM, x, y + 1, false, pick_pal);
                        if (self.frame >> 3).is_multiple_of(3) {
                            fb.put(x + 1, y + 2, GEM_HI);
                        }
                    }
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

        for e in &self.enemies {
            let white = e.hit > 0;
            let (x, y) = ((e.body.x - cam as f32).round() as i32, e.body.y.round() as i32);
            sprites::draw(fb, &BOT[((self.frame >> 3) % 2) as usize], x, y, e.dir < 0.0, |ch| {
                if white { WHITE } else { pal(ch) }
            });
        }

        let p = &self.p;
        let blink = p.invulnerable > 0 && (self.frame >> 2) % 2 == 1;
        if !blink {
            let legs = if !p.on_ground {
                &HERO_JUMP
            } else if p.knockback.abs() < 0.1 && p.dir != 0 {
                if (p.dist / 3.0) as i32 % 2 == 1 { &HERO_RUN1 } else { &HERO_RUN2 }
            } else {
                &HERO_STAND
            };
            let flip = p.face < 0.0;
            let sx = if flip { p.body.x - 1.0 } else { p.body.x } - cam as f32;
            let (sx, sy) = (sx.round() as i32, p.body.y.round() as i32);
            sprites::draw(fb, &HERO_TOP, sx, sy, flip, pal);
            sprites::draw(fb, legs, sx, sy + HERO_TOP.len() as i32, flip, pal);
            if p.flash > 0 {
                let fx = if p.face > 0.0 { p.body.x + 5.0 } else { p.body.x - 2.0 } - cam as f32;
                let (fx, fy) = (fx.round() as i32, (p.body.y + 3.0).round() as i32);
                fb.put(fx, fy, FLASH);
                fb.put(fx + p.face as i32, fy, BULLET);
                fb.put(fx, fy - 1, FLASH);
                fb.put(fx, fy + 1, FLASH);
            }
        }

        for b in &self.bullets {
            let (bx, by) = ((b.x - cam as f32).round() as i32, b.y.round() as i32);
            fb.put(bx, by, BULLET);
            fb.put(bx - b.vx.signum() as i32, by, FLASH);
        }
        for q in &self.parts {
            fb.put((q.x - cam as f32).round() as i32, q.y.round() as i32, q.c);
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
        c = s.text(c, top, "   KEY ", HUD_LABEL, HUD_BG);
        if self.p.key {
            s.text(c, top, "█", KEY, HUD_BG);
        } else {
            s.text(c, top, "·", HUD_OFF, HUD_BG);
        }
        let name = format!("{}  ", self.map.name);
        s.text(s.cols - name.chars().count(), top, &name, HUD_LABEL, HUD_BG);

        s.text(0, bottom, " ← → move   Z jump   X fire   Esc quit", HUD_LABEL, HUD_BG);
        let fps = format!("{fps} fps ");
        s.text(s.cols - fps.len(), bottom, &fps, hex(0x4d5468), HUD_BG);

        if let Some(b) = &self.banner {
            let text = format!("  {}  ", b.text);
            let col = (s.cols - text.len()) / 2;
            s.text(col, s.rows / 2 - 1, &text, b.color, HUD_BG);
        }
    }
}

impl Player {
    fn spawn((tx, ty): (usize, usize)) -> Self {
        Player {
            body: Body { x: tx as f32 * T, y: (ty + 1) as f32 * T - 8.0, w: 4.0, h: 8.0 },
            vy: 0.0,
            knockback: 0.0,
            face: 1.0,
            dir: 0,
            on_ground: false,
            hp: MAX_HP,
            invulnerable: 0,
            cooldown: 0,
            flash: 0,
            key: false,
            jump_held: false,
            dist: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(Level::parse("test", include_str!("../levels/level1.txt")).unwrap())
    }

    fn run(g: &mut Game, c: Controls, ticks: usize) {
        for _ in 0..ticks {
            g.update(c);
        }
    }

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
        g.enemies.clear();
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
        g.enemies.truncate(1);
        g.enemies[0].body = Body { x: px + 24.0, y: py + 3.0, w: 5.0, h: 5.0 };
        let mut saw_one_hit = false;
        for _ in 0..120 {
            g.update(Controls { fire: true, ..IDLE });
            saw_one_hit |= g.enemies.first().is_some_and(|e| e.hp == 1);
        }
        assert!(saw_one_hit);
        assert!(g.enemies.is_empty());
        assert_eq!(g.score, 500);
    }

    #[test]
    fn touching_a_bot_hurts_and_dying_restarts() {
        let mut g = game();
        run(&mut g, IDLE, 30);
        g.enemies.truncate(1);
        g.enemies[0].body.x = g.p.body.x;
        g.enemies[0].body.y = g.p.body.y + 3.0;
        g.update(IDLE);
        assert_eq!(g.p.hp, MAX_HP - 1);
        assert!(g.p.invulnerable > 0);

        g.p.hp = 1;
        g.p.invulnerable = 0;
        g.enemies[0].body.x = g.p.body.x;
        g.enemies[0].body.y = g.p.body.y + 3.0;
        g.update(IDLE);
        assert_eq!(g.banner.as_ref().map(|b| b.text), Some("YOU DIED"));
        run(&mut g, IDLE, 200);
        assert!(g.banner.is_none());
        assert_eq!(g.p.hp, MAX_HP);
        assert_eq!(g.enemies.len(), 6);
    }

    #[test]
    fn door_needs_key_and_exit_clears_level() {
        let mut g = game();
        let door = (78.0 * T + 1.0, 12.0 * T);
        assert!(solid_at(&g.map, false, door.0, door.1));
        assert!(!solid_at(&g.map, true, door.0, door.1));

        // Put the player right before the door, holding the key, and walk to the exit.
        g.enemies.clear();
        g.p.key = true;
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
        g.enemies.clear();
        g.p.hp = 1;
        // Health '+' sits at tile (11, 10), on the girder at row 11.
        g.p.body.x = 11.0 * T;
        g.p.body.y = 10.0 * T - 4.0;
        run(&mut g, IDLE, 20);
        assert_eq!(g.p.hp, 3);
        assert_eq!(g.map.get(11, 10), Some(Tile::Empty));
    }

    #[test]
    fn renders_without_panicking_across_the_level() {
        let mut g = game();
        let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
        let mut s = Screen::new(112, 34);
        for x in 0..84 {
            g.p.body.x = x as f32 * T;
            g.render(&mut fb);
            s.blit(&fb, 1, 32, HUD_BG);
            g.hud(&mut s, 60);
            s.flush(&mut Vec::new()).unwrap();
        }
    }
}

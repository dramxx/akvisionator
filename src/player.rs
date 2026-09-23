use crate::entities::{BULLET_SPEED, Bullet};
use crate::game::Controls;
use crate::level::{Keys, Level};
use crate::physics::{Body, GRAVITY, MAX_FALL, T, move_body};
use crate::render::Framebuffer;
use crate::sprites::{self, BULLET, FLASH, HERO_JUMP, HERO_RUN1, HERO_RUN2, HERO_STAND, HERO_TOP, pal};

// Jump ≈ 14 px ≈ 3.5 tiles high.
const JUMP_SPEED: f32 = -2.5;
const JUMP_CUT: f32 = -1.0;
const RUN_SPEED: f32 = 0.9;
const FIRE_COOLDOWN: u32 = 12;
pub const MAX_HP: i32 = 4;
const INVULNERABLE: u32 = 70;

pub struct Player {
    pub body: Body,
    pub vy: f32,
    pub knockback: f32,
    pub face: f32,
    pub dir: i32,
    pub on_ground: bool,
    pub hp: i32,
    pub invulnerable: u32,
    pub cooldown: u32,
    pub flash: u32,
    pub keys: Keys,
    pub jump_held: bool,
    pub dist: f32,
}

impl Player {
    pub fn spawn((tx, ty): (usize, usize)) -> Self {
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
            keys: Keys::default(),
            jump_held: false,
            dist: 0.0,
        }
    }

    /// One point of damage, knocking the hero away from `from_x`. Returns true if it was fatal.
    pub fn hurt(&mut self, from_x: f32) -> bool {
        self.hp -= 1;
        self.invulnerable = INVULNERABLE;
        self.knockback = if self.body.x < from_x { -2.2 } else { 2.2 };
        self.vy = -1.6;
        self.hp <= 0
    }

    /// One tick of movement, timers and firing. Returns the bullet if one was fired.
    pub fn update(&mut self, map: &Level, c: Controls) -> Option<Bullet> {
        let dir = c.right as i32 - c.left as i32;
        self.dir = dir;
        if dir != 0 {
            self.face = dir as f32;
        }
        if c.jump && self.on_ground && !self.jump_held {
            self.vy = JUMP_SPEED;
        }
        if !c.jump && self.vy < JUMP_CUT {
            self.vy = JUMP_CUT;
        }
        self.jump_held = c.jump;
        self.vy = (self.vy + GRAVITY).min(MAX_FALL);
        let vx = dir as f32 * RUN_SPEED + self.knockback;
        self.knockback *= 0.85;
        let x0 = self.body.x;
        move_body(map, self.keys, &mut self.body, true, vx);
        self.dist += (self.body.x - x0).abs();
        let hit = move_body(map, self.keys, &mut self.body, false, self.vy);
        self.on_ground = hit && self.vy > 0.0;
        if hit {
            self.vy = 0.0;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        self.flash = self.flash.saturating_sub(1);
        self.invulnerable = self.invulnerable.saturating_sub(1);
        if c.fire && self.cooldown == 0 {
            self.cooldown = FIRE_COOLDOWN;
            self.flash = 3;
            let x = if self.face > 0.0 { self.body.x + 5.0 } else { self.body.x - 3.0 };
            return Some(Bullet { x, y: self.body.y + 3.0, vx: BULLET_SPEED * self.face, life: 60 });
        }
        None
    }

    pub fn draw(&self, fb: &mut Framebuffer, cam: (i32, i32), frame: u32) {
        let (cam_x, cam_y) = cam;
        let blink = self.invulnerable > 0 && (frame >> 2) % 2 == 1;
        if blink {
            return;
        }
        let legs = if !self.on_ground {
            &HERO_JUMP
        } else if self.knockback.abs() < 0.1 && self.dir != 0 {
            if (self.dist / 3.0) as i32 % 2 == 1 { &HERO_RUN1 } else { &HERO_RUN2 }
        } else {
            &HERO_STAND
        };
        let flip = self.face < 0.0;
        let sx = if flip { self.body.x - 1.0 } else { self.body.x } - cam_x as f32;
        let (sx, sy) = (sx.round() as i32, (self.body.y - cam_y as f32).round() as i32);
        sprites::draw(fb, &HERO_TOP, sx, sy, flip, pal);
        sprites::draw(fb, legs, sx, sy + HERO_TOP.len() as i32, flip, pal);
        if self.flash > 0 {
            let fx = if self.face > 0.0 { self.body.x + 5.0 } else { self.body.x - 2.0 } - cam_x as f32;
            let (fx, fy) = (fx.round() as i32, (self.body.y + 3.0 - cam_y as f32).round() as i32);
            fb.put(fx, fy, FLASH);
            fb.put(fx + self.face as i32, fy, BULLET);
            fb.put(fx, fy - 1, FLASH);
            fb.put(fx, fy + 1, FLASH);
        }
    }
}

use crate::level::{Keys, Level};
use crate::physics::{Body, GRAVITY, MAX_FALL, box_solid, move_body, solid_at};
use crate::render::Framebuffer;
use crate::sprites::{self, BOT, WHITE, pal};

const BOT_SPEED: f32 = 0.35;
pub const BOT_HP: i32 = 2;

/// Walker bot: patrols its platform, turns at walls and ledges.
pub struct Bot {
    pub vy: f32,
    pub dir: f32,
    pub hp: i32,
    pub hit: u32,
}

impl Bot {
    pub fn spawn(tx: usize) -> Self {
        Bot { vy: 0.0, dir: if tx % 2 == 1 { 1.0 } else { -1.0 }, hp: BOT_HP, hit: 0 }
    }

    pub fn update(&mut self, body: &mut Body, map: &Level, keys: Keys) {
        self.vy = (self.vy + GRAVITY).min(MAX_FALL);
        if move_body(map, keys, body, false, self.vy) {
            self.vy = 0.0;
        }
        let b = *body;
        let nx = if self.dir > 0.0 { b.x + b.w + 0.3 } else { b.x - 0.5 };
        if box_solid(map, keys, nx, b.y, 0.2, b.h - 0.5) || !solid_at(map, keys, nx, b.y + b.h + 1.0) {
            self.dir = -self.dir;
        } else {
            body.x += self.dir * BOT_SPEED;
        }
        self.hit = self.hit.saturating_sub(1);
    }

    pub fn draw(&self, body: &Body, fb: &mut Framebuffer, cam: (i32, i32), frame: u32) {
        let white = self.hit > 0;
        let (x, y) = ((body.x - cam.0 as f32).round() as i32, (body.y - cam.1 as f32).round() as i32);
        // `dir` is always ±1. Written as `self.dir < 0.0` this crashes rustc 1.94's optimizer (opt-level 2/3).
        let flip = self.dir.is_sign_negative();
        sprites::draw(fb, &BOT[((frame >> 3) % 2) as usize], x, y, flip, |ch| if white { WHITE } else { pal(ch) });
    }
}

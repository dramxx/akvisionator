use crate::enemies::Bot;
use crate::level::{Color, Spawn, SpawnKind, TILE};
use crate::physics::{Body, T};
use crate::render::Framebuffer;
use crate::sprites::{
    self, GEM_HI, HINT_GLOBE, PICK_GEM, PICK_HEALTH, PICK_KEY, SWITCH_OFF, SWITCH_ON, key_color, pick_pal, switch_pal,
};

pub const BULLET_SPEED: f32 = 3.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pickup {
    Key(Color),
    Health,
    Gem,
}

/// What an entity is, plus the small state that kind needs.
pub enum Kind {
    Bot(Bot),
    Pickup(Pickup),
    /// Pressed once by touching it; then its linked gates stay open.
    Switch { on: bool },
    /// Index into `Level::hints`; touching it shows that text.
    Hint(usize),
    /// Invisible trigger of a secret area.
    Secret { found: bool },
}

/// Anything in the level that moves or can be picked up or destroyed.
/// Tiles stay a static grid; new mechanics add a `Kind`.
pub struct Entity {
    pub body: Body,
    pub kind: Kind,
}

impl Entity {
    pub fn spawn(s: &Spawn) -> Self {
        let (tx, ty) = (s.x as f32 * T, s.y as f32 * T);
        let tile = Body { x: tx, y: ty, w: TILE as f32, h: TILE as f32 };
        match s.kind {
            // Bots stand on the floor of their spawn tile; the sprite is 5 x 5 px.
            SpawnKind::Bot => Entity { body: Body { x: tx, y: ty + T - 5.0, w: 5.0, h: 5.0 }, kind: Kind::Bot(Bot::spawn(s.x)) },
            SpawnKind::Key(c) => Entity { body: tile, kind: Kind::Pickup(Pickup::Key(c)) },
            SpawnKind::Health => Entity { body: tile, kind: Kind::Pickup(Pickup::Health) },
            SpawnKind::Gem => Entity { body: tile, kind: Kind::Pickup(Pickup::Gem) },
            SpawnKind::Switch => Entity { body: tile, kind: Kind::Switch { on: false } },
            SpawnKind::Hint(i) => Entity { body: tile, kind: Kind::Hint(i) },
            SpawnKind::Secret => Entity { body: tile, kind: Kind::Secret { found: false } },
        }
    }

    pub fn is_dead(&self) -> bool {
        matches!(&self.kind, Kind::Bot(b) if b.hp <= 0)
    }

    pub fn draw(&self, fb: &mut Framebuffer, cam: (i32, i32), frame: u32) {
        let (x, y) = (self.body.x.round() as i32 - cam.0, self.body.y.round() as i32 - cam.1);
        match &self.kind {
            Kind::Bot(b) => b.draw(&self.body, fb, cam, frame),
            Kind::Pickup(Pickup::Key(c)) => {
                let bob = (frame as f32 / 10.0).sin().round() as i32;
                sprites::draw(fb, &PICK_KEY, x, y + bob, false, |_| key_color(*c));
            }
            Kind::Pickup(Pickup::Health) => sprites::draw(fb, &PICK_HEALTH, x, y + 1, false, pick_pal),
            Kind::Pickup(Pickup::Gem) => {
                sprites::draw(fb, &PICK_GEM, x, y + 1, false, pick_pal);
                if (frame >> 3).is_multiple_of(3) {
                    fb.put(x + 1, y + 2, GEM_HI);
                }
            }
            Kind::Switch { on } => sprites::draw(fb, if *on { &SWITCH_ON } else { &SWITCH_OFF }, x, y, false, switch_pal),
            Kind::Hint(_) => {
                let bob = (frame as f32 / 14.0).sin().round() as i32;
                sprites::draw(fb, &HINT_GLOBE, x, y + bob, false, pick_pal);
            }
            Kind::Secret { .. } => {}
        }
    }
}

pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub life: i32,
}

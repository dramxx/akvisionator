use crate::render::{Framebuffer, Rgb, hex};

// Hero: 5 px wide, top half + one set of legs = 8 px tall.
pub const HERO_TOP: [&str; 5] = [".yy..", ".ys..", ".ss..", "rrrgg", "rrr.."];
pub const HERO_STAND: [&str; 3] = [".bb..", ".b.b.", ".k.k."];
pub const HERO_RUN1: [&str; 3] = [".bb..", "b..b.", "k...k"];
pub const HERO_RUN2: [&str; 3] = [".bb..", ".bb..", ".kk.."];
pub const HERO_JUMP: [&str; 3] = [".bbb.", "b....", "k...."];

pub const BOT: [[&str; 5]; 2] = [
    [".mmm.", "mdedm", "mmmmm", ".d.d.", "d...d"],
    [".mmm.", "mdedm", "mmmmm", ".d.d.", ".d.d."],
];

pub const PICK_KEY: [&str; 3] = ["yy..", "y.yy", "yy.y"];
pub const PICK_HEALTH: [&str; 3] = ["wrw", "rrr", "wrw"];
pub const PICK_GEM: [&str; 3] = [".c.", "ccc", ".c."];

/// Palette for hero and bot sprites.
pub fn pal(ch: u8) -> Rgb {
    match ch {
        b'y' => hex(0xf2c14e),
        b's' => hex(0xe9b08a),
        b'r' => hex(0xc8323c),
        b'b' => hex(0x2f5fb3),
        b'k' => hex(0x23232e),
        b'g' => hex(0xa7b0ba),
        b'm' => hex(0x7c8a99),
        b'd' => hex(0x4a5563),
        b'e' => hex(0xff4d4d),
        _ => WHITE,
    }
}

/// Palette for pickups.
pub fn pick_pal(ch: u8) -> Rgb {
    match ch {
        b'y' => KEY,
        b'r' => HEAL,
        b'c' => GEM,
        _ => WHITE,
    }
}

pub const WHITE: Rgb = hex(0xffffff);
pub const BRICK_A: Rgb = hex(0x8a4130);
pub const BRICK_B: Rgb = hex(0x7a3527);
pub const BRICK_C: Rgb = hex(0x933f2b);
pub const MORTAR: Rgb = hex(0x3d1c16);
pub const BRICK_TOP: Rgb = hex(0xc0673f);
pub const GIRDER_1: Rgb = hex(0xe0a33a);
pub const GIRDER_2: Rgb = hex(0xb07a22);
pub const GIRDER_HOLE: Rgb = hex(0x4a3310);
pub const DOOR: Rgb = hex(0x58b4e0);
pub const DOOR_DARK: Rgb = hex(0x1d4d66);
pub const DOOR_FRAME: Rgb = hex(0x2b3140);
pub const EXIT: Rgb = hex(0x3ddc84);
pub const EXIT_DARK: Rgb = hex(0x0f3d25);
pub const KEY: Rgb = hex(0xffd23f);
pub const HEAL: Rgb = hex(0xe23b3b);
pub const GEM: Rgb = hex(0x6fe3ff);
pub const GEM_HI: Rgb = hex(0xe6fbff);
pub const BULLET: Rgb = hex(0xfff2a8);
pub const FLASH: Rgb = hex(0xffd66b);
pub const SPARK_1: Rgb = hex(0xffb347);
pub const SPARK_2: Rgb = hex(0xff6a3d);
pub const STAR: Rgb = hex(0xcfd3ff);
pub const STAR_DIM: Rgb = hex(0x6b6f9e);

/// Draws a text sprite; '.' is transparent. `flip` mirrors it horizontally.
pub fn draw(fb: &mut Framebuffer, rows: &[&str], x: i32, y: i32, flip: bool, color: impl Fn(u8) -> Rgb) {
    for (r, row) in rows.iter().enumerate() {
        let w = row.len() as i32;
        for (c, ch) in row.bytes().enumerate() {
            if ch == b'.' {
                continue;
            }
            let c = c as i32;
            fb.put(if flip { x + w - 1 - c } else { x + c }, y + r as i32, color(ch));
        }
    }
}

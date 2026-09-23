use crate::level::Color;
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
/// Wall lever: `r`/`l` is the knob (off / on), `g` the metal.
pub const SWITCH_OFF: [&str; 4] = ["r...", ".g..", ".g..", "gggg"];
pub const SWITCH_ON: [&str; 4] = ["...l", "..g.", "..g.", "gggg"];
pub const HINT_GLOBE: [&str; 4] = [".cc.", "cwcc", "cccc", ".cc."];
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
        b'r' => HEAL,
        b'c' => GEM,
        _ => WHITE,
    }
}

/// Palette for switches.
pub fn switch_pal(ch: u8) -> Rgb {
    match ch {
        b'r' => hex(0xe64a4a),
        b'l' => EXIT,
        _ => hex(0x8b93a7),
    }
}

/// Key sprite color, also the door's pane color.
pub fn key_color(c: Color) -> Rgb {
    match c {
        Color::Red => hex(0xe64a4a),
        Color::Green => hex(0x4ad66d),
        Color::Blue => hex(0x4a9be6),
        Color::Yellow => KEY,
    }
}

/// Door pane colors: (light, dark).
pub fn door_colors(c: Color) -> (Rgb, Rgb) {
    match c {
        Color::Red => (hex(0xd44a4a), hex(0x5a1d1d)),
        Color::Green => (hex(0x3fbf62), hex(0x14472a)),
        Color::Blue => (hex(0x58b4e0), hex(0x1d4d66)),
        Color::Yellow => (hex(0xe8b830), hex(0x6b5210)),
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
pub const CRACK: Rgb = hex(0x1a0c09);
pub const BARS: Rgb = hex(0x8b93a7);
pub const CRATE_EDGE: Rgb = hex(0x6b4419);
pub const CRATE_FACE: Rgb = hex(0xc98b3f);
pub const CRATE_X: Rgb = hex(0x9a6528);
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

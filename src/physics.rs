use crate::level::{Keys, Level, TILE};

// Per 60 Hz tick, in pixels.
pub const GRAVITY: f32 = 0.22;
pub const MAX_FALL: f32 = 3.5;
pub const T: f32 = TILE as f32;

#[derive(Clone, Copy)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Body {
    pub fn overlaps(&self, o: &Body) -> bool {
        self.x < o.x + o.w && self.x + self.w > o.x && self.y < o.y + o.h && self.y + self.h > o.y
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

pub fn solid_at(map: &Level, keys: Keys, x: f32, y: f32) -> bool {
    let (tx, ty) = ((x / T).floor() as i32, (y / T).floor() as i32);
    map.get(tx, ty).is_none_or(|t| t.is_solid(keys))
}

/// Samples the box every 3 px (tiles are 4 px, so nothing slips through) plus its far edges.
pub fn box_solid(map: &Level, keys: Keys, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (x1, y1) = (x + w - 0.01, y + h - 0.01);
    let mut yy = y;
    loop {
        let py = yy.min(y1);
        let mut xx = x;
        loop {
            let px = xx.min(x1);
            if solid_at(map, keys, px, py) {
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
pub fn move_body(map: &Level, keys: Keys, b: &mut Body, horizontal: bool, amount: f32) -> bool {
    let mut rem = amount;
    while rem.abs() > 1e-4 {
        let step = rem.signum() * rem.abs().min(0.25);
        let old = *b;
        if horizontal { b.x += step } else { b.y += step }
        if box_solid(map, keys, b.x, b.y, b.w, b.h) {
            // Snap flush against the tile we hit, so no sub-pixel gap is left behind
            // (a gap under the feet would make the hero too tall for 2-tile passages).
            let (pos, size) = if horizontal { (&mut b.x, b.w) } else { (&mut b.y, b.h) };
            *pos = if step > 0.0 { ((*pos + size) / T).floor() * T - size } else { ((*pos / T).floor() + 1.0) * T };
            if box_solid(map, keys, b.x, b.y, b.w, b.h) {
                *b = old;
            }
            return true;
        }
        rem -= step;
    }
    false
}

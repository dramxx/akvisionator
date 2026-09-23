use crate::game::Rng;
use crate::render::{Framebuffer, Rgb, hex};
use crate::sprites::{STAR, STAR_DIM};

struct Layer {
    w: usize,
    px: Vec<Option<Rgb>>,
    parallax: f32,
}

/// Sky gradient + stars + two skyline layers, precomputed once per level width.
pub struct Background {
    sky: Vec<Rgb>,
    layers: Vec<Layer>,
}

impl Background {
    pub fn new(level_w: usize, view_w: usize, view_h: usize) -> Self {
        let stops = [(0.0, hex(0x090b1f)), (0.55, hex(0x261a44)), (1.0, hex(0x5a2a4e))];
        let sky = (0..view_h)
            .map(|y| {
                let t = y as f32 / (view_h - 1) as f32;
                let i = if t <= stops[1].0 { 0 } else { 1 };
                let (a, b) = (stops[i].1, stops[i + 1].1);
                let u = (t - stops[i].0) / (stops[i + 1].0 - stops[i].0);
                let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * u).round() as u8;
                Rgb(lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
            })
            .collect();

        let mut rng = Rng::new(11);
        let width = |par: f32| (view_w as f32 + level_w.saturating_sub(view_w) as f32 * par).ceil() as usize + 2;

        let skyline = |rng: &mut Rng, par, h_min, h_max, body, win, win_chance: f32| {
            let w = width(par);
            let mut px = vec![None; w * view_h];
            let mut x = 0;
            while x < w {
                let bw = 5 + (rng.next() * 10.0) as usize;
                let h = h_min + (rng.next() * (h_max - h_min) as f32) as usize;
                for i in 0..bw.min(w - x) {
                    for ly in 0..h {
                        let lit = i % 3 == 1 && ly % 3 == 2 && i < bw - 1 && rng.next() < win_chance;
                        px[(view_h - h + ly) * w + x + i] = Some(if lit { win } else { body });
                    }
                }
                x += bw + if rng.next() < 0.3 { 1 + (rng.next() * 3.0) as usize } else { 0 };
            }
            Layer { w, px, parallax: par }
        };
        let near = skyline(&mut rng, 0.6, 12, 26, hex(0x140f26), hex(0xf2c14e), 0.12);
        let far = skyline(&mut rng, 0.35, 16, 40, hex(0x221a3d), hex(0x8a6a3a), 0.3);

        let w = width(0.15);
        let mut px = vec![None; w * view_h];
        for _ in 0..w.div_ceil(2) {
            let x = (rng.next() * w as f32) as usize;
            let y = (rng.next() * 40.0) as usize;
            px[y * w + x] = Some(if rng.next() < 0.3 { STAR } else { STAR_DIM });
        }
        let stars = Layer { w, px, parallax: 0.15 };

        Self { sky, layers: vec![near, far, stars] }
    }

    pub fn draw(&self, fb: &mut Framebuffer, cam: i32) {
        for y in 0..fb.h {
            for x in 0..fb.w {
                let c = self
                    .layers
                    .iter()
                    .find_map(|l| l.px[y * l.w + x + (cam as f32 * l.parallax).round() as usize])
                    .unwrap_or(self.sky[y]);
                fb.put(x as i32, y as i32, c);
            }
        }
    }
}

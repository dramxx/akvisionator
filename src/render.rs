use std::io::{self, Write};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const fn hex(n: u32) -> Rgb {
    Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)
}

/// RGB pixel buffer. Two pixel rows map onto one terminal row.
pub struct Framebuffer {
    pub w: usize,
    pub h: usize,
    px: Vec<Rgb>,
}

impl Framebuffer {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![Rgb::default(); w * h] }
    }

    /// Clipped write, for sprites that may be partly off screen.
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.px[y * self.w + x]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    ch: char,
    fg: Rgb,
    bg: Rgb,
}

/// Grid of terminal cells. `flush` writes only the cells that changed since the last flush.
pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    full: bool,
    out: Vec<u8>,
}

impl Screen {
    pub fn new(cols: usize, rows: usize) -> Self {
        let blank = Cell { ch: ' ', fg: Rgb::default(), bg: Rgb::default() };
        Self { cols, rows, cells: vec![blank; cols * rows], prev: vec![blank; cols * rows], full: true, out: Vec::new() }
    }

    pub fn fill_row(&mut self, row: usize, bg: Rgb) {
        for c in &mut self.cells[row * self.cols..(row + 1) * self.cols] {
            *c = Cell { ch: ' ', fg: bg, bg };
        }
    }

    /// Draws the framebuffer as `▀` cells (fg = top pixel, bg = bottom pixel) into `rows` rows
    /// starting at `row0`, scaled uniformly (nearest neighbour) to fit and centered on `border`.
    pub fn blit(&mut self, fb: &Framebuffer, row0: usize, rows: usize, border: Rgb) {
        let (w, h) = (self.cols, rows * 2);
        let scale = (w as f32 / fb.w as f32).min(h as f32 / fb.h as f32);
        let (iw, ih) = ((fb.w as f32 * scale) as usize, (fb.h as f32 * scale) as usize);
        let (ox, oy) = ((w - iw) / 2, (h - ih) / 2);
        // Source pixel for each target column / pixel row, None = border.
        let map = |i: usize, off: usize, len: usize, src: usize| {
            (i >= off && i < off + len).then(|| (i - off) * src / len)
        };
        let xs: Vec<_> = (0..w).map(|x| map(x, ox, iw, fb.w)).collect();
        let ys: Vec<_> = (0..h).map(|y| map(y, oy, ih, fb.h)).collect();
        let px = |x: Option<usize>, y: Option<usize>| x.zip(y).map_or(border, |(x, y)| fb.get(x, y));
        for r in 0..rows {
            for (x, &sx) in xs.iter().enumerate() {
                self.cells[(row0 + r) * self.cols + x] = Cell { ch: '▀', fg: px(sx, ys[r * 2]), bg: px(sx, ys[r * 2 + 1]) };
            }
        }
    }

    /// Writes text starting at `col`, returns the column after it.
    pub fn text(&mut self, col: usize, row: usize, s: &str, fg: Rgb, bg: Rgb) -> usize {
        let mut c = col;
        for ch in s.chars() {
            if c < self.cols {
                self.cells[row * self.cols + c] = Cell { ch, fg, bg };
            }
            c += 1;
        }
        c
    }

    pub fn flush(&mut self, w: &mut impl Write) -> io::Result<()> {
        self.out.clear();
        let mut cursor = None;
        let (mut fg, mut bg) = (None, None);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let i = r * self.cols + c;
                let cell = self.cells[i];
                if !self.full && cell == self.prev[i] {
                    continue;
                }
                if cursor != Some((r, c)) {
                    write!(self.out, "\x1b[{};{}H", r + 1, c + 1)?;
                }
                if fg != Some(cell.fg) {
                    let Rgb(r, g, b) = cell.fg;
                    write!(self.out, "\x1b[38;2;{r};{g};{b}m")?;
                    fg = Some(cell.fg);
                }
                if bg != Some(cell.bg) {
                    let Rgb(r, g, b) = cell.bg;
                    write!(self.out, "\x1b[48;2;{r};{g};{b}m")?;
                    bg = Some(cell.bg);
                }
                let mut utf8 = [0; 4];
                self.out.extend_from_slice(cell.ch.encode_utf8(&mut utf8).as_bytes());
                cursor = Some((r, c + 1));
            }
        }
        self.prev.copy_from_slice(&self.cells);
        self.full = false;
        if !self.out.is_empty() {
            w.write_all(&self.out)?;
            w.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_block_uses_top_as_fg_and_bottom_as_bg() {
        let mut fb = Framebuffer::new(2, 2);
        fb.put(0, 0, Rgb(1, 2, 3));
        fb.put(0, 1, Rgb(4, 5, 6));
        let mut s = Screen::new(2, 1);
        s.blit(&fb, 0, s.rows, Rgb::default());
        let mut out = Vec::new();
        s.flush(&mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("\x1b[1;1H\x1b[38;2;1;2;3m\x1b[48;2;4;5;6m▀"), "{out:?}");
    }

    #[test]
    fn unchanged_frame_writes_nothing_and_one_change_writes_one_cell() {
        let mut fb = Framebuffer::new(4, 4);
        let mut s = Screen::new(4, 2);
        s.blit(&fb, 0, s.rows, Rgb::default());
        s.flush(&mut Vec::new()).unwrap();

        let mut out = Vec::new();
        s.blit(&fb, 0, s.rows, Rgb::default());
        s.flush(&mut out).unwrap();
        assert!(out.is_empty());

        fb.put(3, 3, Rgb(9, 9, 9));
        s.blit(&fb, 0, s.rows, Rgb::default());
        s.flush(&mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(out.matches('▀').count(), 1);
        assert!(out.starts_with("\x1b[2;4H"));
    }

    #[test]
    fn scales_uniformly_and_centers_with_border() {
        // 2x2 image into 6 cols x 2 rows (6x4 px): scale 2, one border column each side.
        let mut fb = Framebuffer::new(2, 2);
        fb.put(1, 1, Rgb(9, 9, 9));
        let border = Rgb(1, 1, 1);
        let mut s = Screen::new(6, 2);
        s.blit(&fb, 0, 2, border);
        let at = |c: usize, r: usize| s.cells[r * s.cols + c];
        assert_eq!((at(0, 0).fg, at(5, 1).bg), (border, border));
        assert_eq!((at(1, 0).fg, at(2, 0).bg), (Rgb::default(), Rgb::default()));
        assert_eq!((at(3, 1).fg, at(4, 1).bg), (Rgb(9, 9, 9), Rgb(9, 9, 9)));
        assert_eq!(at(2, 1).fg, Rgb::default());
    }
}

pub const TILE: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Empty,
    Brick,
    Girder,
    Door,
    Exit,
    Key,
    Health,
    Gem,
}

impl Tile {
    pub fn is_solid(self, door_open: bool) -> bool {
        match self {
            Tile::Brick | Tile::Girder => true,
            Tile::Door => !door_open,
            _ => false,
        }
    }
}

/// A level as the game sees it, regardless of where it came from (text file, later a generator).
#[derive(Clone)]
pub struct Level {
    pub name: String,
    pub w: usize,
    pub h: usize,
    tiles: Vec<Tile>,
    /// Spawn positions in tile coordinates.
    pub player: (usize, usize),
    pub enemies: Vec<(usize, usize)>,
}

impl Level {
    pub fn parse(name: &str, text: &str) -> Result<Level, String> {
        let rows: Vec<&str> = text.lines().map(|l| l.trim_end()).filter(|l| !l.is_empty()).collect();
        let h = rows.len();
        let w = rows.first().map_or(0, |r| r.chars().count());
        if h == 0 || w == 0 {
            return Err("level is empty".into());
        }
        let mut tiles = Vec::with_capacity(w * h);
        let mut player = None;
        let mut enemies = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            if row.chars().count() != w {
                return Err(format!("row {} has {} tiles, expected {w}", y + 1, row.chars().count()));
            }
            for (x, ch) in row.chars().enumerate() {
                tiles.push(match ch {
                    '.' => Tile::Empty,
                    '#' => Tile::Brick,
                    '=' => Tile::Girder,
                    'D' => Tile::Door,
                    'X' => Tile::Exit,
                    'k' => Tile::Key,
                    '+' => Tile::Health,
                    '*' => Tile::Gem,
                    'P' => {
                        player = Some((x, y));
                        Tile::Empty
                    }
                    'E' => {
                        enemies.push((x, y));
                        Tile::Empty
                    }
                    _ => return Err(format!("unknown tile '{ch}' at {x},{y}")),
                });
            }
        }
        let player = player.ok_or("level has no player start 'P'")?;
        Ok(Level { name: name.into(), w, h, tiles, player, enemies })
    }

    pub fn get(&self, tx: i32, ty: i32) -> Option<Tile> {
        if tx < 0 || ty < 0 || tx as usize >= self.w || ty as usize >= self.h {
            return None;
        }
        Some(self.tiles[ty as usize * self.w + tx as usize])
    }

    pub fn set(&mut self, tx: i32, ty: i32, t: Tile) {
        if self.get(tx, ty).is_some() {
            self.tiles[ty as usize * self.w + tx as usize] = t;
        }
    }

    pub fn width_px(&self) -> i32 {
        self.w as i32 * TILE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level1_parses() {
        let l = Level::parse("L1", include_str!("../levels/level1.txt")).unwrap();
        assert_eq!((l.w, l.h), (84, 16));
        assert_eq!(l.player, (2, 13));
        assert_eq!(l.enemies.len(), 6);
        assert_eq!(l.get(59, 4), Some(Tile::Key));
        assert_eq!(l.get(78, 10), Some(Tile::Door));
        assert_eq!(l.get(-1, 0), None);
    }

    #[test]
    fn bad_levels_are_rejected() {
        assert!(Level::parse("x", "#P#\n##").is_err());
        assert!(Level::parse("x", "#?#").is_err());
        assert!(Level::parse("x", "###").is_err());
    }
}

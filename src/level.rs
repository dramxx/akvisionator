pub const TILE: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Red,
    Green,
    Blue,
    Yellow,
}

impl Color {
    pub const ALL: [Color; 4] = [Color::Red, Color::Green, Color::Blue, Color::Yellow];

    pub fn name(self) -> &'static str {
        ["red", "green", "blue", "yellow"][self as usize]
    }
}

/// The key colors the hero holds.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Keys(pub u8);

impl Keys {
    pub fn has(self, c: Color) -> bool {
        self.0 & (1 << c as u8) != 0
    }

    pub fn with(self, c: Color) -> Keys {
        Keys(self.0 | (1 << c as u8))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Empty,
    Brick,
    Girder,
    /// Solid until the hero holds the key of its color.
    Door(Color),
    Exit,
    /// Solid until its switch is pressed.
    Gate,
    /// Solid; a shot breaks it and it drops an item.
    Crate,
    /// Looks like cracked brick; a shot breaks it (secret passages).
    Cracked,
}

impl Tile {
    pub fn is_solid(self, keys: Keys) -> bool {
        match self {
            Tile::Brick | Tile::Girder | Tile::Gate | Tile::Crate | Tile::Cracked => true,
            Tile::Door(c) => !keys.has(c),
            Tile::Empty | Tile::Exit => false,
        }
    }
}

/// What a spawn marker in the tile grid turns into at runtime.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpawnKind {
    Bot,
    Key(Color),
    Health,
    Gem,
    Switch,
    /// Index into `Level::hints`.
    Hint(usize),
    /// Invisible; touching it counts a secret area as found.
    Secret,
}

/// An entity marker in tile coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spawn {
    pub kind: SpawnKind,
    pub x: usize,
    pub y: usize,
}

/// Pressing the switch at `switch` opens the group of gate tiles that contains `gate`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link {
    pub switch: (usize, usize),
    pub gate: (usize, usize),
}

/// 1-based position, like the loader's errors and the level file's metadata.
pub fn at(x: usize, y: usize) -> String {
    format!("column {}, row {}", x + 1, y + 1)
}

/// A level as the game sees it, regardless of where it came from (text file, later a generator).
/// Tiles are the static grid; everything that moves or can be picked up is a spawn.
#[derive(Clone)]
pub struct Level {
    pub name: String,
    pub w: usize,
    pub h: usize,
    tiles: Vec<Tile>,
    /// Player start in tile coordinates.
    pub player: (usize, usize),
    pub spawns: Vec<Spawn>,
    pub hints: Vec<String>,
    pub links: Vec<Link>,
}

/// `column,row` (1-based) inside a `w` x `h` level.
fn coord(n: usize, s: &str, w: usize, h: usize) -> Result<(usize, usize), String> {
    let parsed = s.split_once(',').and_then(|(x, y)| Some((x.parse::<usize>().ok()?, y.parse::<usize>().ok()?)));
    match parsed {
        None => Err(format!("line {n}: expected 'column,row' but found '{s}'")),
        Some((x, y)) if x == 0 || y == 0 || x > w || y > h => Err(format!("line {n}: {s} is outside the level ({w} x {h})")),
        Some((x, y)) => Ok((x - 1, y - 1)),
    }
}

impl Level {
    /// Level file v2: the tile grid, then optionally a `---` line followed by metadata lines:
    /// `name <text>`, `hint <column>,<row> <text>`, `link <switch column>,<row> <gate column>,<row>`.
    /// Errors say which line (and column) is wrong.
    pub fn parse(text: &str) -> Result<Level, String> {
        let mut rows = Vec::new();
        let mut meta = Vec::new();
        let mut in_meta = false;
        for (i, line) in text.lines().enumerate() {
            let (n, line) = (i + 1, line.trim_end());
            if line == "---" {
                in_meta = true;
            } else if line.is_empty() {
                continue;
            } else if in_meta {
                meta.push((n, line));
            } else {
                rows.push((n, line));
            }
        }

        let h = rows.len();
        let w = rows.first().map_or(0, |(_, r)| r.chars().count());
        if h == 0 || w == 0 {
            return Err("level is empty".into());
        }
        let mut tiles = Vec::with_capacity(w * h);
        let mut player = None;
        let mut spawns = Vec::new();
        let mut hint_count = 0;
        for (y, &(n, row)) in rows.iter().enumerate() {
            if row.chars().count() != w {
                return Err(format!("line {n}: row has {} tiles, expected {w}", row.chars().count()));
            }
            for (x, ch) in row.chars().enumerate() {
                let (tile, kind) = match ch {
                    '.' => (Tile::Empty, None),
                    '#' => (Tile::Brick, None),
                    '=' => (Tile::Girder, None),
                    'X' => (Tile::Exit, None),
                    '!' => (Tile::Gate, None),
                    'C' => (Tile::Crate, None),
                    '%' => (Tile::Cracked, None),
                    'R' => (Tile::Door(Color::Red), None),
                    'G' => (Tile::Door(Color::Green), None),
                    'B' => (Tile::Door(Color::Blue), None),
                    'Y' => (Tile::Door(Color::Yellow), None),
                    'r' => (Tile::Empty, Some(SpawnKind::Key(Color::Red))),
                    'g' => (Tile::Empty, Some(SpawnKind::Key(Color::Green))),
                    'b' => (Tile::Empty, Some(SpawnKind::Key(Color::Blue))),
                    'y' => (Tile::Empty, Some(SpawnKind::Key(Color::Yellow))),
                    '+' => (Tile::Empty, Some(SpawnKind::Health)),
                    '*' => (Tile::Empty, Some(SpawnKind::Gem)),
                    'E' => (Tile::Empty, Some(SpawnKind::Bot)),
                    'S' => (Tile::Empty, Some(SpawnKind::Switch)),
                    's' => (Tile::Empty, Some(SpawnKind::Secret)),
                    '?' => {
                        hint_count += 1;
                        (Tile::Empty, Some(SpawnKind::Hint(hint_count - 1)))
                    }
                    'P' if player.is_some() => return Err(format!("line {n}, column {}: second player start", x + 1)),
                    'P' => {
                        player = Some((x, y));
                        (Tile::Empty, None)
                    }
                    _ => return Err(format!("line {n}, column {}: unknown tile '{ch}'", x + 1)),
                };
                tiles.push(tile);
                spawns.extend(kind.map(|kind| Spawn { kind, x, y }));
            }
        }
        let player = player.ok_or("level has no player start 'P'")?;

        let mut name = String::from("UNNAMED");
        let mut hints = vec![String::new(); hint_count];
        let mut links = Vec::new();
        for (n, line) in meta {
            let (key, value) = line.split_once(' ').map_or((line, ""), |(k, v)| (k, v.trim()));
            match key {
                "name" if value.is_empty() => return Err(format!("line {n}: 'name' needs a value")),
                "name" => name = value.into(),
                "hint" => {
                    let (pos, text) = value.split_once(' ').unwrap_or((value, ""));
                    let (x, y) = coord(n, pos, w, h)?;
                    if text.trim().is_empty() {
                        return Err(format!("line {n}: 'hint' needs a text"));
                    }
                    let globe = spawns.iter().find_map(|s| match s.kind {
                        SpawnKind::Hint(i) if (s.x, s.y) == (x, y) => Some(i),
                        _ => None,
                    });
                    hints[globe.ok_or(format!("line {n}: no hint globe '?' at {}", at(x, y)))?] = text.trim().into();
                }
                "link" => {
                    let (from, to) = value.split_once(' ').unwrap_or((value, ""));
                    let (switch, gate) = (coord(n, from, w, h)?, coord(n, to.trim(), w, h)?);
                    if !spawns.iter().any(|s| s.kind == SpawnKind::Switch && (s.x, s.y) == switch) {
                        return Err(format!("line {n}: no switch 'S' at {}", at(switch.0, switch.1)));
                    }
                    if tiles[gate.1 * w + gate.0] != Tile::Gate {
                        return Err(format!("line {n}: no gate '!' at {}", at(gate.0, gate.1)));
                    }
                    links.push(Link { switch, gate });
                }
                _ => return Err(format!("line {n}: unknown directive '{key}'")),
            }
        }
        for s in &spawns {
            match s.kind {
                SpawnKind::Hint(i) if hints[i].is_empty() => return Err(format!("the hint globe at {} has no text", at(s.x, s.y))),
                SpawnKind::Switch if !links.iter().any(|l| l.switch == (s.x, s.y)) => {
                    return Err(format!("the switch at {} has no link", at(s.x, s.y)));
                }
                _ => {}
            }
        }
        Ok(Level { name, w, h, tiles, player, spawns, hints, links })
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

    /// Opens the whole connected group of gate tiles that contains (x, y).
    pub fn open_gate(&mut self, x: usize, y: usize) {
        let mut todo = vec![(x as i32, y as i32)];
        while let Some((x, y)) = todo.pop() {
            if self.get(x, y) == Some(Tile::Gate) {
                self.set(x, y, Tile::Empty);
                todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
            }
        }
    }

    pub fn width_px(&self) -> i32 {
        self.w as i32 * TILE
    }

    pub fn height_px(&self) -> i32 {
        self.h as i32 * TILE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(l: &Level, kind: SpawnKind) -> usize {
        l.spawns.iter().filter(|s| s.kind == kind).count()
    }

    #[test]
    fn level1_parses() {
        let l = Level::parse(include_str!("../levels/level1.txt")).unwrap();
        assert_eq!(l.name, "LEVEL 1-1  ROOFTOPS");
        assert_eq!((l.w, l.h), (84, 16));
        assert_eq!(l.player, (2, 13));
        assert_eq!(count(&l, SpawnKind::Bot), 6);
        assert_eq!(count(&l, SpawnKind::Gem), 6);
        assert!(l.spawns.contains(&Spawn { kind: SpawnKind::Key(Color::Yellow), x: 59, y: 4 }));
        assert_eq!(l.get(59, 4), Some(Tile::Empty), "pickups are entities, not tiles");
        assert_eq!(l.get(78, 10), Some(Tile::Door(Color::Yellow)));
        assert_eq!(l.get(-1, 0), None);
    }

    #[test]
    fn big_test_level_parses() {
        let l = Level::parse(include_str!("../levels/test/big.txt")).unwrap();
        assert_eq!((l.w, l.h), (128, 90));
        assert_eq!(l.player, (2, 87));
        assert_eq!((l.width_px(), l.height_px()), (512, 360));
    }

    #[test]
    fn metadata_is_optional_and_name_is_read() {
        assert_eq!(Level::parse("#P#\n###").unwrap().name, "UNNAMED");
        assert_eq!(Level::parse("#P#\n###\n---\nname  My Level \n").unwrap().name, "My Level");
    }

    #[test]
    fn keys_doors_and_pickups_parse() {
        let l = Level::parse("#PrgbyRGBY!C%#\n##############").unwrap();
        for (i, c) in Color::ALL.into_iter().enumerate() {
            assert!(l.spawns.contains(&Spawn { kind: SpawnKind::Key(c), x: 2 + i, y: 0 }));
            assert_eq!(l.get(6 + i as i32, 0), Some(Tile::Door(c)));
        }
        assert_eq!([l.get(10, 0), l.get(11, 0), l.get(12, 0)], [Some(Tile::Gate), Some(Tile::Crate), Some(Tile::Cracked)]);
    }

    #[test]
    fn hints_and_links_are_attached() {
        let l = Level::parse("#P?S.!!#\n########\n---\nhint 3,1 Hello there\nlink 4,1 6,1").unwrap();
        assert_eq!(l.hints, ["Hello there"]);
        assert_eq!(l.links, [Link { switch: (3, 0), gate: (5, 0) }]);
        let mut open = l.clone();
        open.open_gate(5, 0);
        assert_eq!([open.get(5, 0), open.get(6, 0)], [Some(Tile::Empty), Some(Tile::Empty)], "the whole gate group opens");
    }

    #[test]
    fn errors_name_line_and_column() {
        let err = |t: &str| Level::parse(t).err().unwrap();
        assert_eq!(err("#P#
##"), "line 2: row has 2 tiles, expected 3");
        assert_eq!(err("
#P#
#@#"), "line 3, column 2: unknown tile '@'");
        assert_eq!(err("#P#
#P#"), "line 2, column 2: second player start");
        assert_eq!(err("###"), "level has no player start 'P'");
        assert_eq!(err(""), "level is empty");
        assert_eq!(err("#P#
---
name"), "line 3: 'name' needs a value");
        assert_eq!(err("#P#
---

music loud"), "line 4: unknown directive 'music'");
    }

    #[test]
    fn hint_and_link_errors_are_specific() {
        let err = |t: &str| Level::parse(t).err().unwrap();
        assert_eq!(err("#P?#"), "the hint globe at column 3, row 1 has no text");
        assert_eq!(err("#P?#
---
hint 3,1"), "line 3: 'hint' needs a text");
        assert_eq!(err("#P?#
---
hint 2,1 hi"), "line 3: no hint globe '?' at column 2, row 1");
        assert_eq!(err("#P?#
---
hint 9,1 hi"), "line 3: 9,1 is outside the level (4 x 1)");
        assert_eq!(err("#P?#
---
hint x hi"), "line 3: expected 'column,row' but found 'x'");
        assert_eq!(err("#PS!#"), "the switch at column 3, row 1 has no link");
        assert_eq!(err("#PS!#
---
link 2,1 4,1"), "line 3: no switch 'S' at column 2, row 1");
        assert_eq!(err("#PS!#
---
link 3,1 2,1"), "line 3: no gate '!' at column 2, row 1");
    }
}

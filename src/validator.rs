//! Level validator: can the hero get from the start to every key, door, switch and the exit?
//!
//! Breadth-first search over standing positions (tile column, floor row) plus what the hero has
//! done so far (keys held, switches pressed). Each step is a short control script run through the
//! real `Player::update`, so jump height and reach come from the actual physics, not a copy of
//! them. Standing positions are tile-aligned, which keeps about a tile of margin on the longest
//! leaps: a level that passes needs no pixel-perfect jumps.
//!
//! Crates and cracked walls are solid here, so a required area must not depend on breaking them.
//! A second search with them removed tells "needs breaking" apart from "cannot be reached at all".

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use crate::game::Controls;
use crate::level::{Color, Keys, Level, Spawn, SpawnKind, Tile, at};
use crate::physics::{Body, T, solid_at};
use crate::player::Player;

const MAX_TICKS: u32 = 400;
const FOREVER: u32 = u32::MAX;
const MAX_SWITCHES: usize = 32;

/// Hold `dir` for `dir_ticks` ticks and jump for `jump_hold` ticks (0 = walk), until standing
/// again after at least `min_ticks`.
#[derive(Clone, Copy)]
struct Script {
    jump_hold: u32,
    dir: i32,
    dir_ticks: u32,
    min_ticks: u32,
}

fn scripts() -> Vec<Script> {
    let mut v = Vec::new();
    // A step of one tile, then either stop (drop straight down) or keep drifting until landing.
    for dir in [-1, 1] {
        for dir_ticks in [5, FOREVER] {
            v.push(Script { jump_hold: 0, dir, dir_ticks, min_ticks: 5 });
        }
    }
    // Jumps of different heights, straight up or drifting for a while / the whole flight.
    for jump_hold in [2, 5, 8, 12] {
        v.push(Script { jump_hold, dir: 0, dir_ticks: 0, min_ticks: 1 });
        for dir in [-1, 1] {
            for dir_ticks in [6, 12, FOREVER] {
                v.push(Script { jump_hold, dir, dir_ticks, min_ticks: 1 });
            }
        }
    }
    v
}

#[derive(Default, Debug)]
pub struct Report {
    pub errors: Vec<String>,
    pub info: Vec<String>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Where the hero stands and what he has done: the search graph's nodes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct State {
    col: i32,
    feet: i32,
    keys: u8,
    switches: u32,
}

/// What a script touched that changes the state.
#[derive(Default, Clone, Copy)]
struct Touched {
    keys: u8,
    switches: u32,
}

/// What the search has touched so far, in any state.
struct Found {
    item: Vec<bool>,
    door: [bool; 4],
    exit: bool,
}

struct Outcome {
    found: Found,
    reachable: usize,
}

struct Checker {
    /// The level with all gates closed (and, for the second search, breakables removed).
    base: Level,
    /// Everything the hero can touch: keys, health, gems, switches, secrets.
    items: Vec<Spawn>,
    item_at: HashMap<(i32, i32), usize>,
    /// Per item: the bit it sets in `State::switches` (0 if it is no switch).
    switch_bit: Vec<u32>,
    /// The level as it looks with a given set of switches pressed.
    by_switches: RefCell<HashMap<u32, Rc<Level>>>,
}

impl Checker {
    fn new(level: &Level, open_breakables: bool) -> Self {
        let mut base = level.clone();
        if open_breakables {
            for (x, y) in (0..level.h).flat_map(|y| (0..level.w).map(move |x| (x as i32, y as i32))) {
                if matches!(base.get(x, y), Some(Tile::Crate | Tile::Cracked)) {
                    base.set(x, y, Tile::Empty);
                }
            }
        }
        let items: Vec<Spawn> = level.spawns.iter().copied().filter(|s| !matches!(s.kind, SpawnKind::Bot | SpawnKind::Hint(_))).collect();
        let item_at = items.iter().enumerate().map(|(i, s)| ((s.x as i32, s.y as i32), i)).collect();
        let mut next_bit = 0;
        let switch_bit = items
            .iter()
            .map(|s| {
                if s.kind != SpawnKind::Switch {
                    return 0;
                }
                next_bit += 1;
                1 << (next_bit - 1)
            })
            .collect();
        Checker { base, items, item_at, switch_bit, by_switches: RefCell::new(HashMap::new()) }
    }

    fn level(&self, switches: u32) -> Rc<Level> {
        let mut cache = self.by_switches.borrow_mut();
        cache
            .entry(switches)
            .or_insert_with(|| {
                let mut l = self.base.clone();
                for (i, s) in self.items.iter().enumerate() {
                    if self.switch_bit[i] & switches != 0 {
                        for link in self.base.links.iter().filter(|k| k.switch == (s.x, s.y)) {
                            l.open_gate(link.gate.0, link.gate.1);
                        }
                    }
                }
                Rc::new(l)
            })
            .clone()
    }

    /// Records everything the body overlaps and returns what changes the state.
    fn touch(&self, level: &Level, b: &Body, found: &mut Found) -> Touched {
        let range = |lo: f32, hi: f32| (lo / T).floor() as i32..=((hi - 0.01) / T).floor() as i32;
        let mut touched = Touched::default();
        for ty in range(b.y, b.y + b.h) {
            for tx in range(b.x, b.x + b.w) {
                if let Some(&i) = self.item_at.get(&(tx, ty)) {
                    found.item[i] = true;
                    match self.items[i].kind {
                        SpawnKind::Key(c) => touched.keys = Keys(touched.keys).with(c).0,
                        SpawnKind::Switch => touched.switches |= self.switch_bit[i],
                        _ => {}
                    }
                }
                found.exit |= level.get(tx, ty) == Some(Tile::Exit);
            }
            // Standing against a door counts as reaching it.
            for tx in range(b.x - 0.5, b.x + b.w + 0.5) {
                if let Some(Tile::Door(c)) = level.get(tx, ty) {
                    found.door[c as usize] = true;
                }
            }
        }
        touched
    }

    /// Runs the script from a standing position. Returns where the hero stands afterwards
    /// (None if he never does) and what he touched on the way.
    fn run(&self, level: &Level, p: &mut Player, s: Script, found: &mut Found) -> (Option<(f32, f32)>, Touched) {
        let mut touched = Touched::default();
        for t in 0..MAX_TICKS {
            let c = Controls {
                jump: t < s.jump_hold,
                left: s.dir < 0 && t < s.dir_ticks,
                right: s.dir > 0 && t < s.dir_ticks,
                fire: false,
            };
            p.update(level, c);
            let now = self.touch(level, &p.body, found);
            (touched.keys, touched.switches) = (touched.keys | now.keys, touched.switches | now.switches);
            if p.on_ground && t + 1 >= s.min_ticks {
                return (Some((p.body.x, p.body.y)), touched);
            }
        }
        (None, touched)
    }
}

/// Nearest tile-aligned standing position for a hero at (x, y) resting on the floor.
fn node(level: &Level, keys: Keys, x: f32, y: f32) -> (i32, i32) {
    let feet = ((y + 8.0) / T).round() as i32;
    let c0 = (x / T).floor() as i32;
    let overlapped = if x > c0 as f32 * T + 0.01 { vec![c0, c0 + 1] } else { vec![c0] };
    let col = overlapped
        .into_iter()
        .filter(|&c| solid_at(level, keys, c as f32 * T + 2.0, feet as f32 * T + 1.0))
        .min_by_key(|&c| ((x - c as f32 * T).abs() * 100.0) as i32)
        .unwrap_or(c0);
    (col, feet)
}

fn player_at(st: State) -> Player {
    let mut p = Player::spawn((0, 0));
    p.body.x = st.col as f32 * T;
    p.body.y = st.feet as f32 * T - p.body.h;
    p.keys = Keys(st.keys);
    p.on_ground = true;
    p
}

/// Searches everything reachable from the start. None if the start is not above solid ground.
fn analyse(level: &Level, open_breakables: bool) -> Option<Outcome> {
    let ck = Checker::new(level, open_breakables);
    let mut found = Found { item: vec![false; ck.items.len()], door: [false; 4], exit: false };

    // The start may be above the ground: let the hero drop first.
    let closed = ck.level(0);
    let mut p = Player::spawn(level.player);
    let idle = Script { jump_hold: 0, dir: 0, dir_ticks: 0, min_ticks: 1 };
    let (landed, touched) = ck.run(&closed, &mut p, idle, &mut found);
    let (x, y) = landed?;
    let (col, feet) = node(&closed, Keys::default(), x, y);
    let start = State { col, feet, keys: touched.keys, switches: touched.switches };

    let scripts = scripts();
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(st) = queue.pop_front() {
        let lv = ck.level(st.switches);
        for &s in &scripts {
            let mut p = player_at(st);
            let (landed, touched) = ck.run(&lv, &mut p, s, &mut found);
            let Some((x, y)) = landed else { continue };
            let keys = st.keys | touched.keys;
            let (col, feet) = node(&lv, Keys(keys), x, y);
            let next = State { col, feet, keys, switches: st.switches | touched.switches };
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    Some(Outcome { found, reachable: seen.len() })
}

/// A required thing the strict search never touched is an error; if the loose search reached it,
/// the message says breaking something is the way in.
fn require(errors: &mut Vec<String>, text: String, strict_ok: bool, loose_ok: bool, suffix: &str) {
    if !strict_ok {
        let why = if loose_ok { " unless a crate or cracked wall is broken" } else { suffix };
        errors.push(format!("{text} cannot be reached{why}"));
    }
}

pub fn check(level: &Level) -> Report {
    let mut report = Report::default();
    let switches = level.spawns.iter().filter(|s| s.kind == SpawnKind::Switch).count();
    if switches > MAX_SWITCHES {
        report.errors.push(format!("the level has {switches} switches, the validator handles {MAX_SWITCHES}"));
        return report;
    }
    let Some(strict) = analyse(level, false) else {
        report.errors.push(format!("the player start ({}) is not above solid ground", at(level.player.0, level.player.1)));
        return report;
    };
    let has_breakables = (0..level.h).any(|y| (0..level.w).any(|x| matches!(level.get(x as i32, y as i32), Some(Tile::Crate | Tile::Cracked))));
    let loose = if has_breakables { analyse(level, true) } else { None };
    let loose = loose.as_ref().unwrap_or(&strict);

    let items: Vec<Spawn> = level.spawns.iter().copied().filter(|s| !matches!(s.kind, SpawnKind::Bot | SpawnKind::Hint(_))).collect();
    let item = |i: usize| (strict.found.item[i], loose.found.item[i]);

    let keys: Vec<(usize, Color)> = items.iter().enumerate().filter_map(|(i, s)| if let SpawnKind::Key(c) = s.kind { Some((i, c)) } else { None }).collect();
    for &(i, c) in &keys {
        require(&mut report.errors, format!("the {} key at {}", c.name(), at(items[i].x, items[i].y)), item(i).0, item(i).1, "");
    }
    let first_tile = |t: Tile| {
        (0..level.h).flat_map(|y| (0..level.w).map(move |x| (x, y))).find(|&(x, y)| level.get(x as i32, y as i32) == Some(t))
    };
    let mut missing_key = false;
    for c in Color::ALL {
        let Some((x, y)) = first_tile(Tile::Door(c)) else { continue };
        if !keys.iter().any(|&(_, k)| k == c) {
            missing_key = true;
            report.errors.push(format!("the {} door at {} needs a {} key, but the level has none", c.name(), at(x, y), c.name()));
        } else if !strict.found.door[c as usize] {
            let why = if loose.found.door[c as usize] { " unless a crate or cracked wall is broken" } else { "" };
            report.errors.push(format!("the {} door at {} cannot be reached{why}", c.name(), at(x, y)));
        }
    }
    for (i, s) in items.iter().enumerate().filter(|(_, s)| s.kind == SpawnKind::Switch) {
        require(&mut report.errors, format!("the switch at {}", at(s.x, s.y)), item(i).0, item(i).1, "");
    }
    match first_tile(Tile::Exit) {
        None => report.errors.push("the level has no exit".into()),
        Some((x, y)) => {
            let with_keys = !keys.is_empty() && !missing_key && keys.iter().all(|&(i, _)| strict.found.item[i]);
            let why = if with_keys { " even with the keys" } else { "" };
            require(&mut report.errors, format!("the exit at {}", at(x, y)), strict.found.exit, loose.found.exit, why);
        }
    }
    for (i, s) in items.iter().enumerate().filter(|(_, s)| s.kind == SpawnKind::Secret) {
        match (strict.found.item[i], loose.found.item[i]) {
            (true, _) => {}
            (false, true) => report.info.push(format!("the secret area at {} needs a crate or cracked wall broken", at(s.x, s.y))),
            (false, false) => report.errors.push(format!("the secret area at {} cannot be reached", at(s.x, s.y))),
        }
    }

    for (kind, name) in [(SpawnKind::Gem, "gem"), (SpawnKind::Health, "health pickup")] {
        let list = |f: &dyn Fn(usize) -> bool| -> Vec<String> {
            items.iter().enumerate().filter(|(i, s)| s.kind == kind && f(*i)).map(|(_, s)| at(s.x, s.y)).collect()
        };
        let lost = list(&|i| !loose.found.item[i]);
        let behind = list(&|i| loose.found.item[i] && !strict.found.item[i]);
        if !lost.is_empty() {
            report.info.push(format!("{} {name}(s) cannot be reached: {}", lost.len(), lost.join("; ")));
        }
        if !behind.is_empty() {
            report.info.push(format!("{} {name}(s) need a crate or cracked wall broken: {}", behind.len(), behind.join("; ")));
        }
    }
    report.info.push(format!("{} standing positions reachable", strict.reachable));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn check_text(t: &str) -> Report {
        check(&Level::parse(t).unwrap())
    }

    fn level_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()) {
            if e.is_dir() && e.file_name().unwrap() != "invalid" {
                level_files(&e, out);
            } else if e.extension().is_some_and(|x| x == "txt") {
                out.push(e);
            }
        }
    }

    #[test]
    fn every_level_passes() {
        let mut files = Vec::new();
        level_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("levels"), &mut files);
        assert!(files.len() >= 2);
        for f in files {
            let level = Level::parse(&std::fs::read_to_string(&f).unwrap()).unwrap();
            let r = check(&level);
            assert!(r.is_ok(), "{}: {:?}", f.display(), r.errors);
        }
    }

    #[test]
    fn invalid_levels_fail_with_a_clear_message() {
        let r = check(&Level::parse(include_str!("../levels/test/invalid/missing_jump.txt")).unwrap());
        assert_eq!(r.errors, ["the exit at column 19, row 3 cannot be reached"]);
        let r = check(&Level::parse(include_str!("../levels/test/invalid/walled_key.txt")).unwrap());
        assert_eq!(r.errors, ["the yellow key at column 12, row 8 cannot be reached", "the exit at column 20, row 8 cannot be reached"]);
    }

    #[test]
    fn jump_reaches_three_tiles_but_not_four() {
        // Exit on a block `h` tiles above the floor, the hero on the floor next to it.
        let level = |h: usize| {
            let mut g = vec![vec!['.'; 12]; 8];
            g[7] = vec!['#'; 12];
            for y in 7 - h..7 {
                g[y][6] = '#';
                g[y][7] = '#';
            }
            g[7 - h - 1][6] = 'X';
            g[6][2] = 'P';
            g.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
        };
        assert!(check_text(&level(3)).is_ok(), "{:?}", check_text(&level(3)).errors);
        assert!(!check_text(&level(4)).is_ok());
    }

    #[test]
    fn a_door_without_a_matching_key_is_an_error() {
        let r = check_text("##########\n#........#\n#P..g.R.X#\n##########");
        assert_eq!(r.errors, ["the red door at column 7, row 3 needs a red key, but the level has none", "the exit at column 9, row 3 cannot be reached"]);
        let r = check_text("##########\n#........#\n#P..r.R.X#\n##########");
        assert!(r.is_ok(), "{:?}", r.errors);
    }

    #[test]
    fn the_keys_level_needs_every_key() {
        let text = include_str!("../levels/test/keys.txt");
        let r = check_text(&text.replacen('b', ".", 1));
        assert!(r.errors.iter().any(|e| e.contains("the blue door at column 37, row 2 needs a blue key")), "{:?}", r.errors);
        // The green key moved behind the green door can never be collected.
        let mut rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        (rows[7][19], rows[7][30]) = ('.', 'g');
        let moved = rows.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
        let r = check_text(&moved);
        assert!(r.errors.contains(&"the green key at column 31, row 8 cannot be reached".to_string()), "{:?}", r.errors);
    }

    #[test]
    fn a_floating_start_drops_to_the_ground() {
        assert!(check_text("#####\n#...#\n#P..#\n#...#\n#..X#\n#####").is_ok());
    }

    #[test]
    fn the_gate_needs_its_switch_to_be_reachable() {
        // The switch sits on the floor: pressing it opens the gate in front of the exit.
        let open = "##########\n#...!....#\n#PS.!...X#\n##########\n---\nlink 3,3 5,3";
        assert!(check_text(open).is_ok(), "{:?}", check_text(open).errors);
        // The same switch high up on an unreachable ledge.
        let closed = "##########\n#..S.!...#\n#..#.!...#\n#P.#.!..X#\n##########\n---\nlink 4,2 6,2";
        let r = check_text(closed);
        assert_eq!(r.errors, ["the switch at column 4, row 2 cannot be reached", "the exit at column 9, row 4 cannot be reached"]);
    }

    #[test]
    fn breakables_are_solid_but_the_report_says_so() {
        // A wall of crates is too high to hop: the exit needs breaking it.
        let wall = "###########\n#...C.....#\n#...C.....#\n#P..C..*.X#\n###########";
        let r = check_text(wall);
        assert_eq!(r.errors, ["the exit at column 10, row 4 cannot be reached unless a crate or cracked wall is broken"]);
        assert!(r.info.iter().any(|i| i.contains("1 gem(s) need a crate or cracked wall broken: column 8, row 4")), "{:?}", r.info);
    }

    #[test]
    fn secrets_behind_cracked_walls_are_info_and_sealed_ones_are_errors() {
        let ok = "###########\n#.........#\n#P..%.s..X#\n###########";
        let r = check_text(ok);
        assert!(r.errors.is_empty() || r.errors.iter().all(|e| !e.contains("secret")), "{:?}", r.errors);
        assert!(r.info.iter().any(|i| i.contains("secret area at column 7, row 3 needs a crate or cracked wall broken")), "{:?}", r.info);
        let sealed = "###########\n#.........#\n#P..#.s..X#\n###########";
        let r = check_text(sealed);
        assert!(r.errors.iter().any(|e| e.contains("the secret area at column 7, row 3 cannot be reached")), "{:?}", r.errors);
    }
}

# Akvisionator — Plan

A 2D run-and-gun platformer that runs in the terminal, in the spirit of the early-90s
Apogee side-scrollers (Duke Nukem 1/2): big maze-like levels, colored keys, crates full of
goodies, secrets, lots of enemies that shoot back.

Graphics are drawn with the half-block trick: every character cell prints `▀`,
text color = top pixel, background color = bottom pixel → 2 square-ish pixels per cell,
24-bit color.

Visual reference: the browser mockup https://claude.ai/artifact/GQWcpduvyib169PNqJx2oU

**Spiritual successor, not a copy.** Mechanics are fair game; names, characters, sprites,
level maps, music and text of the original games are not. Everything here is our own.

## Tech

- **Rust** (1.94) + **crossterm** (terminal I/O, raw mode, key press/release events on Windows).
- No game engine, no ratatui — own tiny renderer on top of crossterm.
- Target terminal: **Windows Terminal** (truecolor). Other truecolor terminals are a bonus.

## Core numbers

| Thing | Value |
|---|---|
| Terminal | min 112 × 34 cells: HUD row + game + hint row; bigger windows scale the game up (uniform, letterboxed), follows resizes |
| Game view | 112 × 64 pixels (28 × 16 tiles visible) |
| Tile | 4 × 4 px |
| Level | P1: 84 × 16 tiles. P2+: any size, typically up to ~128 × 90 tiles |
| Hero | 5 × 8 px sprite, 4 × 8 hitbox |
| Enemy (bot) | 5 × 5 px |
| Physics | jump ≈ 14 px (3.5 tiles), run 0.9 px/tick — the rules level design must follow |
| Update rate | fixed 60 Hz simulation, render up to 60 fps |

---

## P1 — One playable level ✅ done

`cargo run --release` plays `levels/level1.txt`: run, jump, shoot bots, grab the key, open the
door, reach the exit. Diffing half-block renderer, fixed 60 Hz loop, held-key input, parallax
background, player physics (variable jump, snap-to-tile collisions), bots, bullets, particles,
pickups, HUD, death/restart, window scaling. 14 unit tests.

Level format: plain text, one char per tile. An optional `---` line after the grid starts the
metadata: `name <text>`, `hint <col>,<row> <text>` (one per `?`), `link <col>,<row> <col>,<row>`
(switch, then any tile of the gate). Columns and rows are 1-based; errors report line and column.

| Char | Meaning |
|---|---|
| `.` | empty |
| `#` | brick wall (solid) |
| `=` | steel girder (solid) |
| `R` `G` `B` `Y` | door (red, green, blue, yellow), solid until the hero has the key of that color |
| `r` `g` `b` `y` | key of that color |
| `S` | switch: touch it once, its linked gate opens |
| `!` | gate, solid until its switch is pressed |
| `%` | cracked wall: one shot breaks it (secret passages) |
| `C` | crate: one shot breaks it, it drops a gem, health or a bomb |
| `?` | hint globe: touching it shows its `hint` text in the hint row |
| `s` | secret area trigger (invisible), counts once toward "secrets found" |
| `X` | exit |
| `P` | player start |
| `E` | enemy (bot) spawn |
| `+` | health |
| `*` | gem (+100 score) |

Code layout: `main.rs` (terminal, loop), `render.rs` (framebuffer, half-block output, diffing,
scaling), `input.rs`, `level.rs`, `sprites.rs`, `background.rs`, `camera.rs`; since P2.2 the game is
split into `game.rs` (loop, HUD, bullets), `physics.rs`, `player.rs`, `enemies.rs`, `entities.rs`.
Pickups and bots are entities (`Kind::Bot`, `Kind::Pickup`); tiles are only the static grid.

---

## P2 — The engine for real levels

**Goal:** every mechanic the episode needs exists, works, and is shown off in small test levels
(`levels/test/*.txt`). No real level design yet.

### Steps

1. **Big levels + two-axis camera** ✅ done
   Levels of any size. Camera follows on both axes with a dead zone and a little look-ahead in
   the facing direction; clamps to level edges. Parallax background also scrolls vertically.
   Only visible tiles/entities are drawn; entities far off-screen are frozen.
   → verify: a 128 × 90 test level scrolls smoothly everywhere, fps unchanged vs. P1;
   camera never shows outside the level.
   `levels/test/big.txt` (128 × 90 tower); play it with `cargo run --release -- levels/test/big.txt`.

2. **Level format v2 + entity system** ✅ done (only `name` so far; `hint`/`link`/`teleport` arrive with their features in steps 4 and 7)
   File = tile grid, then a `---` line, then metadata lines:
   `name`, `music`/`theme` (later), `hint <x>,<y> <text>`, `link <switch> <target>`, `teleport <a> <b>`.
   Tiles stay a grid (static); everything that moves or can be picked up/destroyed becomes an
   entity with a kind, position and small per-kind state. `game.rs` splits into
   `player.rs`, `entities.rs`, `enemies.rs`, `physics.rs` as it grows.
   → verify: level1 still plays the same (converted to v2); loader errors name the line/column.

3. **Level validator (automatic checks)** ✅ done (keys/doors; boots and hook join in step 5)
   Coarse movement graph from the physics numbers (walk, fall, jump ≈ 3 tiles up / ~5 across,
   boots and hook when collected). BFS from the start: every key, the door behind it, and the exit
   must be reachable in order; report unreachable gems/secrets as info. Runs in `cargo test`
   over every level, and as `cargo run -- --check <file>`.
   → verify: level1 passes; test levels with a missing jump / walled-off key fail with a clear message.
   This is what lets levels be designed without guesswork (and later: random levels).
   Implemented as a BFS over standing positions where every step is a short walk/jump script run
   through the real `Player::update` (`src/validator.rs`), so it follows the physics automatically.
   Positions are tile-aligned, so a passing level keeps ~1 tile of margin on the longest leaps.
   Deliberately broken levels live in `levels/test/invalid/` (skipped by the all-levels test).

4. **Keys, doors, switches, crates, secrets, hints** ✅ done (see the level format below)
   Four key colors + matching doors. Switches that open/close linked walls. Breakable walls
   (secret passages). Shootable crates dropping items (health, points, powerups, sometimes a
   bomb). Hint globes: touch → text in the hint row. Secret areas marked in the level count
   toward the tally.
   → verify: one test level per feature (`levels/test/keys.txt`, `switch.txt`, `secrets.txt`);
   validator understands keys and switches.
   Choices made: a switch fires once when touched and opens the whole connected gate group;
   crates and cracked walls take one shot and are solid for the validator (a second search with
   them removed tells "needs breaking" from "unreachable"); a crate's content is fixed per
   position (50% gem, 30% health, 20% bomb) until powerups exist (step 5); shots fly at the height
   of the hero's upper body tile, so crates and cracked walls must sit at that height.

5. **Player upgrades + health**
   Health 8 units; items: soda (+1), full meal (+4). Firepower pickups: 1 → 4 bullets on screen,
   faster fire. Jump boots (higher jump). Grappling hook: hang from marked ceiling tiles and
   climb along them. Upgrades last for the whole episode, lost on game over (see step 8).
   → verify: each upgrade has a test level that is impossible without it and passes with it (validator).

6. **Enemies**
   Shared parts: patrol, gravity, hit flash, drops, enemy bullets. Types:
   walker bot (have), **turret** (wall-mounted, shoots on sight), **drone** (flies in a sine,
   dives), **wall crawler** (walks on walls/ceilings), **hopper** (jumps toward the player),
   **heavy** (4 hits, shoots bursts), **mine** (static, explodes). Enemies only activate near the camera.
   → verify: test arena per type; each can hurt the player and can be killed; no enemy walks through walls.

7. **Hazards + moving parts**
   Spikes, fire jets (timed), acid (instant hurt), crumbling floors, conveyor belts, fans (push up),
   elevators / moving platforms (player rides them), teleporters (paired).
   → verify: test level per mechanic; riding a platform has no jitter; validator knows teleporters
   and treats elevators as vertical links.

8. **Game flow**
   Title screen (logo in half-block art, start/quit). Level sequence from `levels/episode1/`.
   Level-end tally: kills %, secrets %, gems, time bonus → score. Pause (P / Esc asks to quit).
   Death = restart level with the health/upgrades you entered it with. Save progress
   (current level, score, upgrades) + high score table in a small file next to the exe.
   → verify: title → level → tally → next level → quit → relaunch continues where you left off.

**P2 done when:** every test level passes the validator and plays correctly, level1 still works,
steady 60 fps in a full-screen Windows Terminal.

---

## P3 — Episode 1

**Goal:** 10 hand-made levels + a boss, in `levels/episode1/`, each introducing or remixing
mechanics, ~3–8 minutes each.

### Level arc (draft)

| # | Idea | New thing |
|---|---|---|
| 1 | Rooftops (level1, enlarged) | basics, crates |
| 2 | Warehouse | colored keys, switches |
| 3 | Sewers | acid, crawlers, first secrets |
| 4 | Factory | conveyors, crumbling floors, turrets |
| 5 | Elevator shaft | tall level, elevators, hook |
| 6 | Lab | teleporters, drones, boots |
| 7 | Reactor | fire jets, heavies, mines |
| 8 | Maze | everything, big secret-heavy level |
| 9 | Fortress | hard remix, few health items |
| 10 | Boss | multi-phase boss in an arena |

### Steps

1. **Workflow**: agent drafts a level → validator passes → you playtest → notes → agent tweaks.
2. **Levels 1–9** in the order above. Each gets its own tile palette tweak (brick/metal/sewer colors).
3. **Boss**: large sprite (≈ 16 × 16 px), 3 phases with different attack patterns, health bar in HUD.
4. **Difficulty pass**: health/enemy placement tuned from playtest notes; full playthrough.

→ verify per level: validator green, you've played it through once and it's fun.

**P3 done when:** the whole episode can be played start to finish, ~45–60 minutes.

---

## P4 — Polish & more

- **Sound** (rodio): synthesized chip-style SFX generated in code (no asset files): shot, hit,
  pickup, door, death. Optional simple music loop. Mute key.
- **Juice**: screen shake on explosions, better death animation, level intro card.
- **Random levels**: `generate(seed, params) -> Level` next to the text loader. Stitch hand-made
  chunk rooms (small text files) in random order, place keys/enemies/items, then run the
  validator — reject and retry until the level is finishable. Seed shown on screen for sharing.
- **Episodes 2–3**: new tile sets, 2–3 new enemy types each, new boss. Same workflow as P3.
- Maybe: level editor inside the game, gamepad support, Linux/macOS key-release fallback.

---

## Effort estimate

Rough baseline (estimated, not measured): **P1 took about 40 min of agent time, ~30k output tokens** (~1,300 lines of
Rust incl. tests). Total tokens processed are much higher (the growing conversation is re-read
every turn, mostly from cache): assume **~3–5M total tokens per agent hour**, ~40–60k of them output.
These are estimates; mechanics with fiddly physics (hook, platforms, crawlers) are the most likely to overrun.

| Phase / step | Agent hours | Output tokens | Total tokens (incl. context) |
|---|---|---|---|
| P2.1 Big levels + camera | 1 – 1.5 | 50k | 4 – 7M |
| P2.2 Format v2 + entities (refactor) | 1.5 – 2 | 80k | 6 – 10M |
| P2.3 Validator | 1 – 1.5 | 50k | 4 – 7M |
| P2.4 Keys, switches, crates, secrets, hints | 1.5 – 2 | 70k | 6 – 10M |
| P2.5 Upgrades + health | 1.5 – 2 | 70k | 6 – 10M |
| P2.6 Enemies (6 types) | 2 – 3 | 110k | 8 – 15M |
| P2.7 Hazards + moving parts | 2 – 3 | 110k | 8 – 15M |
| P2.8 Game flow, tally, save | 1.5 – 2 | 70k | 6 – 10M |
| **P2 total** | **12 – 17** | **~600k** | **~50 – 85M** |
| P3 levels 1–9 (≈1–1.5 h each incl. fixes) | 9 – 14 | 400k | 35 – 70M |
| P3 boss + difficulty pass | 2 – 4 | 100k | 8 – 20M |
| **P3 total** | **11 – 18** | **~500k** | **~45 – 90M** |
| P4 sound + juice | 3 – 5 | 150k | 12 – 25M |
| P4 random levels | 3 – 5 | 150k | 12 – 25M |
| P4 per extra episode | ≈ P3 | ≈ P3 | ≈ P3 |

**Human time** (not included above): playtesting is the bottleneck for P3 — about 30–60 min
per level including notes, ~8–10 hours for the episode. P2 needs ~15 min of play per step to
confirm it feels right.

Working in one session per step (fresh context) keeps total tokens at the low end of each range.

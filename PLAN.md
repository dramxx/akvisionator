# Akvisionator — Plan

A 2D run-and-gun platformer (Duke Nukem 1/2 style) that runs in the terminal.
Graphics are drawn with the half-block trick: every character cell prints `▀`,
text color = top pixel, background color = bottom pixel → 2 square-ish pixels per cell,
24-bit color.

Visual reference: the browser mockup https://claude.ai/artifact/GQWcpduvyib169PNqJx2oU

## Tech

- **Rust** (1.94) + **crossterm** (terminal I/O, raw mode, key press/release events on Windows).
- No game engine, no ratatui — own tiny renderer on top of crossterm.
- Target terminal: **Windows Terminal** (truecolor). Other truecolor terminals are a bonus.

## Core numbers

| Thing | Value |
|---|---|
| Screen | 112 × 34 cells → HUD row + 32 game rows + hint row |
| Game view | 112 × 64 pixels (32 rows × 2) |
| Tile | 4 × 4 px |
| Level (P1) | 84 × 16 tiles = 336 × 64 px, scrolls horizontally |
| Hero | 5 × 8 px sprite, 4 × 8 hitbox |
| Enemy (bot) | 5 × 5 px |
| Update rate | fixed 60 Hz simulation, render as fast as possible up to 60 fps |

---

## P1 — One playable level

**Goal:** Launch `cargo run`, get the level from the mockup in the real terminal,
run, jump, shoot bots, grab the key, open the door, reach the exit.

**Out of scope for P1:** random levels, sound, menus, saving, multiple levels, config.

### Level format

Levels are plain text files, one char per tile. P1 ships `levels/level1.txt`
(the mockup level):

```
####################################################################################
#.............................................................................#....#
#.............................................................................#....#
#.............................................................................#....#
#..........................*.E.............................k..................#....#
#.......................=========.......................=======...............#....#
#.............................................................................#....#
#................*................................E...................E.*.....#....#
#.............=======.........................=========.............=======...#....#
#.............................................................................#....#
#..........+............................E*....................................D....#
#.......=======.....................=========...............=======...........D....#
#...................####................................##....................D..X.#
#.P.................####..E..................*....E.....##........*...........D..X.#
####################################################################################
####################################################################################
```

| Char | Meaning |
|---|---|
| `.` | empty |
| `#` | brick wall (solid) |
| `=` | steel girder (solid) |
| `D` | door, solid until the player has the key |
| `X` | exit |
| `P` | player start |
| `E` | enemy (bot) spawn |
| `k` | key |
| `+` | health |
| `*` | gem (+100 score) |

The loader turns the text into a `Level` struct (tile grid + list of spawns).
The game only ever sees `Level` — see "Future: random levels" below.

### Steps

1. **Project skeleton + terminal setup**
   `cargo new`, add crossterm. Enter raw mode + alternate screen, hide cursor,
   restore everything on exit *and* on panic.
   → verify: `cargo run` shows a blank screen, `Esc`/`Q` quits, terminal is clean afterwards
   (also after a forced panic).

2. **Half-block renderer**
   `Framebuffer` of 112 × 64 RGB pixels → cells (`▀`, fg = top, bg = bottom).
   Keep the previous frame, write only changed cells, batch output into one buffer, flush once per frame.
   Text rows (HUD) drawn as normal characters.
   Warn and show "please enlarge the window" if the terminal is smaller than 112 × 34.
   → verify: a color gradient + moving square renders without flicker or seams;
   show fps in the hint row, target ≥ 30, ideally 60.

3. **Input + game loop**
   Fixed 60 Hz update with accumulator. Track held keys from press/release events
   (←/→/A/D move, Z/↑/W jump, X/Space fire, Esc/Q quit).
   → verify: holding → moves the square smoothly with no auto-repeat stutter;
   pressing two keys at once works (run + jump + fire).

4. **Level loading + drawing**
   Parse `levels/level1.txt`, draw tiles (brick with mortar pattern + lit top edge,
   girders, door, exit), camera follows the player horizontally.
   Parallax background: sky gradient, stars, two skyline layers with lit windows.
   → verify: the level looks like the mockup; camera scrolls from start to exit.

5. **Player physics**
   Gravity, jump (variable height: release early = lower jump), run,
   tile collisions per axis in small steps. Hero sprite with stand / run / jump frames, faces left/right.
   → verify: can reach every platform in level1, can't pass through walls or ceilings,
   no jitter when standing.

6. **Combat + enemies**
   Bullets (fire cooldown, muzzle flash), bots walk and turn at walls and edges,
   2 hits to kill, white hit flash, spark particles. Touching a bot = −1 health,
   knockback, short invulnerability (blinking).
   → verify: bots die after 2 hits, player takes damage and blinks, dies at 0 health.

7. **Pickups, door, exit, HUD**
   Key, health, gems, score. Door opens with the key. Exit shows "LEVEL CLEAR".
   Death shows "YOU DIED" and restarts the level.
   HUD: health bar, score, key, level name. Hint row: controls.
   → verify: full run from start to exit is possible; death and restart work.

**P1 done when:** a full playthrough of level1 in Windows Terminal feels like the mockup,
at a steady frame rate, and the terminal is always restored on exit.

### Code layout (P1)

```
src/
  main.rs       terminal setup/teardown, main loop
  render.rs     Framebuffer, half-block output, diffing, text rows
  input.rs      held-key state from crossterm events
  level.rs      Level struct, text loader, tile queries (solid?)
  game.rs       world state, update step (player, enemies, bullets, pickups)
  sprites.rs    sprite data + palette
  background.rs parallax layers
levels/
  level1.txt
```

---

## Future: random levels (not P1)

Planned for a later phase. P1 already prepares for it:

- The game consumes a `Level` struct and doesn't care where it came from.
  A generator will be a function `generate(seed, params) -> Level`, next to the text loader.
- Seeded RNG, so a seed reproduces a level (handy for sharing and debugging).
- A generated level must be checked as finishable: player can reach key → door → exit
  with the P1 jump height (≈ 3 tiles up) and run speed. The physics numbers from step 5
  become the rules the generator follows.
- Possible approach: stitch hand-made "chunks" (small text-file rooms) in random order,
  then place enemies/pickups. Simpler and more fun than pure noise.

## Later ideas (unsorted)

- Sound (rodio), more enemy types, weapons, more tile sets.
- Levels taller than the screen (vertical scrolling).
- Window size adapts: bigger terminal = bigger view.

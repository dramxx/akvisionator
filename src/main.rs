mod background;
mod camera;
mod enemies;
mod entities;
mod game;
mod input;
mod level;
mod physics;
mod player;
mod render;
mod sprites;
mod validator;

use std::io::{self, Write, stdout};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event};
use crossterm::style::{Print, ResetColor};
use crossterm::terminal::{
    self, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, queue};

use game::{Game, VIEW_H, VIEW_W};
use input::Input;
use level::Level;
use render::{Framebuffer, Screen};

/// Minimum terminal size: HUD row + game at 1x + hint row. Bigger terminals scale the game up.
const COLS: usize = VIEW_W;
const ROWS: usize = VIEW_H / 2 + 2;
const TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// Raw mode + alternate screen for as long as it lives.
struct Terminal;

impl Terminal {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, cursor::Hide, DisableLineWrap, EnableFocusChange, Clear(ClearType::All))?;
        // Windows reports key releases natively; elsewhere they need the kitty keyboard protocol.
        #[cfg(not(windows))]
        if terminal::supports_keyboard_enhancement().unwrap_or(false) {
            use crossterm::event::{KeyboardEnhancementFlags as F, PushKeyboardEnhancementFlags};
            execute!(stdout(), PushKeyboardEnhancementFlags(F::REPORT_EVENT_TYPES | F::DISAMBIGUATE_ESCAPE_CODES))?;
        }
        Ok(Terminal)
    }
}

fn restore() {
    #[cfg(not(windows))]
    let _ = execute!(stdout(), event::PopKeyboardEnhancementFlags);
    let _ = execute!(stdout(), ResetColor, DisableFocusChange, EnableLineWrap, cursor::Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore();
    }
}

fn load(path: &str) -> io::Result<Level> {
    Level::parse(&std::fs::read_to_string(path)?).map_err(|e| io::Error::other(format!("{path}: {e}")))
}

fn main() -> io::Result<()> {
    // `akvisionator [level.txt]` plays a level file instead of the built-in level 1;
    // `akvisionator --check level.txt` only validates it (exit code 1 if it cannot be finished).
    let mut args = std::env::args().skip(1);
    let level = match args.next().as_deref() {
        Some("--check") => {
            let path = args.next().ok_or_else(|| io::Error::other("usage: akvisionator --check <level.txt>"))?;
            let report = validator::check(&load(&path)?);
            report.info.iter().for_each(|i| println!("info: {i}"));
            report.errors.iter().for_each(|e| println!("error: {e}"));
            println!("{path}: {}", if report.is_ok() { "OK" } else { "FAILED" });
            std::process::exit(if report.is_ok() { 0 } else { 1 });
        }
        Some(path) => load(path)?,
        None => Level::parse(include_str!("../levels/level1.txt"))
            .map_err(|e| io::Error::other(format!("levels/level1.txt: {e}")))?,
    };

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        default_hook(info);
    }));

    let _terminal = Terminal::enter()?;
    run(Game::new(level))
}

fn run(mut game: Game) -> io::Result<()> {
    let mut out = stdout();
    let mut input = Input::default();
    let mut fb = Framebuffer::new(VIEW_W, VIEW_H);
    let (mut term_w, mut term_h) = terminal::size()?;
    let mut screen = Screen::new(term_w as usize, term_h as usize);
    let mut warned = false;

    let mut acc = Duration::ZERO;
    let mut last = Instant::now();
    let (mut fps, mut frames, mut fps_since) = (0, 0, Instant::now());

    loop {
        let frame_start = Instant::now();

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => input.handle(k),
                Event::FocusLost => input.clear(),
                Event::Resize(w, h) => {
                    (term_w, term_h) = (w, h);
                    warned = false;
                    execute!(out, ResetColor, Clear(ClearType::All))?;
                    screen = Screen::new(w as usize, h as usize);
                }
                _ => {}
            }
        }
        if input.quit {
            return Ok(());
        }

        acc += (frame_start - last).min(Duration::from_millis(100));
        last = frame_start;
        while acc >= TICK {
            game.update(input.controls());
            acc -= TICK;
        }

        if (term_w as usize) < COLS || (term_h as usize) < ROWS {
            if !warned {
                let msg = format!("Please enlarge the window to at least {COLS} x {ROWS} (now {term_w} x {term_h}). Esc quits.");
                queue!(out, ResetColor, Clear(ClearType::All), cursor::MoveTo(0, 0), Print(msg))?;
                out.flush()?;
                warned = true;
            }
        } else {
            game.render(&mut fb);
            screen.blit(&fb, 1, screen.rows - 2, game::HUD_BG);
            game.hud(&mut screen, fps);
            screen.flush(&mut out)?;
            frames += 1;
        }

        if fps_since.elapsed() >= Duration::from_secs(1) {
            (fps, frames, fps_since) = (frames, 0, Instant::now());
        }

        if let Some(rest) = TICK.checked_sub(frame_start.elapsed()) {
            thread::sleep(rest);
        }
    }
}

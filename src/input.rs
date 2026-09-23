use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::game::Controls;

/// Held-key state built from press/release events (no reliance on auto-repeat).
#[derive(Default)]
pub struct Input {
    held: HashSet<KeyCode>,
    pub quit: bool,
}

impl Input {
    pub fn handle(&mut self, k: KeyEvent) {
        let code = match k.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        match k.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => {
                let ctrl_c = code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL);
                if matches!(code, KeyCode::Esc | KeyCode::Char('q')) || ctrl_c {
                    self.quit = true;
                }
                self.held.insert(code);
            }
            KeyEventKind::Release => {
                self.held.remove(&code);
            }
        }
    }

    /// Drops all held keys, e.g. when the window loses focus and releases would be missed.
    pub fn clear(&mut self) {
        self.held.clear();
    }

    fn any(&self, codes: &[KeyCode]) -> bool {
        codes.iter().any(|c| self.held.contains(c))
    }

    pub fn controls(&self) -> Controls {
        Controls {
            left: self.any(&[KeyCode::Left, KeyCode::Char('a')]),
            right: self.any(&[KeyCode::Right, KeyCode::Char('d')]),
            jump: self.any(&[KeyCode::Up, KeyCode::Char('z'), KeyCode::Char('w')]),
            fire: self.any(&[KeyCode::Char('x'), KeyCode::Char(' ')]),
        }
    }
}

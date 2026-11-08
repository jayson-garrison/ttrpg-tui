//! The thin wrapper around crossterm. Logic lives in `App`; this only moves
//! events in and frames out, so it is verified by the terminal mock and QA.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::backend::Backend;
use ratatui::Terminal;

use super::app::App;

const TICK: Duration = Duration::from_millis(250);

/// Only presses and repeats are keys; releases (sent by some terminals) are not.
pub fn is_keypress(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub fn run_loop<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> io::Result<()>
where
    io::Error: From<B::Error>,
{
    loop {
        terminal.draw(|frame| app.render(frame))?;
        if event::poll(TICK)? {
            if let Event::Key(key) = event::read()? {
                if is_keypress(key.kind) && !app.handle_key(key) {
                    return Ok(());
                }
            }
        }
        app.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_events_are_ignored() {
        assert!(!is_keypress(KeyEventKind::Release));
    }

    #[test]
    fn press_and_repeat_are_handled() {
        assert!(is_keypress(KeyEventKind::Press));
        assert!(is_keypress(KeyEventKind::Repeat));
    }
}

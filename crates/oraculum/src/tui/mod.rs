//! The terminal interface (spec 4, A§5.4). Everything here is pure except
//! `terminal` and `event_loop`.

pub mod action;
pub mod app;
pub mod event_loop;
pub mod input;
pub mod tabs;
pub mod terminal;
pub mod theme;
pub mod widgets;

use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub use app::App;

/// Run the interface until the user quits. The terminal is restored on
/// return, on error and on panic.
pub fn run(mut app: App) -> io::Result<()> {
    terminal::install_panic_hook();
    let guard = terminal::TerminalGuard::enter(terminal::CrosstermOps)?;
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = event_loop::run_loop(&mut app, &mut term);
    guard.restore();
    result
}

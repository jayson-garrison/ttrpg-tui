//! Terminal lifecycle (A§5.4, T7_T9_PLAN §4): raw mode and the alternate
//! screen are always undone, on normal exit, on an error return and on panic.

use std::cell::Cell;
use std::io;

/// The few terminal operations the guard needs; a mock verifies restore order.
pub trait TerminalOps {
    fn enable_raw_mode(&self) -> io::Result<()>;
    fn disable_raw_mode(&self) -> io::Result<()>;
    fn enter_alt_screen(&self) -> io::Result<()>;
    fn leave_alt_screen(&self) -> io::Result<()>;
    fn show_cursor(&self) -> io::Result<()>;
}

/// The real terminal, through crossterm.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrosstermOps;

impl TerminalOps for CrosstermOps {
    fn enable_raw_mode(&self) -> io::Result<()> {
        crossterm::terminal::enable_raw_mode()
    }
    fn disable_raw_mode(&self) -> io::Result<()> {
        crossterm::terminal::disable_raw_mode()
    }
    fn enter_alt_screen(&self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)
    }
    fn leave_alt_screen(&self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)
    }
    fn show_cursor(&self) -> io::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::cursor::Show)
    }
}

/// Undo everything, ignoring failures: there is nothing better to do while
/// already leaving or panicking.
fn restore_all(ops: &dyn TerminalOps) {
    let _ = ops.disable_raw_mode();
    let _ = ops.leave_alt_screen();
    let _ = ops.show_cursor();
}

/// Restores the terminal when dropped. `restore` is idempotent.
pub struct TerminalGuard<O: TerminalOps> {
    ops: O,
    restored: Cell<bool>,
}

impl<O: TerminalOps> TerminalGuard<O> {
    pub fn enter(ops: O) -> io::Result<TerminalGuard<O>> {
        ops.enable_raw_mode()?;
        let guard = TerminalGuard {
            ops,
            restored: Cell::new(false),
        };
        // If this fails, dropping `guard` undoes raw mode.
        guard.ops.enter_alt_screen()?;
        Ok(guard)
    }

    pub fn restore(&self) {
        if !self.restored.replace(true) {
            restore_all(&self.ops);
        }
    }
}

impl<O: TerminalOps> Drop for TerminalGuard<O> {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Restore first, then the earlier hook, so the panic message prints on the
/// normal screen.
pub fn run_hook(restore: impl FnOnce(), previous: impl FnOnce()) {
    restore();
    previous();
}

/// Chain a restoring panic hook in front of the current one. Install it
/// before `TerminalGuard::enter`.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        run_hook(|| restore_all(&CrosstermOps), || previous(info));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Mock {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail_alt: bool,
    }

    impl Mock {
        fn log(&self, what: &'static str) -> io::Result<()> {
            self.calls.borrow_mut().push(what);
            Ok(())
        }
        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }

    impl TerminalOps for Mock {
        fn enable_raw_mode(&self) -> io::Result<()> {
            self.log("raw_on")
        }
        fn disable_raw_mode(&self) -> io::Result<()> {
            self.log("raw_off")
        }
        fn enter_alt_screen(&self) -> io::Result<()> {
            if self.fail_alt {
                return Err(io::Error::other("no alt screen"));
            }
            self.log("alt_on")
        }
        fn leave_alt_screen(&self) -> io::Result<()> {
            self.log("alt_off")
        }
        fn show_cursor(&self) -> io::Result<()> {
            self.log("cursor")
        }
    }

    const RESTORE: [&str; 3] = ["raw_off", "alt_off", "cursor"];

    #[test]
    fn normal_drop_restores_once() {
        let mock = Mock::default();
        {
            let _g = TerminalGuard::enter(mock.clone()).unwrap();
        }
        assert_eq!(
            mock.calls(),
            ["raw_on", "alt_on", "raw_off", "alt_off", "cursor"]
        );
    }

    #[test]
    fn restoring_twice_is_harmless() {
        let mock = Mock::default();
        let g = TerminalGuard::enter(mock.clone()).unwrap();
        g.restore();
        g.restore();
        drop(g);
        let restores = mock.calls().iter().filter(|c| **c == "raw_off").count();
        assert_eq!(restores, 1);
    }

    #[test]
    fn restores_on_unwind() {
        let mock = Mock::default();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _g = TerminalGuard::enter(mock.clone()).unwrap();
            let boom: Option<u8> = None;
            #[allow(clippy::panic)]
            if boom.is_none() {
                panic!("boom");
            }
        }));
        assert!(result.is_err());
        assert_eq!(&mock.calls()[2..], RESTORE);
    }

    #[test]
    fn a_failed_enter_leaves_raw_mode_off() {
        let mock = Mock {
            fail_alt: true,
            ..Mock::default()
        };
        assert!(TerminalGuard::enter(mock.clone()).is_err());
        assert_eq!(mock.calls(), ["raw_on", "raw_off", "alt_off", "cursor"]);
    }

    #[test]
    fn hook_restores_before_the_previous_hook_runs() {
        let order = RefCell::new(Vec::new());
        run_hook(
            || order.borrow_mut().push("restore"),
            || order.borrow_mut().push("previous"),
        );
        assert_eq!(*order.borrow(), ["restore", "previous"]);
    }
}

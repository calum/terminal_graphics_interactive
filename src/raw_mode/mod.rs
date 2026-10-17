//! Raw terminal mode: input arrives immediately, unechoed, as escape sequences.

mod modes;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

#[cfg(not(any(unix, windows)))]
mod sys {
    pub(super) struct Saved;
    pub(super) fn enable() -> std::io::Result<Saved> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "raw terminal input is only supported on Unix and Windows",
        ))
    }
    pub(super) fn restore(_: &Saved) {}
}

use std::io;

pub use modes::{MOUSE_OFF, MOUSE_ON};

/// Puts the terminal into raw mode until it is dropped.
///
/// In raw mode each key press and mouse event is delivered to the program
/// immediately as bytes (feed them to a [`Parser`](crate::Parser)), nothing is
/// echoed to the screen, and Ctrl+C arrives as a key (`Char('c')` with `ctrl`
/// set) instead of stopping the program, so your program must handle it.
/// Dropping the guard, including while a panic unwinds, restores the terminal.
///
/// Raw mode also stops the terminal turning `\n` into a new line at the left
/// edge on Unix, so use `\r\n` if you print lines yourself. `Display` output
/// is unaffected.
///
/// This works in Linux and macOS terminals, Windows PowerShell, Command Prompt
/// and Windows Terminal, and over SSH when a terminal was allocated (`ssh -t`).
/// Without a terminal, [`RawMode::enable`] returns an error saying so.
///
/// ```no_run
/// use terminal_graphics_interactive::RawMode;
///
/// let _raw = RawMode::enable()?;
/// // read stdin, parse events ...
/// // the terminal is restored when `_raw` goes out of scope
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct RawMode {
    saved: sys::Saved,
}

impl RawMode {
    /// Enters raw mode. Fails with [`io::ErrorKind::Unsupported`] and a message
    /// saying what to do if stdin is not a terminal.
    pub fn enable() -> io::Result<RawMode> {
        Ok(RawMode {
            saved: sys::enable()?,
        })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        sys::restore(&self.saved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::IsTerminal;

    #[test]
    fn enable_without_a_terminal_explains_what_to_do() {
        // under `cargo test` in a real terminal stdin is that terminal, so only
        // check the error path when it is not
        if io::stdin().is_terminal() {
            return;
        }
        let error = RawMode::enable()
            .err()
            .expect("must fail without a terminal");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("ssh -t"));
    }
}

//! `Terminal`: raw mode, mouse reporting and a background reader, in one guard.

use crate::event::Event;
use crate::input::{Input, MAX_TIMEOUT};
use crate::raw_mode::{RawMode, MOUSE_OFF, MOUSE_ON};
use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

const HIDE_CURSOR: &str = "\x1b[?25l";
const SHOW_CURSOR: &str = "\x1b[?25h";

/// What is printed when a session starts: mouse reporting on, cursor hidden.
fn start_sequence() -> String {
    format!("{MOUSE_ON}{HIDE_CURSOR}")
}

/// What is printed when a session ends: undo everything `start_sequence` did.
fn stop_sequence() -> String {
    format!("{MOUSE_OFF}{SHOW_CURSOR}")
}

/// Reads `reader` on a background thread and sends each chunk it gets down the
/// returned channel. The channel closes when the reader ends or fails, and the
/// thread ends when the receiver is dropped.
fn spawn_reader(mut reader: impl Read + Send + 'static) -> io::Result<Receiver<Vec<u8>>> {
    let (tx, rx) = channel();
    thread::Builder::new()
        .name("terminal_graphics_interactive-input".into())
        .spawn(move || {
            let mut buffer = [0u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        })?;
    Ok(rx)
}

/// An interactive terminal session: keyboard and mouse input for a program
/// that draws with [`Display`](terminal_graphics::Display).
///
/// [`Terminal::start`] puts the terminal in raw mode, asks it to report the
/// mouse, hides the cursor, and starts reading input in the background.
/// Dropping the `Terminal` undoes all of it (including during a panic), so
/// the shell is left as it was found.
///
/// It works in Linux and macOS terminals, Windows PowerShell and Windows
/// Terminal, and over SSH when a terminal was allocated (`ssh -t`). Without a
/// terminal, [`Terminal::start`] returns an error that says so.
///
/// Ctrl+C does not stop the program while a `Terminal` exists. It arrives as
/// an event (`Key` with `ctrl` set and `Char('c')`), so check for it and exit
/// your loop, or the only way out is closing the window.
///
/// Use `\r\n`, not `\n`, if you print lines yourself during a session.
///
/// ```no_run
/// use std::time::Duration;
/// use terminal_graphics_interactive::{Event, Terminal};
///
/// let mut terminal = Terminal::start()?;
/// loop {
///     // wait up to 50 ms for input, then do a frame of animation
///     if let Some(Event::Key(key)) = terminal.poll(Duration::from_millis(50))? {
///         if key.is_char('q') || key.is_ctrl('c') {
///             break;
///         }
///     }
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct Terminal {
    input: Input,
    /// Declared last so the terminal is restored after the sequences below are printed.
    _raw: RawMode,
}

impl Terminal {
    /// Starts an interactive session. See the [type documentation](Terminal).
    pub fn start() -> io::Result<Terminal> {
        let raw = RawMode::enable()?;
        let mut out = io::stdout();
        out.write_all(start_sequence().as_bytes())?;
        out.flush()?;
        let chunks = spawn_reader(io::stdin())?;
        Ok(Terminal {
            input: Input::new(chunks),
            _raw: raw,
        })
    }

    /// Waits up to `timeout` for the next event. `Ok(None)` means nothing
    /// happened in that time, which is the cue to draw the next animation
    /// frame. `Duration::ZERO` checks without waiting.
    ///
    /// Returns an error if the input has ended (for example stdin was closed).
    pub fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.input.poll(timeout)
    }

    /// Waits for the next event, as long as it takes.
    pub fn wait(&mut self) -> io::Result<Event> {
        loop {
            if let Some(event) = self.input.poll(MAX_TIMEOUT)? {
                return Ok(event);
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let mut out = io::stdout();
        // nothing useful can be done if the terminal is already gone
        let _ = out.write_all(stop_sequence().as_bytes());
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::Duration;

    #[test]
    fn start_turns_on_mouse_reporting_and_hides_the_cursor() {
        let start = start_sequence();
        assert!(start.contains(MOUSE_ON));
        assert!(start.contains(HIDE_CURSOR));
    }

    #[test]
    fn stop_turns_mouse_reporting_off_and_shows_the_cursor_again() {
        let stop = stop_sequence();
        assert!(stop.contains(MOUSE_OFF));
        assert!(stop.contains(SHOW_CURSOR));
    }

    #[test]
    fn the_reader_sends_what_it_reads_then_closes_the_channel() {
        let rx = spawn_reader(Cursor::new(b"hello".to_vec())).unwrap();
        let mut all = Vec::new();
        while let Ok(chunk) = rx.recv_timeout(Duration::from_secs(2)) {
            assert!(!chunk.is_empty(), "empty chunks must not be sent");
            all.extend(chunk);
        }
        assert_eq!(all, b"hello");
        // closed, not just quiet
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn the_reader_stops_when_the_reader_fails() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::Other, "boom"))
            }
        }
        let rx = spawn_reader(Broken).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn the_reader_stops_when_the_receiver_is_dropped() {
        // an endless reader: the thread must notice nobody is listening
        struct Endless;
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                buf[0] = b'x';
                Ok(1)
            }
        }
        let rx = spawn_reader(Endless).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
        drop(rx);
        // nothing to assert beyond not hanging the test process: the thread
        // exits on its next failed send
    }
}

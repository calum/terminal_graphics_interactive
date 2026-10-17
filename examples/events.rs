//! Prints every key and mouse event it receives. Press `q` or Ctrl+C to quit.
//!
//! Run it in a terminal (PowerShell, Windows Terminal, Linux, macOS, or over
//! `ssh -t`): `cargo run --example events`. Click, drag and scroll in the window.

use std::io::{self, Write};
use terminal_graphics_interactive::{Event, Terminal};

fn main() -> io::Result<()> {
    let mut terminal = Terminal::start()?;
    let mut out = io::stdout();
    // `\r\n` because raw mode turns off the terminal's own newline handling
    let (width, height) = terminal.size();
    write!(
        out,
        "Move, click, drag, scroll or type. q or Ctrl+C quits.\r\nsize: {width}x{height}\r\n"
    )?;
    out.flush()?;

    loop {
        let event = terminal.wait()?;
        write!(out, "{event:?}\r\n")?;
        out.flush()?;
        if let Event::Key(key) = event {
            if key.is_char('q') || key.is_ctrl('c') {
                return Ok(());
            }
        }
    }
}

//! Prints every key and mouse event it receives. Press `q` or Ctrl+C to quit.
//!
//! Run it in a terminal (PowerShell, Windows Terminal, Linux, macOS, or over
//! `ssh -t`): `cargo run --example events`. Click, drag and scroll in the window.

use std::io::{self, Read, Write};
use terminal_graphics_interactive::{Event, Parser, RawMode, MOUSE_OFF, MOUSE_ON};

fn main() -> io::Result<()> {
    let raw = RawMode::enable()?;
    let mut out = io::stdout();
    // `\r\n` because raw mode turns off the terminal's own newline handling
    write!(
        out,
        "{MOUSE_ON}Move, click, drag, scroll or type. q or Ctrl+C quits.\r\n"
    )?;
    out.flush()?;

    let result = run(&mut out);

    write!(out, "{MOUSE_OFF}")?;
    out.flush()?;
    drop(raw);
    result
}

fn run(out: &mut impl Write) -> io::Result<()> {
    let mut parser = Parser::new();
    let mut stdin = io::stdin().lock();
    let mut bytes = [0u8; 256];

    loop {
        let read = stdin.read(&mut bytes)?;
        if read == 0 {
            return Ok(());
        }
        parser.feed(&bytes[..read]);

        // a read returns whatever has arrived, so a lone Escape byte is the
        // Escape key and not the start of a longer sequence
        let mut events = Vec::new();
        while let Some(event) = parser.next_event() {
            events.push(event);
        }
        events.extend(parser.flush());

        for event in events {
            write!(out, "{event:?}\r\n")?;
            if let Event::Key(key) = event {
                if key.is_char('q') || key.is_ctrl('c') {
                    return Ok(());
                }
            }
        }
        out.flush()?;
    }
}

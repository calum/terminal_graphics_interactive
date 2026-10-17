use std::time::Duration;
use terminal_graphics_interactive::{Colour, Display, Event, MouseKind, Terminal};

fn main() -> std::io::Result<()> {
    let mut terminal = Terminal::start()?;

    // `draw` needs a display at most one row shorter than the window
    let (width, height) = terminal.size();
    let mut display = Display::new(width, height.saturating_sub(1));

    loop {
        terminal.draw(&display)?;

        // wait up to 50 ms for input; `None` means a frame passed with no input
        match terminal.poll(Duration::from_millis(50))? {
            Some(Event::Key(key)) if key.is_char('q') || key.is_ctrl('c') => return Ok(()),
            Some(Event::Mouse(mouse)) if matches!(mouse.kind, MouseKind::Down(_)) => {
                if let Some((x, y)) = mouse.cell_in(&display) {
                    display.set_pixel(x, y, 'X', Colour::BrightYellow, Colour::Blue);
                }
            }
            _ => {}
        }
    }
}

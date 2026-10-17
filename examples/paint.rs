//! A tiny paint program. Drag with the left mouse button to paint, drag with
//! the right button to erase, press 1-8 or scroll to pick a colour, `c` to
//! clear and `q` (or Ctrl+C) to quit.
//!
//! Run it in a terminal that reports the mouse (PowerShell, Windows Terminal,
//! Linux, macOS, or over `ssh -t`): `cargo run --example paint`.

use std::io;
use std::time::Duration;
use terminal_graphics_interactive::{
    Colour, Display, Event, Graphic, KeyCode, MouseButton, MouseKind, Terminal, Text,
};

/// The colours on offer, picked with the keys 1 to 8.
const PALETTE: [Colour; 8] = [
    Colour::White,
    Colour::Red,
    Colour::Yellow,
    Colour::Green,
    Colour::Cyan,
    Colour::Blue,
    Colour::Magenta,
    Colour::BrightBlack,
];

/// The cells from `from` to `to` inclusive, in a straight line.
///
/// A fast mouse drag jumps several cells between events, so painting only the
/// reported cells would leave gaps.
fn line(from: (isize, isize), to: (isize, isize)) -> Vec<(isize, isize)> {
    // Bresenham's algorithm
    let (mut x, mut y) = from;
    let dx = (to.0 - x).abs();
    let dy = -(to.1 - y).abs();
    let step_x = if x < to.0 { 1 } else { -1 };
    let step_y = if y < to.1 { 1 } else { -1 };
    let mut error = dx + dy;

    let mut cells = Vec::new();
    loop {
        cells.push((x, y));
        if (x, y) == to {
            return cells;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
    }
}

struct Paint {
    /// The canvas, with a status line on its last row.
    display: Display,
    /// Index into `PALETTE`.
    brush: usize,
    /// Where the pointer was at the last paint event, while a button is held.
    last: Option<(isize, isize)>,
}

impl Paint {
    fn new(width: u32, height: u32) -> Paint {
        let mut paint = Paint {
            display: Display::new(width, height),
            brush: 0,
            last: None,
        };
        paint.clear();
        paint
    }

    /// Blanks the canvas and redraws the status line.
    fn clear(&mut self) {
        for y in 0..self.canvas_height() {
            for x in 0..self.display.width() as isize {
                self.display
                    .set_pixel(x, y, ' ', Colour::White, Colour::Black);
            }
        }
        self.draw_status();
    }

    /// The last row of the display is the status line, not canvas.
    fn canvas_height(&self) -> isize {
        self.display.height() as isize - 1
    }

    /// One swatch per colour (the chosen one marked `#`), then the controls.
    fn draw_status(&mut self) {
        let y = self.canvas_height();
        for x in 0..self.display.width() as isize {
            self.display
                .set_pixel(x, y, ' ', Colour::White, Colour::Black);
        }
        for (index, colour) in PALETTE.iter().enumerate() {
            let mark = if index == self.brush { '#' } else { ' ' };
            self.display
                .set_pixel(index as isize, y, mark, Colour::Black, *colour);
        }
        let help = "1-8 or scroll: colour   drag: paint   right drag: erase   c: clear   q: quit";
        Text::new(
            PALETTE.len() as isize + 1,
            y,
            help,
            Colour::White,
            Colour::Black,
        )
        .draw(&mut self.display);
    }

    /// Paints (or erases, with black) from where the pointer last was to `(x, y)`.
    fn stroke(&mut self, x: isize, y: isize, colour: Colour) {
        let width = self.display.width() as isize;
        let height = self.canvas_height();

        // Keep the line walk short and free of overflow however wild the
        // coordinates are: one cell outside the canvas is as good as a mile.
        let clamp = |(x, y): (isize, isize)| (x.clamp(-1, width), y.clamp(-1, height));
        let from = self.last.unwrap_or((x, y));
        for (cx, cy) in line(clamp(from), clamp((x, y))) {
            if (0..width).contains(&cx) && (0..height).contains(&cy) {
                self.display.set_pixel(cx, cy, ' ', Colour::White, colour);
            }
        }
        self.last = Some((x, y));
    }

    /// A button press starts a new stroke. The release that ended the last one
    /// may never arrive (let go outside the window), and without this the next
    /// click would draw a line from wherever the old stroke stopped.
    fn begin_stroke_if_pressed(&mut self, kind: MouseKind) {
        if matches!(kind, MouseKind::Down(_)) {
            self.last = None;
        }
    }

    /// Reacts to one event. Returns `false` when the program should quit.
    fn handle(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(key) => {
                if key.is_char('q') || key.is_ctrl('c') {
                    return false;
                }
                match key.code {
                    KeyCode::Char(c @ '1'..='8') if key.modifiers.is_none() => {
                        self.brush = c as usize - '1' as usize;
                        self.draw_status();
                    }
                    KeyCode::Char('c') if key.modifiers.is_none() => self.clear(),
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseKind::Down(MouseButton::Left) | MouseKind::Drag(MouseButton::Left) => {
                    self.begin_stroke_if_pressed(mouse.kind);
                    self.stroke(mouse.x, mouse.y, PALETTE[self.brush]);
                }
                MouseKind::Down(MouseButton::Right) | MouseKind::Drag(MouseButton::Right) => {
                    self.begin_stroke_if_pressed(mouse.kind);
                    self.stroke(mouse.x, mouse.y, Colour::Black);
                }
                MouseKind::Up(_) => self.last = None,
                MouseKind::ScrollUp => {
                    self.brush = (self.brush + PALETTE.len() - 1) % PALETTE.len();
                    self.draw_status();
                }
                MouseKind::ScrollDown => {
                    self.brush = (self.brush + 1) % PALETTE.len();
                    self.draw_status();
                }
                _ => {}
            },
            // Pasted text is not typing: ignore it, so pasting `cargo run q` can
            // neither clear the canvas nor quit. More kinds of event may come.
            _ => {}
        }
        true
    }
}

fn main() -> io::Result<()> {
    let mut terminal = Terminal::start()?;

    // one row shorter than the window, which `Terminal::draw` needs
    let (width, height) = terminal.size();
    let mut paint = Paint::new(width, height.saturating_sub(1).max(2));

    loop {
        terminal.draw(&paint.display)?;

        // handle everything that has arrived before drawing the next frame
        let mut event = terminal.wait()?;
        loop {
            if !paint.handle(&event) {
                return Ok(());
            }
            match terminal.poll(Duration::ZERO)? {
                Some(next) => event = next,
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics_interactive::{Key, Modifiers, Mouse};

    fn key(c: char) -> Event {
        Event::Key(Key::new(KeyCode::Char(c)))
    }

    fn mouse(kind: MouseKind, x: isize, y: isize) -> Event {
        Event::Mouse(Mouse {
            kind,
            x,
            y,
            modifiers: Modifiers::NONE,
        })
    }

    fn down(x: isize, y: isize) -> Event {
        mouse(MouseKind::Down(MouseButton::Left), x, y)
    }

    fn drag(x: isize, y: isize) -> Event {
        mouse(MouseKind::Drag(MouseButton::Left), x, y)
    }

    fn up(x: isize, y: isize) -> Event {
        mouse(MouseKind::Up(MouseButton::Left), x, y)
    }

    fn background(paint: &Paint, x: isize, y: isize) -> Colour {
        paint.display.pixel(x, y).unwrap().background()
    }

    #[test]
    fn a_line_between_two_cells_includes_both_ends() {
        assert_eq!(line((0, 0), (3, 0)), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(line((3, 0), (0, 0)), vec![(3, 0), (2, 0), (1, 0), (0, 0)]);
        assert_eq!(line((2, 2), (2, 4)), vec![(2, 2), (2, 3), (2, 4)]);
    }

    #[test]
    fn a_line_from_a_cell_to_itself_is_that_cell() {
        assert_eq!(line((5, 5), (5, 5)), vec![(5, 5)]);
    }

    #[test]
    fn a_diagonal_line_has_one_cell_per_step() {
        assert_eq!(line((0, 0), (3, 3)), vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
    }

    #[test]
    fn a_shallow_line_has_no_gaps() {
        let cells = line((0, 0), (10, 3));
        assert_eq!(cells.len(), 11);
        assert_eq!(cells.first(), Some(&(0, 0)));
        assert_eq!(cells.last(), Some(&(10, 3)));
        for pair in cells.windows(2) {
            assert!((pair[1].0 - pair[0].0).abs() <= 1 && (pair[1].1 - pair[0].1).abs() <= 1);
        }
    }

    #[test]
    fn the_canvas_starts_black() {
        let paint = Paint::new(20, 6);
        assert_eq!(background(&paint, 3, 2), Colour::Black);
    }

    #[test]
    fn a_left_click_paints_the_cell_in_the_brush_colour() {
        let mut paint = Paint::new(20, 6);
        assert!(paint.handle(&down(4, 2)));
        assert_eq!(background(&paint, 4, 2), PALETTE[0]);
        assert_eq!(background(&paint, 5, 2), Colour::Black);
    }

    #[test]
    fn number_keys_pick_the_brush() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&key('2'));
        paint.handle(&down(1, 1));
        assert_eq!(background(&paint, 1, 1), PALETTE[1]);
        paint.handle(&key('8'));
        paint.handle(&down(2, 1));
        assert_eq!(background(&paint, 2, 1), PALETTE[7]);
    }

    #[test]
    fn scrolling_cycles_through_the_colours_both_ways() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&mouse(MouseKind::ScrollDown, 0, 0));
        assert_eq!(paint.brush, 1);
        paint.handle(&mouse(MouseKind::ScrollUp, 0, 0));
        paint.handle(&mouse(MouseKind::ScrollUp, 0, 0));
        assert_eq!(paint.brush, PALETTE.len() - 1);
    }

    #[test]
    fn dragging_fills_the_gap_between_reported_positions() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&key('2'));
        paint.handle(&down(0, 0));
        paint.handle(&drag(6, 0));
        for x in 0..=6 {
            assert_eq!(background(&paint, x, 0), PALETTE[1], "x = {x}");
        }
        assert_eq!(background(&paint, 7, 0), Colour::Black);
    }

    #[test]
    fn a_stroke_with_extreme_coordinates_neither_hangs_nor_panics() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&down(2, 2));
        paint.handle(&drag(isize::MAX, isize::MIN));
        paint.handle(&drag(isize::MIN, isize::MAX));
        paint.handle(&down(isize::MAX, isize::MAX));
    }

    #[test]
    fn a_stroke_that_leaves_and_comes_back_paints_only_the_canvas_part() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&key('2'));
        paint.handle(&down(2, 2));
        paint.handle(&drag(-40, 2));
        assert_eq!(background(&paint, 0, 2), PALETTE[1]);
        assert_eq!(background(&paint, 2, 2), PALETTE[1]);
    }

    #[test]
    fn releasing_the_button_starts_a_new_stroke() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&down(0, 0));
        paint.handle(&up(0, 0));
        paint.handle(&down(8, 0));
        assert_eq!(
            background(&paint, 4, 0),
            Colour::Black,
            "no line between strokes"
        );
    }

    #[test]
    fn the_right_button_erases() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&down(3, 3));
        paint.handle(&mouse(MouseKind::Down(MouseButton::Right), 3, 3));
        assert_eq!(background(&paint, 3, 3), Colour::Black);
    }

    #[test]
    fn painting_outside_the_canvas_or_on_the_status_line_does_nothing() {
        let mut paint = Paint::new(20, 6);
        let before = paint.display.clone();
        paint.handle(&down(-1, 2));
        paint.handle(&down(20, 2));
        paint.handle(&down(2, 99));
        assert_eq!(paint.display, before);
        // the status line is the last row, y = 5
        paint.handle(&down(15, 5));
        assert_ne!(background(&paint, 15, 5), PALETTE[0]);
    }

    #[test]
    fn a_stroke_that_leaves_the_canvas_is_clipped_without_panicking() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&down(2, 2));
        paint.handle(&drag(500, -500));
    }

    #[test]
    fn pasted_text_is_ignored_so_it_cannot_trigger_commands() {
        // a right click often pastes the clipboard, and `cargo run ...` contains
        // a `c`, which would otherwise clear the canvas, and a `q`, which quits
        let mut paint = Paint::new(20, 6);
        paint.handle(&key('3'));
        paint.handle(&down(3, 3));
        let before = paint.display.clone();

        assert!(paint.handle(&Event::Paste("cargo run --example paint q 7 c".into())));

        assert_eq!(paint.display, before);
        assert_eq!(paint.brush, 2);
    }

    #[test]
    fn c_clears_the_canvas() {
        let mut paint = Paint::new(20, 6);
        paint.handle(&down(3, 3));
        paint.handle(&key('c'));
        assert_eq!(background(&paint, 3, 3), Colour::Black);
    }

    #[test]
    fn q_and_ctrl_c_quit_and_other_events_do_not() {
        let mut paint = Paint::new(20, 6);
        assert!(!paint.handle(&key('q')));
        let ctrl_c = Event::Key(Key {
            code: KeyCode::Char('c'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        });
        assert!(!paint.handle(&ctrl_c));
        assert!(paint.handle(&key('x')));
    }

    #[test]
    fn the_status_line_shows_the_selected_colour() {
        let mut paint = Paint::new(40, 6);
        paint.handle(&key('3'));
        let y = paint.display.height() as isize - 1;
        assert_eq!(paint.display.pixel(2, y).unwrap().character(), '#');
        assert_eq!(paint.display.pixel(1, y).unwrap().character(), ' ');
    }

    #[test]
    fn a_tiny_display_does_not_panic() {
        let mut paint = Paint::new(1, 2);
        paint.handle(&down(0, 0));
        paint.handle(&key('8'));
        paint.handle(&key('c'));
        let _ = Text::new(0, 0, "x", Colour::White, Colour::Black).contains(0, 0);
    }
}

//! `Renderer`: writes only the cells of a [`Display`] that changed since the
//! last frame, so a steady picture costs no bytes and a small change costs few.

use std::io::{self, Write};
use terminal_graphics::{Colour, Display, Pixel};

/// Up to this many unchanged cells between two changes are written again rather
/// than skipped with a cursor move.
const MAX_GAP: u32 = 3;

/// Writes frames to any [`Write`], sending only the cells that changed since
/// the last frame. The first frame clears the screen and draws everything;
/// after that a frame costs only the cells that differ, and an unchanged
/// frame costs nothing.
///
/// [`Terminal::draw`](crate::Terminal::draw) uses one of these on stdout. Use
/// it directly to send frames somewhere else, such as a file or a socket, or
/// to test what a frame looks like as bytes.
///
/// Positions are absolute (`ESC[row;colH`), so the output does not depend on
/// the cursor's position before the frame. Each frame is written with one
/// `write_all` call and ends with a reset.
///
/// ```
/// use terminal_graphics::{Colour, Display};
/// use terminal_graphics_interactive::Renderer;
///
/// let mut renderer = Renderer::new();
/// let mut display = Display::new(4, 1);
/// let mut screen = Vec::new();
///
/// renderer.draw_to(&display, &mut screen)?;   // the first frame draws everything
/// screen.clear();
/// display.set_pixel(2, 0, 'x', Colour::Red, Colour::Blue);
/// renderer.draw_to(&display, &mut screen)?;   // only the changed cell is sent
/// assert_eq!(screen, b"\x1b[1;3H\x1b[31;44mx\x1b[0m");
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Debug)]
pub struct Renderer {
    previous: Vec<Pixel>,
    size: (u32, u32),
    valid: bool,
    buffer: Vec<u8>,
}

impl Renderer {
    /// A renderer that has not drawn anything yet, so its first frame draws
    /// every cell.
    ///
    /// ```
    /// use terminal_graphics::Display;
    /// use terminal_graphics_interactive::Renderer;
    ///
    /// let mut renderer = Renderer::new();
    /// let mut out = Vec::new();
    /// let written = renderer.draw_to(&Display::new(2, 1), &mut out)?;
    /// assert_eq!(written, out.len());
    /// assert!(out.starts_with(b"\x1b[0m\x1b[2J"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn new() -> Renderer {
        Renderer {
            previous: Vec::new(),
            size: (0, 0),
            valid: false,
            buffer: Vec::new(),
        }
    }

    /// The next frame is drawn in full, even if nothing changed. Call it
    /// when the screen may show something other than the last frame, for
    /// example after printing text on top of the picture.
    ///
    /// ```
    /// use terminal_graphics::Display;
    /// use terminal_graphics_interactive::Renderer;
    ///
    /// let mut renderer = Renderer::new();
    /// let display = Display::new(2, 1);
    /// renderer.draw_to(&display, &mut Vec::new())?;
    /// renderer.invalidate();
    ///
    /// let mut out = Vec::new();
    /// renderer.draw_to(&display, &mut out)?;
    /// assert!(out.starts_with(b"\x1b[0m\x1b[2J"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Writes the cells of `display` that changed since the last call to
    /// `out`, and returns how many bytes were written.
    ///
    /// The first call, the first call after [`Renderer::invalidate`], and a
    /// call after the size changed write the whole display, starting with a
    /// clear. A frame with no changes writes nothing and returns `Ok(0)`. A
    /// display with no width or no height writes nothing.
    ///
    /// Nothing is flushed. Call `flush` on `out` if it needs it (stdout does).
    ///
    /// ```
    /// use terminal_graphics::{Colour, Display};
    /// use terminal_graphics_interactive::Renderer;
    ///
    /// let mut renderer = Renderer::new();
    /// let mut display = Display::new(10, 3);
    /// let mut out = Vec::new();
    ///
    /// renderer.draw_to(&display, &mut out)?;
    /// out.clear();
    ///
    /// // one frame of a game: only the cell that moved is sent
    /// display.set_pixel(4, 1, '@', Colour::Yellow, Colour::Black);
    /// let written = renderer.draw_to(&display, &mut out)?;
    /// assert_eq!(written, out.len());
    /// assert_eq!(out, b"\x1b[2;5H\x1b[33;40m@\x1b[0m");
    ///
    /// // nothing changed: nothing is written
    /// assert_eq!(renderer.draw_to(&display, &mut out)?, 0);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn draw_to<W: Write>(&mut self, display: &Display, out: &mut W) -> io::Result<usize> {
        let (width, height) = display.size();
        if width == 0 || height == 0 {
            return Ok(0);
        }
        let full = !self.valid || self.size != (width, height);
        self.buffer.clear();
        if full {
            if self.size != (width, height) {
                self.previous.clear();
                self.previous
                    .resize(width as usize * height as usize, Pixel::default());
                self.size = (width, height);
            }
            self.buffer.extend_from_slice(b"\x1b[0m\x1b[2J");
            self.valid = true;
        }

        // the colours the terminal has been told to use for this write
        let mut colours: Option<(Colour, Colour)> = None;
        for y in 0..height {
            // the column just after the last cell written on this row, if any
            let mut cursor: Option<u32> = None;
            for x in 0..width {
                let Some(pixel) = display.pixel(x as isize, y as isize) else {
                    continue;
                };
                let index = y as usize * width as usize + x as usize;
                if !full && pixel == &self.previous[index] {
                    continue;
                }
                self.previous[index].clone_from(pixel);

                // a short gap of unchanged cells is cheaper to rewrite than a cursor move
                let run_from = cursor.filter(|&column| column <= x && x - column <= MAX_GAP);
                match run_from {
                    Some(column) => {
                        for gap in column..x {
                            if let Some(cell) = display.pixel(gap as isize, y as isize) {
                                write_cell(&mut self.buffer, &mut colours, cell);
                            }
                        }
                    }
                    None => {
                        let _ = write!(self.buffer, "\x1b[{};{}H", y + 1, x + 1);
                    }
                }
                write_cell(&mut self.buffer, &mut colours, pixel);
                cursor = Some(x + 1);
            }
        }

        if self.buffer.is_empty() {
            return Ok(0);
        }
        self.buffer.extend_from_slice(b"\x1b[0m");
        out.write_all(&self.buffer)?;
        Ok(self.buffer.len())
    }
}

impl Default for Renderer {
    fn default() -> Renderer {
        Renderer::new()
    }
}

/// Writes one cell's colours (only if they differ from `colours`, the ones the
/// terminal is set to) and its character.
fn write_cell(buffer: &mut Vec<u8>, colours: &mut Option<(Colour, Colour)>, pixel: &Pixel) {
    let pair = (pixel.colour(), pixel.background());
    if *colours != Some(pair) {
        let (foreground, _) = pair.0.ansi_codes();
        let (_, background) = pair.1.ansi_codes();
        let _ = write!(buffer, "\x1b[{foreground};{background}m");
        *colours = Some(pair);
    }
    let mut utf8 = [0u8; 4];
    buffer.extend_from_slice(pixel.character().encode_utf8(&mut utf8).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws one frame and checks the returned count is the number of bytes written.
    fn draw(renderer: &mut Renderer, display: &Display) -> Vec<u8> {
        let mut out = Vec::new();
        let written = renderer.draw_to(display, &mut out).unwrap();
        assert_eq!(written, out.len());
        out
    }

    #[test]
    fn the_first_frame_clears_the_screen_then_draws_every_cell() {
        let mut renderer = Renderer::new();
        let display = Display::new(2, 1);
        let out = draw(&mut renderer, &display);
        assert!(out.starts_with(b"\x1b[0m\x1b[2J"), "{out:?}");
        assert!(out.ends_with(b"\x1b[0m"), "{out:?}");
        let mut screen = Screen::new(2, 1);
        screen.apply(&out);
        screen.assert_shows(&display, 0);
    }

    #[test]
    fn consecutive_changed_cells_in_a_row_share_one_cursor_move() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(4, 1);
        draw(&mut renderer, &display);
        display.set_pixel(0, 0, 'a', Colour::Green, Colour::Blue);
        display.set_pixel(1, 0, 'b', Colour::Green, Colour::Blue);
        display.set_pixel(2, 0, 'c', Colour::Red, Colour::Blue);
        let out = draw(&mut renderer, &display);
        // one move, the green pair shares one colour sequence, then a new colour
        assert_eq!(out, b"\x1b[1;1H\x1b[32;44mab\x1b[31;44mc\x1b[0m".to_vec());
    }

    #[test]
    fn a_changed_cell_at_the_end_of_a_row_and_one_at_the_start_of_the_next_moves_the_cursor() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(3, 2);
        draw(&mut renderer, &display);
        display.set_pixel(2, 0, 'a', Colour::Red, Colour::Blue);
        display.set_pixel(0, 1, 'b', Colour::Red, Colour::Blue);
        let out = draw(&mut renderer, &display);
        assert_eq!(out, b"\x1b[1;3H\x1b[31;44ma\x1b[2;1Hb\x1b[0m".to_vec());
    }

    #[test]
    fn changed_cells_four_apart_in_a_row_get_a_cursor_move_and_the_gap_is_not_rewritten() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(7, 1);
        draw(&mut renderer, &display);
        display.set_pixel(1, 0, 'a', Colour::Green, Colour::Blue);
        display.set_pixel(6, 0, 'b', Colour::Green, Colour::Blue);
        let out = draw(&mut renderer, &display);
        assert_eq!(out, b"\x1b[1;2H\x1b[32;44ma\x1b[1;7Hb\x1b[0m".to_vec());
    }

    #[test]
    fn a_display_with_no_width_or_no_height_writes_nothing() {
        let mut renderer = Renderer::new();
        assert!(draw(&mut renderer, &Display::new(0, 3)).is_empty());
        assert!(draw(&mut renderer, &Display::new(4, 0)).is_empty());
    }

    #[test]
    fn an_identical_second_frame_writes_nothing_at_all() {
        let mut renderer = Renderer::new();
        let display = Display::new(3, 2);
        draw(&mut renderer, &display);
        assert!(draw(&mut renderer, &display).is_empty());
    }

    #[test]
    fn invalidate_makes_the_next_identical_frame_a_full_redraw() {
        let mut renderer = Renderer::new();
        let display = Display::new(2, 1);
        draw(&mut renderer, &display);
        renderer.invalidate();
        let out = draw(&mut renderer, &display);
        assert!(out.starts_with(b"\x1b[0m\x1b[2J"), "{out:?}");
    }

    #[test]
    fn a_change_of_size_is_a_full_redraw() {
        let mut renderer = Renderer::new();
        draw(&mut renderer, &Display::new(2, 1));
        let out = draw(&mut renderer, &Display::new(3, 1));
        assert!(out.starts_with(b"\x1b[0m\x1b[2J"), "{out:?}");
    }

    #[test]
    fn a_gap_of_three_unchanged_cells_is_rewritten_rather_than_moving_the_cursor() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(6, 1);
        for x in 0..6 {
            display.set_pixel(x, 0, 'x', Colour::Red, Colour::Blue);
        }
        draw(&mut renderer, &display);
        display.set_pixel(1, 0, 'a', Colour::Green, Colour::Blue);
        display.set_pixel(5, 0, 'b', Colour::Green, Colour::Blue);
        let out = draw(&mut renderer, &display);
        // the three red cells between them are written again, in the same run
        assert_eq!(
            out,
            b"\x1b[1;2H\x1b[32;44ma\x1b[31;44mxxx\x1b[32;44mb\x1b[0m".to_vec()
        );
    }

    /// A simulated terminal screen. It understands only what the renderer
    /// writes: cursor positions, colour sequences, `ESC[2J` and reset, and
    /// printable characters. Anything else panics, so a new sequence is noticed.
    struct Screen {
        /// Each cell is `(character, foreground code, background code)`.
        rows: Vec<Vec<(char, u8, u8)>>,
        row: usize,
        column: usize,
        foreground: u8,
        background: u8,
    }

    const DEFAULT_FOREGROUND: u8 = 39;
    const DEFAULT_BACKGROUND: u8 = 49;
    const BLANK: (char, u8, u8) = (' ', DEFAULT_FOREGROUND, DEFAULT_BACKGROUND);

    impl Screen {
        fn new(width: usize, height: usize) -> Screen {
            Screen {
                rows: vec![vec![BLANK; width]; height],
                row: 0,
                column: 0,
                foreground: DEFAULT_FOREGROUND,
                background: DEFAULT_BACKGROUND,
            }
        }

        fn apply(&mut self, bytes: &[u8]) {
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == 0x1b {
                    assert_eq!(bytes.get(i + 1), Some(&b'['), "only CSI sequences");
                    let start = i + 2;
                    let length = bytes[start..]
                        .iter()
                        .position(|b| (0x40..=0x7e).contains(b))
                        .expect("a sequence that ends");
                    let end = start + length;
                    let params = std::str::from_utf8(&bytes[start..end]).unwrap();
                    self.control(params, bytes[end]);
                    i = end + 1;
                } else {
                    let length = match bytes[i] {
                        0..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let text = std::str::from_utf8(&bytes[i..i + length]).unwrap();
                    self.print(text.chars().next().unwrap());
                    i += length;
                }
            }
        }

        fn control(&mut self, params: &str, last: u8) {
            let numbers: Vec<u32> = if params.is_empty() {
                Vec::new()
            } else {
                params.split(';').map(|p| p.parse().unwrap()).collect()
            };
            match (last, numbers.as_slice()) {
                (b'H', []) => self.move_to(0, 0),
                (b'H', [row, column]) => self.move_to(*row as usize - 1, *column as usize - 1),
                (b'm', [] | [0]) => {
                    self.foreground = DEFAULT_FOREGROUND;
                    self.background = DEFAULT_BACKGROUND;
                }
                (b'm', [foreground, background]) => {
                    self.foreground = *foreground as u8;
                    self.background = *background as u8;
                }
                (b'J', [2]) => {
                    for row in &mut self.rows {
                        row.fill(BLANK);
                    }
                }
                _ => panic!("unexpected sequence ESC[{params}{}", last as char),
            }
        }

        fn move_to(&mut self, row: usize, column: usize) {
            assert!(row < self.rows.len(), "cursor row {row} is off the screen");
            assert!(
                column < self.rows[0].len(),
                "cursor column {column} is off the screen"
            );
            self.row = row;
            self.column = column;
        }

        fn print(&mut self, character: char) {
            assert!(
                self.column < self.rows[0].len(),
                "printed past the end of a row"
            );
            self.rows[self.row][self.column] = (character, self.foreground, self.background);
            self.column += 1;
        }

        /// Checks every cell against `display`.
        fn assert_shows(&self, display: &Display, frame: usize) {
            let (width, height) = display.size();
            assert_eq!(self.rows.len(), height as usize, "frame {frame}");
            for y in 0..height {
                for x in 0..width {
                    let pixel = display.pixel(x as isize, y as isize).unwrap();
                    let expected = (
                        pixel.character(),
                        pixel.colour().ansi_codes().0,
                        pixel.background().ansi_codes().1,
                    );
                    assert_eq!(
                        self.rows[y as usize][x as usize], expected,
                        "frame {frame}, cell ({x}, {y})"
                    );
                }
            }
        }
    }

    /// A tiny random number generator, so the tests need no crates.
    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    #[test]
    fn the_screen_shows_the_display_after_every_one_of_many_random_frames() {
        let (width, height) = (17u32, 9u32);
        let characters = [' ', '#', '.', 'o', 'x', '@', '*', '\u{2588}', '\u{2591}'];
        // percent of the cells that change in each frame: none, a few, many, all
        let rates = [0u64, 1, 5, 30, 100];
        let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
        let mut renderer = Renderer::new();
        let mut display = Display::new(width, height);
        let mut screen = Screen::new(width as usize, height as usize);

        for frame in 0..200 {
            let rate = rates[frame % rates.len()];
            for y in 0..height {
                for x in 0..width {
                    if rng.below(100) < rate {
                        let character = characters[rng.below(characters.len() as u64) as usize];
                        let colour = Colour::ALL[rng.below(16) as usize];
                        let background = Colour::ALL[rng.below(16) as usize];
                        display.set_pixel(x as isize, y as isize, character, colour, background);
                    }
                }
            }
            if frame == 120 {
                renderer.invalidate();
            }
            let out = draw(&mut renderer, &display);
            screen.apply(&out);
            screen.assert_shows(&display, frame);
        }
    }

    /// Counts how many `write` calls it receives, to check one write per frame.
    struct Counting {
        calls: usize,
        bytes: Vec<u8>,
    }

    impl Write for Counting {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_frame_reaches_the_writer_in_one_write_call() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(4, 2);
        let mut out = Counting {
            calls: 0,
            bytes: Vec::new(),
        };
        renderer.draw_to(&display, &mut out).unwrap();
        assert_eq!(out.calls, 1);
        display.set_pixel(0, 0, 'q', Colour::Red, Colour::Blue);
        display.set_pixel(3, 1, 'r', Colour::Red, Colour::Blue);
        renderer.draw_to(&display, &mut out).unwrap();
        assert_eq!(out.calls, 2, "a changed frame is one write");
        renderer.draw_to(&display, &mut out).unwrap();
        assert_eq!(out.calls, 2, "an unchanged frame writes nothing");
    }

    #[test]
    fn one_changed_cell_is_a_cursor_move_its_colours_and_a_reset() {
        let mut renderer = Renderer::new();
        let mut display = Display::new(4, 2);
        draw(&mut renderer, &display);
        display.set_pixel(2, 1, 'x', Colour::Red, Colour::Blue);
        let out = draw(&mut renderer, &display);
        // row 2, column 3 (1-based), red (31) on blue (44)
        assert_eq!(out, b"\x1b[2;3H\x1b[31;44mx\x1b[0m".to_vec());
    }
}

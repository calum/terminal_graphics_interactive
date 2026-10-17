//! The [`Screen`]: what a game draws on each frame.

use super::Sprite;
use crate::art::sketch_region;
use crate::{Cell, Palette, Pointer};
use terminal_graphics::{Colour, Display, Pixel};

/// The surface a game draws on: a [`Display`] plus helpers for text, fills,
/// bars and [`Sprite`](super::Sprite)s.
///
/// Every position is an `i32` so that things may be partly or fully off the
/// screen. Anything outside is clipped silently, so no drawing call panics.
///
/// ```
/// use terminal_graphics::Colour;
/// use terminal_graphics_interactive::game::*;
///
/// let mut screen = Screen::new(20, 5);
/// screen.clear(Colour::Blue);
/// screen.text_centred(2, "hello", Colour::White);
/// assert_eq!(screen.width(), 20);
/// assert_eq!(screen.display().pixel(9, 2).unwrap().character(), 'l');
/// ```
#[derive(Clone, Debug)]
pub struct Screen {
    display: Display,
    time: f32,
    tick: u64,
}

impl Screen {
    /// A screen `width` columns by `height` rows, initially the default
    /// [`Display`] contents. Use the terminal size for a full-screen game.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(80, 24);
    /// assert_eq!(screen.size(), (80, 24));
    /// ```
    pub fn new(width: u32, height: u32) -> Screen {
        Screen {
            display: Display::new(width, height),
            time: 0.0,
            tick: 0,
        }
    }

    /// Fills the whole screen with spaces on `background`. Call it first in
    /// `draw` when the background is plain colour.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(3, 2);
    /// screen.clear(Colour::Blue);
    /// assert_eq!(screen.display().pixel(2, 1).unwrap().background(), Colour::Blue);
    /// ```
    pub fn clear(&mut self, background: Colour) {
        let (width, height) = self.size();
        for y in 0..height as isize {
            for x in 0..width as isize {
                self.display.set_pixel(x, y, ' ', Colour::Black, background);
            }
        }
    }

    /// Fills the whole screen with a sketch formula, as the background of the
    /// frame. `formula` runs once per cell and returns -1.0 to 1.0, the same as
    /// [`sketch_frame`](crate::sketch_frame). The formula is given the screen
    /// [`time`](Screen::time), and there is no mouse. Call it first in `draw`,
    /// then draw sprites and text on top.
    ///
    /// A two-layer scene: a sky over the top half, a sea over the bottom half
    /// and a boat drawn on top of both.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// fn wave(x: f32, t: f32) -> f32 {
    ///     (x * 0.08 + t).sin() * 0.5 + (x * 0.19 - t * 1.3).sin() * 0.25
    /// }
    /// fn sea(cell: &Cell) -> f32 {
    ///     ((cell.height / 2.0 + wave(cell.x, cell.t) * 6.0 - cell.y) * 0.25).clamp(-1.0, 1.0)
    /// }
    ///
    /// let mut screen = Screen::new(40, 12);
    /// screen.backdrop_in(0, 0, 40, 6, &Palette::fire(), |cell| 1.0 - cell.y / cell.height);
    /// screen.backdrop_in(0, 6, 40, 6, &Palette::ocean(), sea);
    /// screen.put(18, 5, '^', Colour::White);
    /// assert_eq!(screen.display().pixel(18, 5).unwrap().character(), '^');
    /// assert_ne!(screen.display().pixel(0, 6).unwrap().character(), ' ');
    /// ```
    pub fn backdrop(&mut self, palette: &Palette, formula: impl FnMut(&Cell) -> f32) {
        let (width, height) = self.size();
        sketch_region(
            &mut self.display,
            (0, 0, width, height),
            palette,
            self.time,
            &Pointer::default(),
            formula,
        );
    }

    /// Fills the rectangle at `(x, y)` that is `w` by `h` with a sketch
    /// formula, such as a sky over the top half of the screen. The part off the
    /// screen is not drawn, and a negative size is an empty rectangle. Inside the
    /// formula, `cell.x` and `cell.y` count from the rectangle's top left and
    /// `cell.width` and `cell.height` are its size.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(10, 4);
    /// screen.backdrop_in(0, 0, 10, 2, &Palette::fire(), |_| 1.0);
    /// screen.backdrop_in(0, 2, 10, 2, &Palette::ocean(), |_| -1.0);
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().character(), '\u{2588}');
    /// assert_eq!(screen.display().pixel(0, 3).unwrap().character(), ' ');
    /// ```
    // the signature is the one the game API is documented with; a struct would add a concept
    #[allow(clippy::too_many_arguments)]
    pub fn backdrop_in(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        palette: &Palette,
        formula: impl FnMut(&Cell) -> f32,
    ) {
        // a negative size is an empty rectangle, so it becomes a size of zero
        let size = |n: i32| u32::try_from(n).unwrap_or(0);
        sketch_region(
            &mut self.display,
            (x, y, size(w), size(h)),
            palette,
            self.time,
            &Pointer::default(),
            formula,
        );
    }

    /// Draws `ch` in `colour` at column `x`, row `y`, keeping the background
    /// that is already there. Positions off the screen are ignored.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(3, 1);
    /// screen.put(1, 0, '@', Colour::Yellow);
    /// screen.put(-5, 0, '@', Colour::Yellow); // off screen: nothing happens
    /// assert_eq!(screen.display().pixel(1, 0).unwrap().character(), '@');
    /// ```
    pub fn put(&mut self, x: i32, y: i32, ch: char, colour: Colour) {
        if let Some(pixel) = self.cell_mut(i64::from(x), i64::from(y)) {
            pixel.set_character(ch);
            pixel.set_colour(colour);
        }
    }

    /// Fills a `w` by `h` rectangle whose top left corner is `(x, y)` with
    /// `ch` in `colour` on `background`. The part of the rectangle that is off
    /// the screen is not drawn, and a negative size fills nothing.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(4, 2);
    /// screen.fill(2, 0, 10, 1, '=', Colour::White, Colour::Black);
    /// assert_eq!(screen.display().pixel(3, 0).unwrap().character(), '=');
    /// assert_eq!(screen.display().pixel(3, 1).unwrap().character(), ' ');
    /// ```
    // the signature is the one the game API is documented with; a struct would add a concept
    #[allow(clippy::too_many_arguments)]
    pub fn fill(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        ch: char,
        colour: Colour,
        background: Colour,
    ) {
        let (width, height) = self.size();
        let x0 = i64::from(x).max(0);
        let y0 = i64::from(y).max(0);
        let x1 = (i64::from(x) + i64::from(w)).min(i64::from(width));
        let y1 = (i64::from(y) + i64::from(h)).min(i64::from(height));
        for row in y0..y1 {
            for column in x0..x1 {
                self.display
                    .set_pixel(column as isize, row as isize, ch, colour, background);
            }
        }
    }

    /// Draws a horizontal bar `width` cells wide, such as a health bar. The
    /// part up to `value` is drawn in `full` and the rest in `empty`. `value`
    /// is clamped to 0.0 to 1.0, and NaN counts as 0.0.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(10, 1);
    /// screen.bar(0, 0, 10, 0.3, Colour::Green, Colour::Red);
    /// let health = screen.display().pixel(2, 0).unwrap();
    /// assert_eq!((health.character(), health.colour()), ('\u{2588}', Colour::Green));
    /// assert_eq!(screen.display().pixel(5, 0).unwrap().colour(), Colour::Red);
    /// ```
    pub fn bar(&mut self, x: i32, y: i32, width: i32, value: f32, full: Colour, empty: Colour) {
        if width <= 0 {
            return;
        }
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        };
        let filled = i64::from((value * width as f32).round() as i32);
        let (screen_width, screen_height) = self.size();
        let row = i64::from(y);
        if row < 0 || row >= i64::from(screen_height) {
            return;
        }
        // only the cells of the bar that are on the screen are visited
        let first = (-i64::from(x)).max(0);
        let last = (i64::from(screen_width) - i64::from(x)).min(i64::from(width));
        for cell in first..last {
            let colour = if cell < filled { full } else { empty };
            self.display.set_pixel(
                (i64::from(x) + cell) as isize,
                row as isize,
                '\u{2588}',
                colour,
                colour,
            );
        }
    }

    /// Draws a [`Sprite`] with its top left cell at `(x, y)`. Transparent
    /// cells are skipped, so the background shows through them, and any part
    /// off the screen is clipped.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(10, 3);
    /// let boat = Sprite::new("|\\\n~~~", Colour::White);
    /// screen.sprite(4, 1, &boat);
    /// assert_eq!(screen.display().pixel(4, 1).unwrap().character(), '|');
    /// ```
    pub fn sprite(&mut self, x: i32, y: i32, sprite: &Sprite) {
        let (x, y) = (i64::from(x), i64::from(y));
        for row in 0..sprite.height() {
            for column in 0..sprite.width() {
                if let Some((ch, colour)) = sprite.cell(column, row) {
                    if let Some(pixel) = self.cell_mut(x + column as i64, y + row as i64) {
                        pixel.set_character(ch);
                        pixel.set_colour(colour);
                    }
                }
            }
        }
    }

    /// Draws `text` on one row, starting at column `x`, keeping the
    /// backgrounds that are already there. Text that runs off either side is
    /// cut off.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(10, 1);
    /// screen.text(2, 0, "score", Colour::Yellow);
    /// assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 's');
    /// ```
    pub fn text(&mut self, x: i32, y: i32, text: &str, colour: Colour) {
        self.text_at(i64::from(x), i64::from(y), text, colour);
    }

    /// Draws `text` on row `y`, centred on the screen. The text is counted in
    /// characters, so accents and other non-ASCII text centre correctly.
    /// When the space does not divide evenly, the extra column goes on the
    /// right.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(9, 1);
    /// screen.text_centred(0, "hi", Colour::White);
    /// assert_eq!(screen.display().pixel(3, 0).unwrap().character(), 'h');
    /// ```
    pub fn text_centred(&mut self, y: i32, text: &str, colour: Colour) {
        let free = i64::from(self.size().0) - text.chars().count() as i64;
        self.text_at(free.div_euclid(2), i64::from(y), text, colour);
    }

    /// [`text`](Screen::text) with an `i64` start, so callers can offset it
    /// without overflow.
    fn text_at(&mut self, x: i64, y: i64, text: &str, colour: Colour) {
        for (i, ch) in text.chars().enumerate() {
            if let Some(pixel) = self.cell_mut(x + i as i64, y) {
                // a control character such as '\n' would break the frame, so it shows as a space
                pixel.set_character(if ch.is_control() { ' ' } else { ch });
                pixel.set_colour(colour);
            }
        }
    }

    /// Draws `ch` in `colour` on `background` at column `x`, row `y`. Use it
    /// when a cell needs a new background, such as a wall or a hit flash.
    /// Positions off the screen are ignored.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(3, 1);
    /// screen.put_on(0, 0, '#', Colour::White, Colour::Red);
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().background(), Colour::Red);
    /// ```
    pub fn put_on(&mut self, x: i32, y: i32, ch: char, colour: Colour, background: Colour) {
        if let Some(pixel) = self.cell_mut(i64::from(x), i64::from(y)) {
            pixel.set_character(ch);
            pixel.set_colour(colour);
            pixel.set_background(background);
        }
    }

    /// The cell at `(x, y)` if it is on the screen. The position is `i64` so
    /// that the bounds check cannot overflow.
    fn cell_mut(&mut self, x: i64, y: i64) -> Option<&mut Pixel> {
        let (width, height) = self.size();
        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
            return None;
        }
        self.display.pixel_mut(x as isize, y as isize)
    }

    /// The width in columns.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(20, 5);
    /// let centre_x = screen.width() / 2;
    /// assert_eq!(centre_x, 10);
    /// ```
    pub fn width(&self) -> i32 {
        to_i32(self.display.width())
    }

    /// The height in rows.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(20, 5);
    /// let middle_row = screen.height() / 2;
    /// assert_eq!(middle_row, 2);
    /// ```
    pub fn height(&self) -> i32 {
        to_i32(self.display.height())
    }

    /// The size as `(width, height)` in columns and rows.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(20, 5);
    /// assert_eq!(screen.size(), (20, 5));
    /// ```
    pub fn size(&self) -> (u32, u32) {
        self.display.size()
    }

    /// The underlying [`Display`], for the terminal_graphics and art drawing
    /// functions that take one, such as `sketch_frame`.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(4, 1);
    /// assert_eq!(screen.display().size(), (4, 1));
    /// ```
    pub fn display(&self) -> &Display {
        &self.display
    }

    /// Mutable access to the underlying [`Display`]. Anything drawn through it
    /// is shown on the next frame.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(4, 1);
    /// screen.display_mut().set_pixel(0, 0, 'x', Colour::Red, Colour::Black);
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'x');
    /// ```
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.display
    }

    /// Seconds since the game started, as set by the game loop. Use it to
    /// animate: `screen.time().sin()` moves smoothly.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(10, 1);
    /// // a marker that slides right by one cell each second
    /// let x = screen.time() as i32;
    /// screen.put(x, 0, '*', Colour::Yellow);
    /// assert_eq!(screen.time(), 0.0); // the loop has not set the clock yet
    /// ```
    pub fn time(&self) -> f32 {
        self.time
    }

    /// How many updates the game has run, as set by the game loop.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(4, 1);
    /// let frame = screen.tick();
    /// // blink: even updates show the star, odd updates hide it
    /// let ch = if frame % 2 == 0 { '*' } else { ' ' };
    /// screen.put(0, 0, ch, Colour::Yellow);
    /// assert_eq!(frame, 0);
    /// ```
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Called by the game loop before drawing, to set the clock.
    pub(crate) fn set_clock(&mut self, time: f32, tick: u64) {
        self.time = time;
        self.tick = tick;
    }
}

/// A size as an `i32` for drawing arithmetic. Terminals are nowhere near
/// `i32::MAX` wide, so the clamp only matters for absurd sizes.
fn to_i32(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_loop_sets_the_clock_and_drawing_code_reads_it() {
        let mut screen = Screen::new(1, 1);
        assert_eq!((screen.time(), screen.tick()), (0.0, 0));
        screen.set_clock(1.5, 7);
        assert_eq!((screen.time(), screen.tick()), (1.5, 7));
    }

    #[test]
    fn put_keeps_the_background_so_things_float_over_art() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        screen.put(0, 0, 'x', Colour::Red);
        assert_eq!(
            screen.display().pixel(0, 0).unwrap().background(),
            Colour::Blue
        );
    }

    #[test]
    fn text_draws_characters_from_left_to_right() {
        let mut screen = Screen::new(4, 1);
        screen.text(1, 0, "hi", Colour::Red);
        assert_eq!(screen.display().pixel(1, 0).unwrap().character(), 'h');
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'i');
        assert_eq!(screen.display().pixel(2, 0).unwrap().colour(), Colour::Red);
    }

    #[test]
    fn text_is_clipped_on_the_right_and_does_not_wrap_to_the_next_row() {
        let mut screen = Screen::new(3, 2);
        screen
            .display_mut()
            .set_pixel(0, 1, 'q', Colour::Red, Colour::Red);
        screen.text(2, 0, "xyz", Colour::Red);
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'x');
        assert_eq!(screen.display().pixel(0, 1).unwrap().character(), 'q');
    }

    #[test]
    fn fill_paints_a_rectangle_and_leaves_the_rest_alone() {
        let mut screen = Screen::new(4, 4);
        screen.fill(1, 1, 2, 2, '#', Colour::Red, Colour::Blue);
        let inside = screen.display().pixel(2, 2).unwrap();
        assert_eq!(
            (inside.character(), inside.colour(), inside.background()),
            ('#', Colour::Red, Colour::Blue)
        );
        assert_eq!(screen.display().pixel(3, 3).unwrap().character(), ' ');
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), ' ');
    }

    #[test]
    fn a_bar_is_filled_in_proportion_to_its_value() {
        let mut screen = Screen::new(4, 1);
        screen.bar(0, 0, 4, 0.5, Colour::Green, Colour::Red);
        let colour_at = |x| screen.display().pixel(x, 0).unwrap().colour();
        assert_eq!(
            (colour_at(0), colour_at(1), colour_at(2), colour_at(3)),
            (Colour::Green, Colour::Green, Colour::Red, Colour::Red)
        );
    }

    #[test]
    fn a_sprite_at_the_extremes_of_i32_is_clipped_without_overflow() {
        let mut screen = Screen::new(2, 2);
        let sprite = Sprite::new("abc\nabc", Colour::Red);
        screen.sprite(i32::MAX - 1, i32::MAX - 1, &sprite);
        screen.sprite(i32::MIN, i32::MIN, &sprite);
        screen.sprite(i32::MIN + 1, 0, &sprite);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(screen.display().pixel(x, y).unwrap().character(), ' ');
            }
        }
    }

    #[test]
    fn a_sprite_hanging_off_the_top_or_bottom_edge_is_clipped() {
        let mut screen = Screen::new(1, 2);
        screen.sprite(0, -1, &Sprite::new("a\nb\nc", Colour::Red));
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'b');
        assert_eq!(screen.display().pixel(0, 1).unwrap().character(), 'c');
    }

    #[test]
    fn a_sprite_hanging_off_the_left_or_right_edge_is_clipped() {
        let mut screen = Screen::new(3, 1);
        screen.sprite(-1, 0, &Sprite::new("abc", Colour::Red));
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'b');
        assert_eq!(screen.display().pixel(1, 0).unwrap().character(), 'c');
        screen.sprite(2, 0, &Sprite::new("xyz", Colour::Red));
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'x');
        assert_eq!(screen.display().render().matches('y').count(), 0);
    }

    #[test]
    fn a_sprite_leaves_transparent_cells_showing_the_background() {
        let mut screen = Screen::new(3, 1);
        screen.clear(Colour::Blue);
        screen.sprite(0, 0, &Sprite::new("a b", Colour::Red));
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'a');
        let gap = screen.display().pixel(1, 0).unwrap();
        assert_eq!((gap.character(), gap.background()), (' ', Colour::Blue));
    }

    #[test]
    fn a_bar_at_zero_is_empty_and_at_one_is_full() {
        let mut screen = Screen::new(2, 2);
        screen.bar(0, 0, 2, 0.0, Colour::Green, Colour::Red);
        screen.bar(0, 1, 2, 1.0, Colour::Green, Colour::Red);
        assert_eq!(screen.display().pixel(1, 0).unwrap().colour(), Colour::Red);
        assert_eq!(
            screen.display().pixel(1, 1).unwrap().colour(),
            Colour::Green
        );
    }

    #[test]
    fn a_bar_is_clipped_and_a_huge_bar_is_quick() {
        let mut screen = Screen::new(3, 1);
        screen.bar(-1, 0, i32::MAX, 1.0, Colour::Green, Colour::Red);
        assert_eq!(
            screen.display().pixel(0, 0).unwrap().colour(),
            Colour::Green
        );
        assert_eq!(
            screen.display().pixel(2, 0).unwrap().colour(),
            Colour::Green
        );
        screen.bar(i32::MAX, 0, 5, 1.0, Colour::Blue, Colour::Blue);
        screen.bar(0, 0, 0, 1.0, Colour::Blue, Colour::Blue);
        assert_eq!(
            screen.display().pixel(2, 0).unwrap().colour(),
            Colour::Green
        );
    }

    #[test]
    fn a_negative_or_nan_bar_value_is_an_empty_bar() {
        let mut screen = Screen::new(3, 2);
        screen.bar(0, 0, 3, -1.0, Colour::Green, Colour::Red);
        screen.bar(0, 1, 3, f32::NAN, Colour::Green, Colour::Red);
        assert_eq!(screen.display().pixel(0, 0).unwrap().colour(), Colour::Red);
        assert_eq!(screen.display().pixel(2, 1).unwrap().colour(), Colour::Red);
    }

    #[test]
    fn a_bar_value_above_one_is_a_full_bar() {
        let mut screen = Screen::new(3, 1);
        screen.bar(0, 0, 3, 2.0, Colour::Green, Colour::Red);
        assert_eq!(
            screen.display().pixel(2, 0).unwrap().colour(),
            Colour::Green
        );
    }

    #[test]
    fn fill_with_a_huge_size_fills_only_the_screen() {
        let mut screen = Screen::new(2, 2);
        screen.fill(0, 0, i32::MAX, i32::MAX, '#', Colour::Red, Colour::Red);
        screen.fill(
            i32::MAX,
            i32::MAX,
            i32::MAX,
            i32::MAX,
            '@',
            Colour::Red,
            Colour::Red,
        );
        assert_eq!(screen.display().pixel(1, 1).unwrap().character(), '#');
        assert_eq!(screen.display().render().matches('#').count(), 4);
    }

    #[test]
    fn fill_fills_nothing_for_a_negative_size() {
        let mut screen = Screen::new(3, 3);
        screen.fill(1, 1, -5, -5, '#', Colour::Red, Colour::Red);
        assert_eq!(screen.display().pixel(1, 1).unwrap().character(), ' ');
    }

    #[test]
    fn fill_is_clipped_to_the_screen() {
        let mut screen = Screen::new(3, 3);
        screen.fill(-2, -2, 4, 4, '#', Colour::Red, Colour::Red);
        assert_eq!(screen.display().pixel(1, 1).unwrap().character(), '#');
        assert_eq!(screen.display().pixel(2, 2).unwrap().character(), ' ');
    }

    #[test]
    fn text_wider_than_the_screen_is_cut_off_on_both_sides() {
        let mut screen = Screen::new(3, 1);
        screen.text_centred(0, "abcde", Colour::Red);
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'b');
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'd');
    }

    #[test]
    fn text_centred_counts_characters_not_bytes() {
        let mut screen = Screen::new(9, 1);
        screen.text_centred(0, "ééé", Colour::Red);
        assert_eq!(screen.display().pixel(3, 0).unwrap().character(), 'é');
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), ' ');
    }

    #[test]
    fn text_centred_on_an_even_width_leaves_equal_margins() {
        let mut screen = Screen::new(10, 1);
        screen.text_centred(0, "abcd", Colour::Red);
        assert_eq!(screen.display().pixel(3, 0).unwrap().character(), 'a');
        assert_eq!(screen.display().pixel(6, 0).unwrap().character(), 'd');
        assert_eq!(screen.display().pixel(7, 0).unwrap().character(), ' ');
    }

    #[test]
    fn text_centred_on_an_odd_width_puts_the_extra_space_on_the_right() {
        let mut screen = Screen::new(9, 1);
        screen.text_centred(0, "abcd", Colour::Red);
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'a');
        assert_eq!(screen.display().pixel(5, 0).unwrap().character(), 'd');
    }

    #[test]
    fn text_at_the_extremes_of_i32_is_clipped_without_overflow() {
        let mut screen = Screen::new(3, 1);
        screen.text(i32::MAX, 0, "abc", Colour::Red);
        screen.text(i32::MIN, 0, "abc", Colour::Red);
        screen.text(i32::MAX, i32::MAX, "abc", Colour::Red);
        screen.text(i32::MIN, i32::MIN, "abc", Colour::Red);
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), ' ');
    }

    #[test]
    fn a_newline_in_text_stays_on_one_row_and_shows_as_a_space() {
        let mut screen = Screen::new(3, 2);
        screen
            .display_mut()
            .set_pixel(0, 1, 'q', Colour::Red, Colour::Red);
        screen.text(0, 0, "a\nb", Colour::Red);
        assert_eq!(screen.display().pixel(1, 0).unwrap().character(), ' ');
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'b');
        assert_eq!(screen.display().pixel(0, 1).unwrap().character(), 'q');
    }

    #[test]
    fn text_above_or_below_the_screen_draws_nothing() {
        let mut screen = Screen::new(2, 2);
        screen
            .display_mut()
            .set_pixel(0, 0, 'q', Colour::Red, Colour::Red);
        screen
            .display_mut()
            .set_pixel(0, 1, 'q', Colour::Red, Colour::Red);
        screen.text(0, -1, "abc", Colour::Blue);
        screen.text(0, 2, "abc", Colour::Blue);
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'q');
        assert_eq!(screen.display().pixel(0, 1).unwrap().character(), 'q');
    }

    #[test]
    fn text_is_clipped_on_the_left() {
        let mut screen = Screen::new(2, 1);
        screen.text(-1, 0, "abc", Colour::Red);
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'b');
        assert_eq!(screen.display().pixel(1, 0).unwrap().character(), 'c');
    }

    #[test]
    fn put_on_sets_the_background_as_well() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        screen.put_on(0, 0, 'x', Colour::Red, Colour::Green);
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(
            (cell.colour(), cell.background()),
            (Colour::Red, Colour::Green)
        );
    }

    #[test]
    fn put_at_the_extremes_of_i32_is_ignored_without_overflow() {
        let mut screen = Screen::new(2, 2);
        screen.put(i32::MIN, i32::MIN, 'x', Colour::Red);
        screen.put(i32::MAX, i32::MAX, 'x', Colour::Red);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(screen.display().pixel(x, y).unwrap().character(), ' ');
            }
        }
    }

    #[test]
    fn put_draws_one_character_in_a_colour() {
        let mut screen = Screen::new(3, 1);
        screen.put(1, 0, 'x', Colour::Red);
        let cell = screen.display().pixel(1, 0).unwrap();
        assert_eq!((cell.character(), cell.colour()), ('x', Colour::Red));
    }

    #[test]
    fn clear_makes_every_cell_a_space_on_the_background() {
        let mut screen = Screen::new(2, 2);
        screen
            .display_mut()
            .set_pixel(1, 1, 'x', Colour::Red, Colour::Red);
        screen.clear(Colour::Blue);
        let cell = screen.display().pixel(1, 1).unwrap();
        assert_eq!(
            (cell.character(), cell.colour(), cell.background()),
            (' ', Colour::Black, Colour::Blue)
        );
    }

    #[test]
    fn the_underlying_display_can_be_drawn_on_directly() {
        let mut screen = Screen::new(3, 1);
        screen
            .display_mut()
            .set_pixel(2, 0, 'z', Colour::Red, Colour::Blue);
        assert_eq!(screen.display().pixel(2, 0).unwrap().character(), 'z');
    }

    #[test]
    fn a_new_screen_has_the_size_it_was_made_with() {
        let screen = Screen::new(7, 3);
        assert_eq!(screen.size(), (7, 3));
        assert_eq!((screen.width(), screen.height()), (7, 3));
    }

    #[test]
    fn a_backdrop_is_given_the_screen_clock() {
        let mut screen = Screen::new(2, 1);
        screen.set_clock(2.0, 120);
        let mut seen = f32::NAN;
        screen.backdrop(&Palette::ocean(), |cell| {
            seen = cell.t;
            0.0
        });
        assert_eq!(seen, 2.0);
    }

    #[test]
    fn a_backdrop_draws_the_formula_in_every_cell() {
        let mut screen = Screen::new(3, 2);
        screen.backdrop(&Palette::mono(), |_| 1.0);
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(screen.display().pixel(x, y).unwrap().character(), '█');
            }
        }
    }

    #[test]
    fn backdrop_in_only_changes_its_own_rectangle() {
        let mut screen = Screen::new(6, 4);
        screen.fill(0, 0, 6, 4, 'q', Colour::Red, Colour::Red);
        screen.backdrop_in(1, 1, 3, 2, &Palette::mono(), |_| 1.0);
        for y in 0..4 {
            for x in 0..6 {
                let inside = (1..4).contains(&x) && (1..3).contains(&y);
                let drawn = screen.display().pixel(x, y).unwrap().character() == '█';
                assert_eq!(drawn, inside, "cell ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_backdrop_rectangle_partly_off_the_screen_is_clipped_to_it() {
        let mut screen = Screen::new(3, 3);
        screen.fill(0, 0, 3, 3, 'q', Colour::Red, Colour::Red);
        screen.backdrop_in(-1, -1, 3, 3, &Palette::mono(), |_| 1.0);
        for y in 0..3 {
            for x in 0..3 {
                let inside = x < 2 && y < 2;
                let drawn = screen.display().pixel(x, y).unwrap().character() == '█';
                assert_eq!(drawn, inside, "cell ({x}, {y})");
            }
        }
    }

    #[test]
    fn inside_a_backdrop_rectangle_the_formula_counts_from_its_corner() {
        let mut screen = Screen::new(10, 10);
        let mut first = None;
        let mut last = None;
        screen.backdrop_in(4, 5, 3, 2, &Palette::mono(), |cell| {
            first.get_or_insert((cell.x, cell.y, cell.width, cell.height));
            last = Some((cell.x, cell.y));
            0.0
        });
        assert_eq!(first, Some((0.0, 0.0, 3.0, 4.0)));
        assert_eq!(last, Some((2.0, 2.0)));
    }

    #[test]
    fn an_empty_negative_or_off_screen_backdrop_rectangle_draws_nothing() {
        let rectangles = [
            (1, 1, -2, 2),
            (1, 1, 2, -2),
            (1, 1, 0, 2),
            (1, 1, 2, 0),
            (10, 10, 3, 3),
            (-10, -10, 3, 3),
            (i32::MAX, i32::MAX, i32::MAX, i32::MAX),
            (i32::MIN, i32::MIN, -1, -1),
            (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
        ];
        for (x, y, w, h) in rectangles {
            let mut screen = Screen::new(3, 3);
            screen.fill(0, 0, 3, 3, 'q', Colour::Red, Colour::Red);
            screen.backdrop_in(x, y, w, h, &Palette::mono(), |_| 1.0);
            let drawn = screen.display().render().matches('█').count();
            assert_eq!(drawn, 0, "rectangle ({x}, {y}, {w}, {h})");
        }
    }

    #[test]
    fn a_character_drawn_over_a_backdrop_keeps_the_backdrop_colour() {
        let palette = Palette::ocean();
        let mut backdrop_only = Screen::new(4, 2);
        backdrop_only.backdrop(&palette, |cell| cell.x / 4.0 * 2.0 - 1.0);
        let mut screen = Screen::new(4, 2);
        screen.backdrop(&palette, |cell| cell.x / 4.0 * 2.0 - 1.0);
        screen.put(1, 1, '@', Colour::Yellow);
        let before = backdrop_only.display().pixel(1, 1).unwrap();
        let after = screen.display().pixel(1, 1).unwrap();
        assert_eq!(after.character(), '@');
        assert_eq!(after.background(), before.background());
    }
}

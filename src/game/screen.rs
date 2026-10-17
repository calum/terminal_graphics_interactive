//! The [`Screen`]: what a game draws on each frame.

use super::{PixelArt, Sprite};
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
    // one entry per square pixel: two per cell row, allocated once in `new`
    layer: Vec<Option<Colour>>,
    layer_width: usize,
    layer_height: usize,
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
        let layer_height = height as usize * 2;
        let layer_width = width as usize;
        Screen {
            display: Display::new(width, height),
            time: 0.0,
            tick: 0,
            layer: vec![None; layer_width.checked_mul(layer_height).unwrap_or(0)],
            layer_width,
            layer_height,
        }
    }

    /// Fills the whole screen with spaces on `background`, and throws away the
    /// pixels drawn so far in this frame. Call it first in `draw` when the
    /// background is plain colour.
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
        self.layer.fill(None);
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

    /// Sets the square pixel at `(x, y)` to `colour`. `x` is the column and
    /// `y` the half row: pixel rows `0` and `1` are the top and bottom halves
    /// of cell row `0`. Two pixels in one cell are drawn as one half block or
    /// one solid cell by [`Screen::finish_frame`]. Positions off the screen are
    /// ignored.
    ///
    /// Pixels are composited when the frame finishes, so they show above
    /// everything drawn with `put`, `text`, `fill`, `sprite` or `backdrop` in
    /// the same frame. `run` and [`Headless`] call `finish_frame` for you.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(3, 1);
    /// screen.clear(Colour::Blue);
    /// screen.pixel(1, 0, Colour::Red); // top half of cell (1, 0)
    /// screen.pixel(1, 1, Colour::Red); // bottom half: both match, so a solid cell
    /// screen.finish_frame();
    /// assert_eq!(screen.display().pixel(1, 0).unwrap().background(), Colour::Red);
    /// ```
    pub fn pixel(&mut self, x: i32, y: i32, colour: Colour) {
        if let Some(pixel) = self.layer_mut(i64::from(x), i64::from(y)) {
            *pixel = Some(colour);
        }
    }

    /// The layer entry for pixel `(x, y)`, or `None` when it is off the screen.
    fn layer_mut(&mut self, x: i64, y: i64) -> Option<&mut Option<Colour>> {
        if x < 0 || y < 0 || x >= self.layer_width as i64 || y >= self.layer_height as i64 {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.layer_width)?
            .checked_add(x as usize)?;
        self.layer.get_mut(index)
    }

    /// Fills a round disc of pixels: the pixels whose centres are within
    /// `radius` of `(cx, cy)`. The centre is in pixel units, so `cy` counts
    /// half rows (see [`Screen::pixel_height`]), and the radius is in
    /// pixels too. Pixels are square, so a disc is round on screen.
    ///
    /// A radius that is not finite or not above zero draws nothing, and so
    /// does a centre that is not finite. Parts off the screen are clipped.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(8, 4);
    /// screen.clear(Colour::Black);
    /// screen.disc(4.0, 4.0, 2.0, Colour::Yellow); // a ball in pixel units
    /// screen.finish_frame();
    /// assert_eq!(screen.display().pixel(4, 2).unwrap().background(), Colour::Yellow);
    /// ```
    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, colour: Colour) {
        if !(cx.is_finite() && cy.is_finite() && radius.is_finite() && radius > 0.0) {
            return;
        }
        let (x0, x1) = span(cx, radius, self.width());
        let (y0, y1) = span(cy, radius, self.pixel_height());
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy < radius * radius {
                    if let Some(pixel) = self.layer_mut(i64::from(x), i64::from(y)) {
                        *pixel = Some(colour);
                    }
                }
            }
        }
    }

    /// A soft light centred on `(cx, cy)`, such as a lamp or a glowing player.
    /// The centre is in pixel units (`cy` counts half rows) and so is the
    /// radius. The light is drawn at once onto the cells already there, so it
    /// blends into the background behind it: a solid core, then shade blocks
    /// (`░ ▒ ▓`) that keep the background, fading out at the radius. The core
    /// is the bright end of `palette`.
    ///
    /// It is drawn straight onto the cells, not composited, so text drawn after
    /// it shows above it. A non-finite centre or radius, or a radius that is not
    /// above zero, draws nothing.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(20, 10);
    /// screen.clear(Colour::Black);
    /// screen.glow(10.0, 10.0, 4.0, &Palette::fire());
    /// assert_eq!(screen.display().pixel(10, 5).unwrap().character(), '\u{2593}');
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().background(), Colour::Black);
    /// ```
    pub fn glow(&mut self, cx: f32, cy: f32, radius: f32, palette: &Palette) {
        if !(cx.is_finite() && cy.is_finite() && radius.is_finite() && radius > 0.0) {
            return;
        }
        let colours = palette.colours();
        let steps = colours.len() - 1;
        let (x0, x1) = span(cx, radius, self.width());
        let (y0, y1) = span(cy / 2.0, radius / 2.0, self.height());
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = 2.0 * y as f32 + 1.0 - cy;
                let reach = (1.0 - (dx * dx + dy * dy).sqrt() / radius).clamp(0.0, 1.0);
                let intensity = reach * reach;
                if intensity < 0.08 {
                    continue;
                }
                let colour = colours[((intensity * steps as f32).round() as usize).min(steps)];
                let glyph = if intensity < 0.25 {
                    Some('\u{2591}')
                } else if intensity < 0.5 {
                    Some('\u{2592}')
                } else if intensity < 0.75 {
                    Some('\u{2593}')
                } else {
                    None
                };
                if let Some(pixel) = self.display.pixel_mut(x as isize, y as isize) {
                    match glyph {
                        Some(shade) => {
                            pixel.set_character(shade);
                            pixel.set_colour(colour);
                        }
                        None => {
                            pixel.set_character(' ');
                            pixel.set_background(colour);
                        }
                    }
                }
            }
        }
    }

    /// Draws a [`PixelArt`] with its top left pixel at `(x, y)` in pixel units
    /// (see [`Screen::pixel`]). Transparent pixels leave the layer as it is.
    /// The part off the screen is clipped.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(6, 2);
    /// screen.clear(Colour::Blue);
    /// let dot = PixelArt::new("W", &[('W', Colour::White)]);
    /// screen.pixel_art(2, 3, &dot); // pixel row 3 is the bottom half of cell row 1
    /// screen.finish_frame();
    /// assert_eq!(screen.display().pixel(2, 1).unwrap().character(), '\u{2584}');
    /// ```
    pub fn pixel_art(&mut self, x: i32, y: i32, art: &PixelArt) {
        let (x, y) = (i64::from(x), i64::from(y));
        for row in 0..art.height() {
            for column in 0..art.width() {
                if let Some(colour) = art.pixel(column, row) {
                    let (px, py) = (
                        x.saturating_add(i64::try_from(column).unwrap_or(i64::MAX)),
                        y.saturating_add(i64::try_from(row).unwrap_or(i64::MAX)),
                    );
                    if let Some(pixel) = self.layer_mut(px, py) {
                        *pixel = Some(colour);
                    }
                }
            }
        }
    }

    /// Erases the pixels drawn so far in the `w` by `h` block of cells whose top
    /// left cell is `(x, y)`, both halves of every cell in it. Cells are counted
    /// as in [`Screen::put`], not pixels. Pixels outside the block stay, and the
    /// part off the screen is clipped. A block that is empty erases nothing.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(3, 1);
    /// screen.pixel(0, 0, Colour::Red);
    /// screen.pixel(2, 0, Colour::Red);
    /// screen.clear_pixels(1, 0, 2, 1); // the cells at columns 1 and 2
    /// screen.finish_frame();
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().character(), '\u{2580}');
    /// assert_eq!(screen.display().pixel(2, 0).unwrap().character(), ' ');
    /// ```
    pub fn clear_pixels(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let (width, height) = self.size();
        let x0 = i64::from(x).max(0);
        let y0 = i64::from(y).max(0);
        let x1 = (i64::from(x) + i64::from(w)).min(i64::from(width));
        let y1 = (i64::from(y) + i64::from(h)).min(i64::from(height));
        for row in y0..y1 {
            for column in x0..x1 {
                for half in [2 * row, 2 * row + 1] {
                    if let Some(pixel) = self.layer_mut(column, half) {
                        *pixel = None;
                    }
                }
            }
        }
    }

    /// Draws a panel: a `w` by `h` rectangle at `(x, y)`, including its border,
    /// with a double-line border in `border` colour and a diamond at each
    /// corner, and its inside filled with `background`. Use it for menus,
    /// pause screens and game-over text. A panel smaller than 2 by 2 draws
    /// nothing.
    ///
    /// The panel also erases the pixels drawn so far under it (see
    /// [`Screen::clear_pixels`]), so the sea or the sky does not show through.
    /// Pixels drawn after it still show above it, so draw panels last.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(12, 4);
    /// screen.clear(Colour::Blue);
    /// screen.frame(0, 0, 12, 4, Colour::Yellow, Colour::Black);
    /// screen.text_centred(1, "paused", Colour::White);
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().character(), '\u{25C6}');
    /// assert_eq!(screen.display().pixel(3, 1).unwrap().character(), 'p');
    /// ```
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, border: Colour, background: Colour) {
        if w < 2 || h < 2 {
            return;
        }
        // i64 so that no position or size overflows; the loops below only visit
        // the cells of the border that are on the screen, however big the panel
        let (x0, y0) = (i64::from(x), i64::from(y));
        let (x1, y1) = (x0 + i64::from(w) - 1, y0 + i64::from(h) - 1);
        self.fill(x, y, w, h, ' ', border, background);
        self.clear_pixels(x, y, w, h);
        let (width, height) = self.size();
        let (width, height) = (i64::from(width), i64::from(height));
        for column in x0.max(0)..=x1.min(width - 1) {
            for row in [y0, y1] {
                let corner = column == x0 || column == x1;
                let ch = if corner { '\u{25C6}' } else { '\u{2550}' };
                self.border_cell(column, row, ch, border, background);
            }
        }
        for row in (y0 + 1).max(0)..=(y1 - 1).min(height - 1) {
            for column in [x0, x1] {
                self.border_cell(column, row, '\u{2551}', border, background);
            }
        }
    }

    /// Draws one character of a panel's border, if it is on the screen.
    fn border_cell(&mut self, x: i64, y: i64, ch: char, colour: Colour, background: Colour) {
        if let Some(pixel) = self.cell_mut(x, y) {
            pixel.set_character(ch);
            pixel.set_colour(colour);
            pixel.set_background(background);
        }
    }

    /// The height in square pixels: two for each row, so `pixel_height` is
    /// twice [`Screen::height`].
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let screen = Screen::new(10, 4);
    /// assert_eq!(screen.pixel_height(), 8);
    /// ```
    pub fn pixel_height(&self) -> i32 {
        to_i32(self.layer_height as u32)
    }

    /// Composites the pixel layer onto the screen. Call it once a frame, after
    /// everything is drawn; `run` and [`Headless`] do this for you.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut screen = Screen::new(1, 1);
    /// screen.clear(Colour::Blue);
    /// screen.pixel(0, 0, Colour::Red);
    /// screen.finish_frame();
    /// assert_eq!(screen.display().pixel(0, 0).unwrap().character(), '\u{2580}');
    /// ```
    pub fn finish_frame(&mut self) {
        let (width, height) = self.size();
        for row in 0..height as usize {
            for column in 0..width as usize {
                let top = self.layer_at(column, 2 * row);
                let bottom = self.layer_at(column, 2 * row + 1);
                let Some(pixel) = self.display.pixel_mut(column as isize, row as isize) else {
                    continue;
                };
                match (top, bottom) {
                    (None, None) => {}
                    (Some(a), Some(b)) if a == b => {
                        pixel.set_character(' ');
                        pixel.set_background(a);
                    }
                    (Some(a), Some(b)) => {
                        pixel.set_character('\u{2580}');
                        pixel.set_colour(a);
                        pixel.set_background(b);
                    }
                    (Some(a), None) => {
                        pixel.set_character('\u{2580}');
                        pixel.set_colour(a);
                    }
                    (None, Some(b)) => {
                        pixel.set_character('\u{2584}');
                        pixel.set_colour(b);
                    }
                }
            }
        }
        self.layer.fill(None);
    }

    /// The colour of layer pixel `(column, y)` for compositing; `None` when
    /// nothing is drawn there.
    fn layer_at(&self, column: usize, y: usize) -> Option<Colour> {
        self.layer
            .get(y * self.layer_width + column)
            .copied()
            .flatten()
    }
}

/// The range `lo..hi` of whole numbers from `centre - radius` to `centre +
/// radius`, cut down to `0..limit`. The centre and radius must be finite.
fn span(centre: f32, radius: f32, limit: i32) -> (i32, i32) {
    let top = limit as f32;
    let lo = (centre - radius).floor().clamp(0.0, top) as i32;
    let hi = ((centre + radius).ceil() + 1.0).clamp(0.0, top) as i32;
    (lo, hi)
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
    fn a_top_half_pixel_draws_an_upper_half_block_on_the_existing_background() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        screen.pixel(0, 0, Colour::Red);
        screen.finish_frame();
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(
            (cell.character(), cell.colour(), cell.background()),
            ('\u{2580}', Colour::Red, Colour::Blue)
        );
    }

    #[test]
    fn a_bottom_half_pixel_draws_a_lower_half_block_on_the_existing_background() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        screen.pixel(0, 1, Colour::Green);
        screen.finish_frame();
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(
            (cell.character(), cell.colour(), cell.background()),
            ('\u{2584}', Colour::Green, Colour::Blue)
        );
    }

    #[test]
    fn two_different_pixels_in_one_cell_draw_one_half_block_with_both_colours() {
        let mut screen = Screen::new(1, 1);
        screen.clear(Colour::White);
        screen.pixel(0, 0, Colour::Red);
        screen.pixel(0, 1, Colour::Blue);
        screen.finish_frame();
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(
            (cell.character(), cell.colour(), cell.background()),
            ('\u{2580}', Colour::Red, Colour::Blue)
        );
    }

    #[test]
    fn two_equal_pixels_in_one_cell_draw_a_space_on_that_colour_and_keep_the_foreground() {
        let mut screen = Screen::new(1, 1);
        screen.clear(Colour::White);
        screen.put(0, 0, 'x', Colour::Yellow);
        screen.pixel(0, 0, Colour::Red);
        screen.pixel(0, 1, Colour::Red);
        screen.finish_frame();
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(
            (cell.character(), cell.colour(), cell.background()),
            (' ', Colour::Yellow, Colour::Red)
        );
    }

    #[test]
    fn pixels_off_every_edge_of_the_screen_are_clipped_and_never_wrap_round() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        let outside = [
            (-1, 0),
            (2, 0),
            (0, -1),
            (0, 2),
            (-1, 1),
            (2, 1),
            (1, -1),
            (i32::MIN, 0),
            (i32::MAX, 0),
            (0, i32::MIN),
            (0, i32::MAX),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            (i32::MIN, i32::MAX),
            (i32::MAX, i32::MIN),
        ];
        for (x, y) in outside {
            screen.pixel(x, y, Colour::Red);
        }
        screen.finish_frame();
        assert_eq!(screen.display().render().matches('\u{2580}').count(), 0);
        assert_eq!(screen.display().render().matches('\u{2584}').count(), 0);
        for x in 0..2 {
            assert_eq!(
                screen.display().pixel(x, 0).unwrap().background(),
                Colour::Blue
            );
        }
    }

    #[test]
    fn clear_throws_away_the_pixels_drawn_so_far() {
        let mut screen = Screen::new(1, 1);
        screen.pixel(0, 0, Colour::Red);
        screen.clear(Colour::Blue);
        screen.finish_frame();
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!((cell.character(), cell.background()), (' ', Colour::Blue));
    }

    #[test]
    fn a_second_finish_frame_changes_nothing() {
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        screen.pixel(0, 0, Colour::Red);
        screen.finish_frame();
        screen.put(0, 0, 'x', Colour::Yellow);
        screen.finish_frame();
        assert_eq!(screen.display().pixel(0, 0).unwrap().character(), 'x');
    }

    #[test]
    fn the_pixel_height_is_twice_the_rows() {
        let screen = Screen::new(5, 3);
        assert_eq!(screen.pixel_height(), 6);
    }

    /// Whether the layer has a pixel at `(x, y)`, for the tests of the layer.
    fn lit(screen: &Screen, x: i32, y: i32) -> bool {
        screen.layer[y as usize * screen.layer_width + x as usize].is_some()
    }

    #[test]
    fn a_disc_lights_the_pixels_whose_centres_are_within_its_radius() {
        let mut screen = Screen::new(20, 10);
        screen.disc(10.0, 10.0, 3.0, Colour::Red);
        assert!(lit(&screen, 10, 10), "the centre is lit");
        assert!(lit(&screen, 12, 10), "a pixel 2.5 away is lit");
        assert!(!lit(&screen, 13, 10), "a pixel 3.5 away is not");
        assert!(!lit(&screen, 10, 14), "a pixel 3.5 rows away is not");
    }

    /// How many pixels of the layer are lit.
    fn lit_count(screen: &Screen) -> usize {
        screen.layer.iter().filter(|p| p.is_some()).count()
    }

    #[test]
    fn a_disc_is_round_so_its_area_is_close_to_pi_times_the_radius_squared() {
        let mut screen = Screen::new(40, 20);
        screen.disc(20.0, 20.0, 5.0, Colour::Red);
        let area = std::f32::consts::PI * 25.0;
        let count = lit_count(&screen) as f32;
        assert!(
            (count - area).abs() < area * 0.05,
            "a radius 5 disc lit {count} pixels, expected about {area}"
        );
    }

    #[test]
    fn a_huge_or_off_screen_disc_is_clipped_to_the_screen() {
        let mut screen = Screen::new(3, 1);
        screen.disc(0.0, 0.0, 1.0e30, Colour::Red);
        assert_eq!(lit_count(&screen), 6, "every pixel of a 3 by 1 screen");

        let mut edge = Screen::new(3, 1);
        edge.disc(-1.0, 1.0, 2.5, Colour::Red);
        assert!(
            lit(&edge, 0, 0) && lit(&edge, 0, 1),
            "the part on screen is lit"
        );
        assert!(!lit(&edge, 2, 1), "the far side is not");
        edge.disc(f32::MAX, f32::MAX, f32::MAX, Colour::Red); // must not panic or loop
    }

    #[test]
    fn a_disc_with_a_nan_negative_or_zero_radius_or_a_bad_centre_draws_nothing() {
        let mut screen = Screen::new(10, 10);
        screen.disc(5.0, 5.0, f32::NAN, Colour::Red);
        screen.disc(5.0, 5.0, -3.0, Colour::Red);
        screen.disc(5.0, 5.0, 0.0, Colour::Red);
        screen.disc(5.0, 5.0, f32::INFINITY, Colour::Red);
        screen.disc(f32::NAN, 5.0, 2.0, Colour::Red);
        screen.disc(5.0, f32::NEG_INFINITY, 2.0, Colour::Red);
        assert_eq!(lit_count(&screen), 0);
    }

    #[test]
    fn pixel_art_is_drawn_from_its_top_left_pixel_and_clipped_at_the_edge() {
        let mut screen = Screen::new(2, 1);
        let art = PixelArt::new("RB\nBR", &[('R', Colour::Red), ('B', Colour::Blue)]);
        screen.pixel_art(-1, 0, &art);
        // the art's column 1 lands on screen column 0; its column 0 is off screen
        assert_eq!(screen.layer[0], Some(Colour::Blue));
        assert_eq!(screen.layer[2], Some(Colour::Red));
        assert_eq!(lit_count(&screen), 2);
    }

    /// The colour a cell shows as light: its background when it is a solid
    /// block, otherwise its foreground.
    fn light_of(screen: &Screen, x: i32, y: i32) -> Colour {
        let cell = screen.display().pixel(x as isize, y as isize).unwrap();
        if cell.character() == ' ' {
            cell.background()
        } else {
            cell.colour()
        }
    }

    #[test]
    fn a_glow_is_brighter_at_its_centre_than_at_its_edge() {
        let palette = Palette::fire();
        let rank = |colour: Colour| palette.colours().iter().position(|c| *c == colour);
        let mut screen = Screen::new(40, 20);
        screen.clear(Colour::Black);
        screen.glow(20.0, 20.0, 12.0, &palette);
        let centre = rank(light_of(&screen, 20, 10)).unwrap();
        let edge = rank(light_of(&screen, 27, 10)).unwrap();
        assert!(
            centre > edge,
            "centre {centre} should be brighter than edge {edge}"
        );
    }

    #[test]
    fn a_glow_softens_into_shade_blocks_that_keep_the_background_behind_them() {
        let mut screen = Screen::new(40, 20);
        screen.clear(Colour::Blue);
        screen.glow(20.0, 20.0, 12.0, &Palette::fire());
        // intensities are about 0.14, 0.38 and 0.72 at these cells
        let shown = |x, y| {
            let cell = screen.display().pixel(x, y).unwrap();
            (cell.character(), cell.colour(), cell.background())
        };
        assert_eq!(shown(27, 10), ('\u{2591}', Colour::Red, Colour::Blue));
        assert_eq!(shown(24, 10), ('\u{2592}', Colour::BrightRed, Colour::Blue));
        assert_eq!(
            shown(21, 10),
            ('\u{2593}', Colour::BrightYellow, Colour::Blue)
        );
    }

    #[test]
    fn a_glow_does_not_touch_a_cell_beyond_its_radius() {
        let mut screen = Screen::new(40, 20);
        screen.clear(Colour::Blue);
        screen.glow(20.0, 20.0, 12.0, &Palette::fire());
        for (x, y) in [(34, 10), (20, 0), (0, 0), (39, 19)] {
            let cell = screen.display().pixel(x, y).unwrap();
            assert_eq!(
                (cell.character(), cell.background()),
                (' ', Colour::Blue),
                "cell ({x}, {y}) is outside the glow"
            );
        }
    }

    #[test]
    fn a_glow_is_clipped_at_the_edges_and_bad_values_draw_nothing() {
        let palette = Palette::fire();
        let mut edge = Screen::new(10, 4);
        edge.clear(Colour::Blue);
        edge.glow(0.0, 0.0, 6.0, &palette); // centred on the top left corner
        assert_eq!(edge.display().pixel(0, 0).unwrap().character(), '\u{2593}');
        edge.glow(-1.0e30, 1.0e30, 1.0e30, &palette); // huge and off screen: no panic, no hang

        let mut bad = Screen::new(10, 4);
        bad.clear(Colour::Blue);
        let before = bad.clone();
        bad.glow(f32::NAN, 2.0, 4.0, &palette);
        bad.glow(5.0, f32::INFINITY, 4.0, &palette);
        bad.glow(5.0, 2.0, f32::NAN, &palette);
        bad.glow(5.0, 2.0, 0.0, &palette);
        bad.glow(5.0, 2.0, -4.0, &palette);
        assert_eq!(bad.display().render(), before.display().render());
    }

    #[test]
    fn two_hundred_random_shapes_drawn_and_finished_never_panic() {
        use crate::game::Rng;
        let mut rng = Rng::new(2026);
        let mut screen = Screen::new(7, 3);
        let palette = Palette::fire();
        let art = PixelArt::new(
            "RB.\n.GG",
            &[
                ('R', Colour::Red),
                ('B', Colour::Blue),
                ('G', Colour::Green),
            ],
        );
        let colours = [Colour::Red, Colour::Blue, Colour::Yellow, Colour::White];
        // a number that is usually ordinary but is sometimes NaN, infinite or extreme
        let number = |rng: &mut Rng| -> f32 {
            match rng.range_i32(0, 8) {
                0 => f32::NAN,
                1 => f32::INFINITY,
                2 => f32::MIN,
                3 => f32::MAX,
                4 => 0.0,
                _ => rng.range_f32(-40.0, 40.0),
            }
        };
        let position = |rng: &mut Rng| -> i32 {
            match rng.range_i32(0, 6) {
                0 => i32::MIN,
                1 => i32::MAX,
                _ => rng.range_i32(-30, 30),
            }
        };
        for _ in 0..200 {
            let colour = colours[rng.range_i32(0, colours.len() as i32) as usize];
            match rng.range_i32(0, 4) {
                0 => screen.pixel(position(&mut rng), position(&mut rng), colour),
                1 => {
                    let (cx, cy, r) = (number(&mut rng), number(&mut rng), number(&mut rng));
                    screen.disc(cx, cy, r, colour);
                }
                2 => {
                    let (cx, cy, r) = (number(&mut rng), number(&mut rng), number(&mut rng));
                    screen.glow(cx, cy, r, &palette);
                }
                _ => screen.pixel_art(position(&mut rng), position(&mut rng), &art),
            }
            if rng.chance(0.5) {
                screen.finish_frame();
            }
        }
        screen.finish_frame();
    }

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

    #[test]
    fn clear_pixels_erases_the_pixels_of_the_cells_inside_and_keeps_the_rest() {
        let mut screen = Screen::new(4, 3);
        for y in 0..6 {
            for x in 0..4 {
                screen.pixel(x, y, Colour::Red);
            }
        }
        // cells (1, 1) and (2, 1) are pixel rows 2 and 3, columns 1 and 2
        screen.clear_pixels(1, 1, 2, 1);
        assert!(
            !lit(&screen, 1, 2) && !lit(&screen, 2, 3),
            "inside: both halves"
        );
        assert!(
            !lit(&screen, 2, 2) && !lit(&screen, 1, 3),
            "inside: the other cells"
        );
        assert!(lit(&screen, 0, 2) && lit(&screen, 3, 3), "beside it: kept");
        assert!(
            lit(&screen, 1, 1) && lit(&screen, 1, 4),
            "above and below: kept"
        );
    }

    #[test]
    fn clear_pixels_is_clipped_to_the_screen_and_does_nothing_for_an_empty_block() {
        let mut screen = Screen::new(3, 2);
        for y in 0..4 {
            for x in 0..3 {
                screen.pixel(x, y, Colour::Red);
            }
        }
        screen.clear_pixels(1, 1, 0, 5);
        screen.clear_pixels(1, 1, 5, -1);
        assert_eq!(lit_count(&screen), 12, "an empty block erases nothing");
        screen.clear_pixels(-5, -5, 100, 100);
        assert_eq!(
            lit_count(&screen),
            0,
            "a block far bigger than the screen clears it"
        );
        screen.pixel(0, 0, Colour::Red);
        screen.clear_pixels(i32::MIN, i32::MAX, i32::MAX, 3);
        screen.clear_pixels(i32::MAX, i32::MIN, 3, i32::MAX);
        screen.clear_pixels(i32::MIN, i32::MIN, i32::MIN, i32::MIN);
        assert!(
            lit(&screen, 0, 0),
            "extreme blocks off the screen leave it alone"
        );
    }

    #[test]
    fn a_frame_has_diamond_corners_and_a_double_line_border() {
        let mut screen = Screen::new(10, 5);
        screen.clear(Colour::Blue);
        screen.frame(1, 1, 6, 3, Colour::Yellow, Colour::Black);
        let ch = |x: isize, y: isize| screen.display().pixel(x, y).unwrap().character();
        for (x, y) in [(1, 1), (6, 1), (1, 3), (6, 3)] {
            assert_eq!(ch(x, y), '\u{25C6}', "corner at ({x}, {y})");
        }
        assert_eq!(ch(3, 1), '\u{2550}', "top edge");
        assert_eq!(ch(2, 3), '\u{2550}', "bottom edge");
        assert_eq!(ch(1, 2), '\u{2551}', "left edge");
        assert_eq!(ch(6, 2), '\u{2551}', "right edge");
    }

    #[test]
    fn a_frame_fills_its_inside_with_the_background_and_leaves_the_outside_alone() {
        let mut screen = Screen::new(10, 5);
        screen.clear(Colour::Blue);
        screen.frame(1, 1, 6, 3, Colour::Yellow, Colour::Black);
        let inside = screen.display().pixel(3, 2).unwrap();
        assert_eq!(
            (inside.character(), inside.background()),
            (' ', Colour::Black),
            "the inside is spaces on the background"
        );
        let border = screen.display().pixel(1, 1).unwrap();
        assert_eq!(
            (border.colour(), border.background()),
            (Colour::Yellow, Colour::Black)
        );
        let outside = screen.display().pixel(0, 0).unwrap();
        assert_eq!(outside.background(), Colour::Blue, "outside is not touched");
    }

    #[test]
    fn a_frame_erases_the_pixels_drawn_before_it_inside_and_keeps_those_outside() {
        let mut screen = Screen::new(10, 5);
        for y in 0..10 {
            for x in 0..10 {
                screen.pixel(x, y, Colour::Red);
            }
        }
        screen.frame(1, 1, 6, 3, Colour::Yellow, Colour::Black);
        // the frame covers cell rows 1 to 3 and columns 1 to 6: pixel rows 2 to 7
        for y in 2..8 {
            for x in 1..7 {
                assert!(!lit(&screen, x, y), "({x}, {y}) inside the panel");
            }
        }
        assert!(lit(&screen, 0, 2) && lit(&screen, 7, 5), "beside it");
        assert!(
            lit(&screen, 3, 1) && lit(&screen, 3, 8),
            "above and below it"
        );
    }

    #[test]
    fn a_frame_hanging_off_the_top_left_corner_is_clipped_on_both_edges() {
        let mut screen = Screen::new(6, 4);
        screen.clear(Colour::Blue);
        // the frame covers columns -1 to 2 and rows -1 to 1
        screen.frame(-1, -1, 4, 3, Colour::Yellow, Colour::Black);
        let ch = |x: isize, y: isize| screen.display().pixel(x, y).unwrap().character();
        assert_eq!(ch(2, 0), '\u{2551}', "the right edge is on the screen");
        assert_eq!(ch(0, 1), '\u{2550}', "the bottom edge is on the screen");
        assert_eq!(
            ch(2, 1),
            '\u{25C6}',
            "the bottom right corner is on the screen"
        );
        assert_eq!(ch(0, 0), ' ', "the inside is filled");
        assert_eq!(
            screen.display().pixel(3, 0).unwrap().background(),
            Colour::Blue
        );
    }

    #[test]
    fn a_frame_with_extreme_positions_and_sizes_does_not_panic() {
        let mut screen = Screen::new(6, 4);
        screen.clear(Colour::Blue);
        screen.frame(
            i32::MAX - 1,
            i32::MAX - 1,
            10,
            10,
            Colour::Yellow,
            Colour::Black,
        );
        screen.frame(
            i32::MIN,
            i32::MIN,
            i32::MAX,
            i32::MAX,
            Colour::Yellow,
            Colour::Black,
        );
        assert_eq!(
            screen.display().pixel(0, 0).unwrap().background(),
            Colour::Blue,
            "the frames off the screen draw nothing"
        );
    }

    #[test]
    fn a_frame_smaller_than_two_by_two_draws_nothing() {
        let mut screen = Screen::new(6, 4);
        screen.clear(Colour::Blue);
        let before = screen.display().render();
        for (w, h) in [(1, 3), (3, 1), (0, 0), (-3, 4), (4, -3), (1, 1)] {
            screen.frame(1, 1, w, h, Colour::Yellow, Colour::Black);
        }
        assert_eq!(screen.display().render(), before);
    }

    #[test]
    fn a_frame_covering_the_whole_screen_fills_it_and_is_quick() {
        let mut screen = Screen::new(6, 4);
        screen.clear(Colour::Blue);
        screen.pixel(2, 2, Colour::Red);
        // a huge panel that starts far off the top left: only its middle is on screen
        screen.frame(
            -1_000_000,
            -1_000_000,
            i32::MAX,
            i32::MAX,
            Colour::Yellow,
            Colour::Black,
        );
        screen.finish_frame();
        assert_eq!(
            screen.display().pixel(5, 3).unwrap().background(),
            Colour::Black
        );
        assert_eq!(screen.display().render().matches('\u{25C6}').count(), 0);
    }

    #[test]
    fn pixels_drawn_after_a_frame_show_above_it() {
        let mut screen = Screen::new(10, 5);
        screen.clear(Colour::Blue);
        screen.frame(1, 1, 6, 3, Colour::Yellow, Colour::Black);
        screen.pixel(3, 4, Colour::Red); // the top half of cell (3, 2), inside the panel
        screen.finish_frame();
        let cell = screen.display().pixel(3, 2).unwrap();
        assert_eq!(
            (cell.character(), cell.colour()),
            ('\u{2580}', Colour::Red),
            "a pixel drawn after the panel is on top of it"
        );
    }
}

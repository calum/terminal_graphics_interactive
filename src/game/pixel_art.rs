//! [`PixelArt`]: small pictures made of square pixels, each one a colour.

use terminal_graphics::Colour;

/// A picture in square pixels, written as text with a legend that says which
/// colour each character is.
///
/// Draw it with [`Screen::pixel_art`](super::Screen::pixel_art). Pixels are
/// square on screen, so a picture 6 pixels wide and 4 tall covers 6 columns
/// and 2 rows. Rows are separated by `\n`; a leading newline is ignored, a
/// character not in the legend (such as a space or a dot) is transparent, and
/// short rows are padded with transparent pixels.
///
/// ```
/// use terminal_graphics::Colour;
/// use terminal_graphics_interactive::game::*;
///
/// let boat = PixelArt::new(
///     r#"
///    W
///    WW
/// RRRRRR
///  RRRR
/// "#,
///     &[('W', Colour::White), ('R', Colour::Red)],
/// );
/// assert_eq!((boat.width(), boat.height()), (6, 4));
///
/// let mut screen = Screen::new(6, 2);
/// screen.clear(Colour::Blue);
/// screen.pixel_art(0, 0, &boat);
/// screen.finish_frame();
/// // cell (3, 0) has two white pixels: a solid white cell
/// let sail = screen.display().pixel(3, 0).unwrap();
/// assert_eq!((sail.character(), sail.background()), (' ', Colour::White));
/// // cell (0, 1) has red over transparent: an upper half block in red
/// assert_eq!(screen.display().pixel(0, 1).unwrap().character(), '\u{2580}');
/// assert_eq!(screen.display().pixel(0, 1).unwrap().colour(), Colour::Red);
/// // cell (1, 1) has two red pixels: a solid red cell
/// assert_eq!(screen.display().pixel(1, 1).unwrap().background(), Colour::Red);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct PixelArt {
    width: usize,
    height: usize,
    cells: Vec<Option<Colour>>,
}

impl PixelArt {
    /// A picture from rows of text and a legend that maps characters to
    /// colours. The first legend entry for a character wins.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let sun = PixelArt::new("YY\nY.", &[('Y', Colour::Yellow)]);
    /// assert_eq!(sun.pixel(0, 1), Some(Colour::Yellow));
    /// assert_eq!(sun.pixel(1, 1), None, "a dot is transparent");
    /// ```
    pub fn new(art: &str, legend: &[(char, Colour)]) -> PixelArt {
        let art = art.strip_prefix('\n').unwrap_or(art);
        let rows: Vec<&str> = art.lines().collect();
        let width = rows
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0);
        let height = rows.len();
        let mut cells = vec![None; width * height];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                cells[y * width + x] = legend
                    .iter()
                    .find(|(known, _)| *known == ch)
                    .map(|(_, colour)| *colour);
            }
        }
        PixelArt {
            width,
            height,
            cells,
        }
    }

    /// The width in pixels: the widest row.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let art = PixelArt::new("RRR\nR", &[('R', Colour::Red)]);
    /// assert_eq!(art.width(), 3);
    /// ```
    pub fn width(&self) -> usize {
        self.width
    }

    /// The height in pixels: one per row.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let art = PixelArt::new("R\nR\nR", &[('R', Colour::Red)]);
    /// assert_eq!(art.height(), 3);
    /// ```
    pub fn height(&self) -> usize {
        self.height
    }

    /// The same picture mirrored left to right, such as a boat that sails the
    /// other way.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let arrow = PixelArt::new("R.", &[('R', Colour::Red)]);
    /// assert_eq!(arrow.flipped().pixel(1, 0), Some(Colour::Red));
    /// ```
    pub fn flipped(&self) -> PixelArt {
        let mut cells = Vec::with_capacity(self.cells.len());
        for row in self.cells.chunks(self.width.max(1)).take(self.height) {
            cells.extend(row.iter().rev());
        }
        PixelArt {
            width: self.width,
            height: self.height,
            cells,
        }
    }

    /// The colour of pixel `(x, y)`, or `None` when it is transparent or
    /// outside the picture.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let art = PixelArt::new("R.", &[('R', Colour::Red)]);
    /// assert_eq!(art.pixel(0, 0), Some(Colour::Red));
    /// assert_eq!(art.pixel(1, 0), None);
    /// ```
    pub fn pixel(&self, x: usize, y: usize) -> Option<Colour> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells[y * self.width + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_character_is_looked_up_in_the_legend() {
        let art = PixelArt::new("RB\nBR", &[('R', Colour::Red), ('B', Colour::Blue)]);
        assert_eq!((art.width(), art.height()), (2, 2));
        assert_eq!(art.pixel(0, 0), Some(Colour::Red));
        assert_eq!(art.pixel(1, 0), Some(Colour::Blue));
        assert_eq!(art.pixel(0, 1), Some(Colour::Blue));
    }

    #[test]
    fn a_character_not_in_the_legend_is_transparent_and_a_leading_newline_is_dropped() {
        let art = PixelArt::new("\nR .\n.R ", &[('R', Colour::Red)]);
        assert_eq!((art.width(), art.height()), (3, 2));
        assert_eq!(art.pixel(0, 0), Some(Colour::Red));
        assert_eq!(art.pixel(1, 0), None, "a space is transparent");
        assert_eq!(art.pixel(2, 0), None, "a dot is transparent");
        assert_eq!(art.pixel(1, 1), Some(Colour::Red));
    }

    #[test]
    fn short_rows_are_padded_with_transparent_pixels_to_the_widest_row() {
        let art = PixelArt::new("RRR\nR", &[('R', Colour::Red)]);
        assert_eq!((art.width(), art.height()), (3, 2));
        assert_eq!(art.pixel(0, 1), Some(Colour::Red));
        assert_eq!(art.pixel(2, 1), None);
    }

    #[test]
    fn flipped_mirrors_the_picture_left_to_right() {
        let art = PixelArt::new(
            "RB\n.G",
            &[
                ('R', Colour::Red),
                ('B', Colour::Blue),
                ('G', Colour::Green),
            ],
        );
        let mirrored = art.flipped();
        assert_eq!(mirrored.pixel(0, 0), Some(Colour::Blue));
        assert_eq!(mirrored.pixel(1, 0), Some(Colour::Red));
        assert_eq!(mirrored.pixel(0, 1), Some(Colour::Green));
        assert_eq!(mirrored.pixel(1, 1), None);
        assert_eq!(
            mirrored.flipped(),
            art,
            "flipping twice gives the picture back"
        );
    }

    #[test]
    fn a_pixel_outside_the_picture_is_none() {
        let art = PixelArt::new("R", &[('R', Colour::Red)]);
        assert_eq!(art.pixel(1, 0), None);
        assert_eq!(art.pixel(0, 1), None);
        assert_eq!(art.pixel(usize::MAX, usize::MAX), None);
    }
}

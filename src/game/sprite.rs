//! Sprites: small pictures made from string literals, drawn onto a [`Screen`](super::Screen).

use terminal_graphics::Colour;

/// One cell of a sprite: a character and its colour, or `None` for a
/// transparent cell that does not change what is underneath.
type Cell = Option<(char, Colour)>;

/// A picture made of characters and colours, written as text.
///
/// Rows are separated by `\n`. A space is transparent, so the background
/// shows through it. Every other character is drawn in the colour you give.
///
/// ```
/// use terminal_graphics::Colour;
/// use terminal_graphics_interactive::game::*;
///
/// let boat = Sprite::new(
///     r#"
///    |\
///    | \
///  ~~~~~~~
/// "#,
///     Colour::White,
/// );
/// assert_eq!(boat.width(), 8);
/// assert_eq!(boat.height(), 3);
/// assert_eq!(boat.cell(3, 0), Some(('|', Colour::White)));
/// assert_eq!(boat.cell(0, 0), None); // a space is transparent
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    rows: Vec<Vec<Cell>>,
    width: usize,
}

impl Sprite {
    /// Makes a sprite from rows of text. A space is transparent and every
    /// other character is drawn in `colour`. A leading and a trailing newline
    /// are ignored, so a raw string that starts on its own line works. Short
    /// rows are padded with transparent cells.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let heart = Sprite::new(
    ///     r#"
    /// <3 <3
    /// "#,
    ///     Colour::Red,
    /// );
    /// assert_eq!((heart.width(), heart.height()), (5, 1));
    /// assert_eq!(heart.cell(1, 0), Some(('3', Colour::Red)));
    /// assert_eq!(heart.cell(2, 0), None); // the space between the hearts
    /// ```
    pub fn new(art: &str, colour: Colour) -> Sprite {
        let art = art.strip_prefix('\n').unwrap_or(art);
        let mut width = 0;
        let rows = art
            .lines()
            .map(|line| {
                width = width.max(line.chars().count());
                trim_transparent_end(
                    line.chars()
                        .map(|ch| if ch == ' ' { None } else { Some((ch, colour)) })
                        .collect(),
                )
            })
            .collect();
        Sprite { rows, width }
    }

    /// The width in columns: the number of characters in the widest row.
    /// Spaces count, so `Sprite::new("a  ", c)` is 3 wide.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// assert_eq!(Sprite::new("a  \nb", Colour::Red).width(), 3);
    /// ```
    pub fn width(&self) -> usize {
        self.width
    }

    /// The character and colour at column `x`, row `y`, or `None` if that
    /// cell is transparent or outside the sprite.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let dot = Sprite::new("o", Colour::Yellow);
    /// assert_eq!(dot.cell(0, 0), Some(('o', Colour::Yellow)));
    /// assert_eq!(dot.cell(5, 5), None); // outside the sprite
    /// ```
    pub fn cell(&self, x: usize, y: usize) -> Option<(char, Colour)> {
        self.rows.get(y)?.get(x).copied().flatten()
    }

    /// A copy of this sprite where every cell showing `ch` is drawn in
    /// `colour` instead. Use it to give one sprite two colours.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let boat = Sprite::new("|#", Colour::White).paint('#', Colour::Red);
    /// assert_eq!(boat.cell(1, 0), Some(('#', Colour::Red)));
    /// assert_eq!(boat.cell(0, 0), Some(('|', Colour::White)));
    /// ```
    pub fn paint(mut self, ch: char, colour: Colour) -> Sprite {
        for cell in self.rows.iter_mut().flatten().flatten() {
            if cell.0 == ch {
                cell.1 = colour;
            }
        }
        self
    }

    /// A copy of this sprite mirrored left to right.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let arrow = Sprite::new("> ", Colour::Green).flipped();
    /// assert_eq!(arrow.cell(1, 0), Some(('<', Colour::Green))); // now points left
    /// ```
    pub fn flipped(&self) -> Sprite {
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let mut flipped = vec![None; self.width - row.len()];
                flipped.extend(
                    row.iter()
                        .rev()
                        .map(|cell| cell.map(|(ch, c)| (mirror(ch), c))),
                );
                trim_transparent_end(flipped)
            })
            .collect();
        Sprite {
            rows,
            width: self.width,
        }
    }

    /// The number of rows. An empty string gives 0.
    ///
    /// ```
    /// use terminal_graphics::Colour;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// assert_eq!(Sprite::new("a\nb\nc", Colour::Red).height(), 3);
    /// assert_eq!(Sprite::new("", Colour::Red).height(), 0);
    /// ```
    pub fn height(&self) -> usize {
        self.rows.len()
    }
}

/// Drops transparent cells from the end of a row. Rows are stored this way so
/// that two sprites showing the same picture compare equal.
fn trim_transparent_end(mut row: Vec<Cell>) -> Vec<Cell> {
    while let Some(None) = row.last() {
        row.pop();
    }
    row
}

/// The character that reads as the same shape when the picture is mirrored.
/// Characters with no mirror image are returned unchanged.
fn mirror(ch: char) -> char {
    match ch {
        '/' => '\\',
        '\\' => '/',
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_and_a_trailing_newline_are_dropped() {
        let sprite = Sprite::new("\nab\ncd\n", Colour::Red);
        assert_eq!(sprite.height(), 2);
        assert_eq!(sprite.cell(0, 0), Some(('a', Colour::Red)));
    }

    #[test]
    fn rows_of_different_lengths_are_padded_with_transparent_cells() {
        let sprite = Sprite::new("abc\nd", Colour::Red);
        assert_eq!(sprite.width(), 3);
        assert_eq!(sprite.cell(0, 1), Some(('d', Colour::Red)));
        assert_eq!(sprite.cell(2, 1), None);
    }

    #[test]
    fn a_space_is_transparent_and_any_other_character_takes_the_colour() {
        let sprite = Sprite::new("a b", Colour::Blue);
        assert_eq!(sprite.cell(0, 0), Some(('a', Colour::Blue)));
        assert_eq!(sprite.cell(1, 0), None);
    }

    #[test]
    fn the_width_counts_characters_not_bytes() {
        let sprite = Sprite::new("héé", Colour::Red);
        assert_eq!(sprite.width(), 3);
        assert_eq!(sprite.cell(2, 0), Some(('é', Colour::Red)));
    }

    #[test]
    fn paint_recolours_only_the_cells_with_that_character() {
        let sprite = Sprite::new("a/a", Colour::White).paint('a', Colour::Red);
        assert_eq!(sprite.cell(0, 0), Some(('a', Colour::Red)));
        assert_eq!(sprite.cell(1, 0), Some(('/', Colour::White)));
    }

    #[test]
    fn flipped_mirrors_left_to_right_and_keeps_short_rows_on_their_side() {
        let sprite = Sprite::new("abc\nd", Colour::Red).flipped();
        assert_eq!(sprite.cell(0, 0), Some(('c', Colour::Red)));
        assert_eq!(sprite.cell(2, 0), Some(('a', Colour::Red)));
        assert_eq!(sprite.cell(0, 1), None);
        assert_eq!(sprite.cell(2, 1), Some(('d', Colour::Red)));
    }

    #[test]
    fn flipped_swaps_mirror_pairs_so_the_art_still_reads_right() {
        let sprite = Sprite::new("/()[]{}<>\\", Colour::Red).flipped();
        assert_eq!(sprite, Sprite::new("/<>{}[]()\\", Colour::Red));
    }

    #[test]
    fn flipping_twice_gives_back_an_equal_sprite() {
        let sprite = Sprite::new("abc\nd", Colour::Red);
        assert_eq!(sprite.flipped().flipped(), sprite);
    }

    #[test]
    fn a_sprite_of_only_spaces_keeps_its_size_but_draws_nothing() {
        let sprite = Sprite::new("   \n   ", Colour::Red);
        assert_eq!((sprite.width(), sprite.height()), (3, 2));
        assert_eq!(sprite.cell(1, 1), None);
    }

    #[test]
    fn an_empty_string_has_no_rows_and_no_cells() {
        let sprite = Sprite::new("", Colour::Red);
        assert_eq!(sprite.width(), 0);
        assert_eq!(sprite.height(), 0);
        assert_eq!(sprite.cell(0, 0), None);
    }
}

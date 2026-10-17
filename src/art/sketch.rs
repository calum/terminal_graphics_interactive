//! Formula art: colour every cell from a formula, every frame.

use super::palette::Palette;
use super::stage::{run_loop, Pointer};
use crate::Display;
use std::io;

/// What a formula is told about the cell it is colouring.
///
/// `x` is the column. `y` is the row stretched by two, so one unit looks as
/// tall as one unit of `x` is wide (terminal cells are about twice as tall as
/// they are wide) and circles come out round. The mouse position and the
/// window size use the same units.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Column of this cell.
    pub x: f32,
    /// Row of this cell, stretched so it matches `x`.
    pub y: f32,
    /// Seconds since the sketch started.
    pub t: f32,
    /// Column the mouse is over.
    pub mouse_x: f32,
    /// Row the mouse is over, stretched like `y`.
    pub mouse_y: f32,
    /// Whether the left mouse button is held.
    pub mouse_down: bool,
    /// Width of the window.
    pub width: f32,
    /// Height of the window, stretched like `y`.
    pub height: f32,
}

impl Cell {
    /// Distance from this cell to the mouse.
    pub fn dist_to_mouse(&self) -> f32 {
        ((self.x - self.mouse_x).powi(2) + (self.y - self.mouse_y).powi(2)).sqrt()
    }
}

/// Draws one frame of a sketch into `display`, with no terminal: every cell is
/// coloured from `formula`, which returns a number from -1.0 to 1.0.
///
/// [`sketch`] calls this for you every frame. Call it yourself to test a
/// formula, to draw into a `Display` you print some other way, or to measure
/// how fast a formula is.
///
/// ```
/// use terminal_graphics_interactive::{sketch_frame, Display, Palette, Pointer};
///
/// let mut display = Display::new(20, 6);
/// sketch_frame(&mut display, &Palette::ocean(), 0.0, &Pointer::default(), |cell| {
///     (cell.x * 0.3).sin()
/// });
/// assert_ne!(display.pixel(0, 0), display.pixel(5, 0));
/// ```
pub fn sketch_frame(
    display: &mut Display,
    palette: &Palette,
    t: f32,
    pointer: &Pointer,
    mut formula: impl FnMut(&Cell) -> f32,
) {
    let (width, height) = display.size();
    let mut cell = Cell {
        x: 0.0,
        y: 0.0,
        t,
        mouse_x: pointer.x,
        mouse_y: pointer.y * 2.0,
        mouse_down: pointer.left,
        width: width as f32,
        height: height as f32 * 2.0,
    };
    for row in 0..height {
        for column in 0..width {
            cell.x = column as f32;
            cell.y = row as f32 * 2.0;
            let (block, foreground, background) = palette.shade_signed(formula(&cell));
            display.set_pixel(column as isize, row as isize, block, foreground, background);
        }
    }
}

/// Fills the window every frame from a formula, until the user presses `q`.
pub fn sketch(formula: impl FnMut(&Cell) -> f32) -> io::Result<()> {
    sketch_with(Palette::ocean(), formula)
}

/// Like [`sketch`], with a choice of [`Palette`].
pub fn sketch_with(palette: Palette, mut formula: impl FnMut(&Cell) -> f32) -> io::Result<()> {
    run_loop(|display, t, pointer| sketch_frame(display, &palette, t, pointer, &mut formula))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics::Colour;

    fn pointer(x: f32, y: f32, left: bool) -> Pointer {
        Pointer {
            x,
            y,
            left,
            ..Pointer::default()
        }
    }

    #[test]
    fn distance_to_the_mouse_is_the_straight_line_distance() {
        let cell = Cell {
            x: 3.0,
            y: 4.0,
            t: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(cell.dist_to_mouse(), 5.0);
    }

    #[test]
    fn the_formula_runs_once_for_every_cell() {
        let mut display = Display::new(7, 3);
        let mut calls = 0;
        sketch_frame(
            &mut display,
            &Palette::ocean(),
            0.0,
            &Pointer::default(),
            |_| {
                calls += 1;
                0.0
            },
        );
        assert_eq!(calls, 21);
    }

    #[test]
    fn x_is_the_column_and_y_is_the_row_stretched_by_two() {
        let mut display = Display::new(4, 3);
        let mut seen = Vec::new();
        sketch_frame(
            &mut display,
            &Palette::ocean(),
            0.0,
            &Pointer::default(),
            |cell| {
                seen.push((cell.x, cell.y));
                0.0
            },
        );
        assert_eq!(seen.first(), Some(&(0.0, 0.0)));
        assert_eq!(seen[1], (1.0, 0.0), "columns run along a row");
        assert_eq!(seen[4], (0.0, 2.0), "the next row is two units down");
        assert_eq!(seen.last(), Some(&(3.0, 4.0)));
    }

    #[test]
    fn the_formula_is_told_the_time_the_mouse_and_the_window_size() {
        let mut display = Display::new(10, 5);
        let mut told = None;
        sketch_frame(
            &mut display,
            &Palette::ocean(),
            1.5,
            &pointer(6.0, 3.0, true),
            |cell| {
                told = Some(cell.clone());
                0.0
            },
        );
        let cell = told.unwrap();
        assert_eq!(cell.t, 1.5);
        assert_eq!(
            (cell.mouse_x, cell.mouse_y),
            (6.0, 6.0),
            "mouse y is stretched too"
        );
        assert!(cell.mouse_down);
        assert_eq!((cell.width, cell.height), (10.0, 10.0));
    }

    #[test]
    fn what_the_formula_returns_becomes_the_cell_colour() {
        let palette = Palette::fire();
        let mut display = Display::new(2, 1);
        let mut value = -1.0;
        sketch_frame(&mut display, &palette, 0.0, &Pointer::default(), |_| {
            let this = value;
            value = 1.0;
            this
        });
        for (x, expected) in [
            (0, palette.shade_signed(-1.0)),
            (1, palette.shade_signed(1.0)),
        ] {
            let pixel = display.pixel(x, 0).unwrap();
            assert_eq!(
                (pixel.character(), pixel.colour(), pixel.background()),
                expected
            );
        }
    }

    #[test]
    fn nan_and_infinite_results_are_clamped_not_a_crash() {
        let palette = Palette::ocean();
        let mut display = Display::new(3, 1);
        let mut results = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY].into_iter();
        sketch_frame(&mut display, &palette, 0.0, &Pointer::default(), |_| {
            results.next().unwrap()
        });
        let shades: Vec<_> = (0..3)
            .map(|x| {
                let p = display.pixel(x, 0).unwrap();
                (p.character(), p.colour(), p.background())
            })
            .collect();
        assert_eq!(shades[0], palette.shade_signed(-1.0));
        assert_eq!(shades[1], palette.shade_signed(1.0));
        assert_eq!(shades[2], palette.shade_signed(-1.0));
    }

    #[test]
    fn a_window_with_no_cells_never_calls_the_formula() {
        let mut calls = 0;
        for (w, h) in [(0, 0), (0, 4), (4, 0)] {
            let mut display = Display::new(w, h);
            sketch_frame(
                &mut display,
                &Palette::mono(),
                0.0,
                &Pointer::default(),
                |_| {
                    calls += 1;
                    0.0
                },
            );
        }
        assert_eq!(calls, 0);
    }

    #[test]
    fn it_paints_with_the_palette_colours_only() {
        let palette = Palette::new(&[Colour::Blue, Colour::Red]);
        let mut display = Display::new(5, 5);
        sketch_frame(&mut display, &palette, 2.0, &Pointer::default(), |cell| {
            (cell.x + cell.y).sin()
        });
        for y in 0..5 {
            for x in 0..5 {
                let pixel = display.pixel(x, y).unwrap();
                assert!(palette.colours().contains(&pixel.colour()));
                assert!(palette.colours().contains(&pixel.background()));
            }
        }
    }
}

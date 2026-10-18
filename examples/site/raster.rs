//! Turns a `Display` into pixels: what the README GIFs show.
//!
//! Each cell is `CELL_WIDTH` by `CELL_HEIGHT` pixels. The shade blocks the art
//! API draws (`░ ▒ ▓ █`) are blends of the foreground over the background, the
//! same way the website paints them, and a mouse cursor and click rings can be
//! drawn on top so a GIF can show what the mouse is doing.

use std::collections::HashMap;
use terminal_graphics_interactive::Display;

pub const CELL_WIDTH: usize = 8;
pub const CELL_HEIGHT: usize = 16;

pub type Rgb = [u8; 3];

/// A picture: `width * height` colours, row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Rgb>,
}

impl Raster {
    pub fn new(width: usize, height: usize, colour: Rgb) -> Raster {
        Raster {
            width,
            height,
            pixels: vec![colour; width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.pixels[y * self.width + x]
    }

    /// Sets a pixel; does nothing outside the picture.
    pub fn set(&mut self, x: i32, y: i32, colour: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = colour;
        }
    }
}

/// `foreground` covering `quarters` quarters of the way over `background`.
pub fn blend(foreground: Rgb, background: Rgb, quarters: u32) -> Rgb {
    let quarters = quarters.min(4);
    let mix = |f: u8, b: u8| ((f as u32 * quarters + b as u32 * (4 - quarters) + 2) / 4) as u8;
    [
        mix(foreground[0], background[0]),
        mix(foreground[1], background[1]),
        mix(foreground[2], background[2]),
    ]
}

fn rgb(colour: terminal_graphics_interactive::Colour) -> Rgb {
    let (r, g, b) = colour.rgb();
    [r, g, b]
}

/// Draws a display, one `CELL_WIDTH` by `CELL_HEIGHT` rectangle per cell.
pub fn render(display: &Display) -> Raster {
    let (columns, rows) = display.size();
    let (columns, rows) = (columns as usize, rows as usize);
    let mut raster = Raster::new(columns * CELL_WIDTH, rows * CELL_HEIGHT, [0, 0, 0]);

    for row in 0..rows {
        for column in 0..columns {
            let Some(pixel) = display.pixel(column as isize, row as isize) else {
                continue;
            };
            let (foreground, background) = (rgb(pixel.colour()), rgb(pixel.background()));
            let quarters = match pixel.character() {
                ' ' => Some(0),
                '░' => Some(1),
                '▒' => Some(2),
                '▓' => Some(3),
                '█' => Some(4),
                _ => None,
            };
            let fill = blend(foreground, background, quarters.unwrap_or(0));
            let (left, top) = (column * CELL_WIDTH, row * CELL_HEIGHT);
            // the half blocks of the pixel layer: the colour fills one half
            let half = match pixel.character() {
                '▀' => Some(true),
                '▄' => Some(false),
                _ => None,
            };
            for y in top..top + CELL_HEIGHT {
                let colour = match half {
                    Some(upper) if (y - top < CELL_HEIGHT / 2) == upper => foreground,
                    Some(_) => background,
                    None => fill,
                };
                for x in left..left + CELL_WIDTH {
                    raster.pixels[y * raster.width + x] = colour;
                }
            }
            // a letter or symbol is only a dot: enough to see something is there
            if quarters.is_none() && half.is_none() {
                for y in top + 5..top + 11 {
                    for x in left + 2..left + 6 {
                        raster.pixels[y * raster.width + x] = foreground;
                    }
                }
            }
        }
    }
    raster
}

/// What the mouse is doing, for the cursor that is drawn on top.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mouse {
    /// The tip of the arrow, in pixels from the top left.
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
}

/// The arrow: `#` is outline, `.` is fill, a space is see-through.
pub const ARROW: [&str; 19] = [
    "#           ",
    "##          ",
    "#.#         ",
    "#..#        ",
    "#...#       ",
    "#....#      ",
    "#.....#     ",
    "#......#    ",
    "#.......#   ",
    "#........#  ",
    "#.....#####.",
    "#..#..#     ",
    "#.# #..#    ",
    "##  #..#    ",
    "#    #..#   ",
    "     #..#   ",
    "      #..#  ",
    "      #..#  ",
    "       ##   ",
];

/// Draws the mouse arrow with its tip at the mouse position. The fill turns
/// yellow while the left button is held and cyan for the right one.
pub fn draw_cursor(raster: &mut Raster, mouse: Mouse) {
    if !mouse.x.is_finite() || !mouse.y.is_finite() {
        return;
    }
    let fill = if mouse.left {
        [255, 230, 0]
    } else if mouse.right {
        [0, 230, 230]
    } else {
        [255, 255, 255]
    };
    let (left, top) = (mouse.x.round() as i32, mouse.y.round() as i32);
    for (dy, row) in ARROW.iter().enumerate() {
        for (dx, character) in row.chars().enumerate() {
            match character {
                '#' => raster.set(left + dx as i32, top + dy as i32, [0, 0, 0]),
                '.' => raster.set(left + dx as i32, top + dy as i32, fill),
                _ => {}
            }
        }
    }
}

/// Draws a ring of `radius` pixels around `(x, y)`, `strength` (0.0 to 1.0)
/// of the way from the picture to `colour`.
pub fn draw_ring(raster: &mut Raster, x: f32, y: f32, radius: f32, colour: Rgb, strength: f32) {
    let sane = x.is_finite() && y.is_finite() && radius.is_finite() && strength.is_finite();
    if !sane || !(0.0..=10_000.0).contains(&radius) || strength <= 0.0 {
        return;
    }
    let strength = strength.min(1.0);

    // only look at the part of the picture the ring could touch
    let reach = radius + 2.0;
    let clamp = |value: f32, limit: usize| value.clamp(0.0, limit as f32) as usize;
    let (x0, x1) = (
        clamp(x - reach, raster.width),
        clamp(x + reach + 1.0, raster.width),
    );
    let (y0, y1) = (
        clamp(y - reach, raster.height),
        clamp(y + reach + 1.0, raster.height),
    );

    for py in y0..y1 {
        for px in x0..x1 {
            let distance = ((px as f32 - x).powi(2) + (py as f32 - y).powi(2)).sqrt();
            if (distance - radius).abs() <= 1.0 {
                let under = raster.get(px, py);
                let mixed = |c: usize| {
                    (under[c] as f32 + (colour[c] as f32 - under[c] as f32) * strength).round()
                        as u8
                };
                raster.set(px as i32, py as i32, [mixed(0), mixed(1), mixed(2)]);
            }
        }
    }
}

fn distance_squared(a: Rgb, b: Rgb) -> i32 {
    (0..3).map(|c| (a[c] as i32 - b[c] as i32).pow(2)).sum()
}

/// Picks at most 256 colours for a set of pictures and expresses each picture
/// as indexes into them. Colours beyond 256 become the nearest popular one.
/// The result depends only on the pictures, never on hash ordering.
pub fn quantize(rasters: &[Raster]) -> (Vec<Rgb>, Vec<Vec<u8>>) {
    // every colour, in the order it first appears, with how often it is used
    let mut counts: HashMap<Rgb, usize> = HashMap::new();
    let mut order: Vec<Rgb> = Vec::new();
    for raster in rasters {
        for pixel in &raster.pixels {
            let count = counts.entry(*pixel).or_insert(0);
            if *count == 0 {
                order.push(*pixel);
            }
            *count += 1;
        }
    }

    // keep the 256 most used, in order of first appearance
    let mut palette = order.clone();
    if palette.len() > 256 {
        let mut by_use: Vec<Rgb> = order.clone();
        by_use.sort_by_key(|colour| (std::cmp::Reverse(counts[colour]), *colour));
        by_use.truncate(256);
        palette = order
            .iter()
            .copied()
            .filter(|c| by_use.contains(c))
            .collect();
    }

    let mut index_of: HashMap<Rgb, u8> = palette
        .iter()
        .enumerate()
        .map(|(i, colour)| (*colour, i as u8))
        .collect();
    for colour in &order {
        if !index_of.contains_key(colour) {
            let nearest = (0..palette.len())
                .min_by_key(|&i| (distance_squared(palette[i], *colour), i))
                .expect("a palette");
            index_of.insert(*colour, nearest as u8);
        }
    }

    let frames = rasters
        .iter()
        .map(|raster| raster.pixels.iter().map(|pixel| index_of[pixel]).collect())
        .collect();
    (palette, frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics_interactive::Colour;

    const RED: Rgb = [200, 0, 0];
    const BLUE: Rgb = [0, 0, 200];

    fn display_with(character: char, fg: Colour, bg: Colour) -> Display {
        let mut display = Display::new(2, 1);
        display.set_pixel(0, 0, character, fg, bg);
        display
    }

    #[test]
    fn half_blocks_fill_half_the_cell_with_the_colour_and_half_with_the_background() {
        use terminal_graphics_interactive::Colour;
        let (red, blue) = (rgb(Colour::Red), rgb(Colour::Blue));
        let top = render(&display_with('▀', Colour::Red, Colour::Blue));
        let bottom = render(&display_with('▄', Colour::Red, Colour::Blue));
        for y in 0..CELL_HEIGHT {
            let (want_top, want_bottom) = if y < CELL_HEIGHT / 2 {
                (red, blue)
            } else {
                (blue, red)
            };
            assert_eq!(top.get(3, y), want_top, "upper half block, row {y}");
            assert_eq!(bottom.get(3, y), want_bottom, "lower half block, row {y}");
        }
    }
    #[test]
    fn blending_goes_from_all_background_to_all_foreground_in_quarters() {
        assert_eq!(blend(RED, BLUE, 0), BLUE);
        assert_eq!(blend(RED, BLUE, 4), RED);
        assert_eq!(blend(RED, BLUE, 2), [100, 0, 100]);
        assert_eq!(blend(RED, BLUE, 1), [50, 0, 150]);
        assert_eq!(blend(RED, BLUE, 3), [150, 0, 50]);
    }

    #[test]
    fn blending_rounds_to_the_nearest_value() {
        assert_eq!(blend([255, 255, 255], [0, 0, 0], 1), [64, 64, 64]);
        assert_eq!(blend([255, 255, 255], [0, 0, 0], 3), [191, 191, 191]);
    }

    #[test]
    fn a_picture_is_one_rectangle_per_cell() {
        let raster = render(&Display::new(5, 3));
        assert_eq!(raster.width, 5 * CELL_WIDTH);
        assert_eq!(raster.height, 3 * CELL_HEIGHT);
        assert_eq!(raster.pixels.len(), raster.width * raster.height);
    }

    #[test]
    fn the_shade_blocks_blend_the_foreground_over_the_background() {
        let (fg, bg) = (Colour::BrightRed.rgb(), Colour::Blue.rgb());
        let fg = [fg.0, fg.1, fg.2];
        let bg = [bg.0, bg.1, bg.2];
        for (character, quarters) in [(' ', 0), ('░', 1), ('▒', 2), ('▓', 3), ('█', 4)] {
            let raster = render(&display_with(character, Colour::BrightRed, Colour::Blue));
            let expected = blend(fg, bg, quarters);
            // every pixel of the first cell, corners included
            for (x, y) in [
                (0, 0),
                (CELL_WIDTH - 1, 0),
                (0, CELL_HEIGHT - 1),
                (3, 7),
                (CELL_WIDTH - 1, CELL_HEIGHT - 1),
            ] {
                assert_eq!(raster.get(x, y), expected, "{character:?} at {x},{y}");
            }
        }
    }

    #[test]
    fn neighbouring_cells_do_not_bleed_into_each_other() {
        let raster = render(&display_with('█', Colour::BrightRed, Colour::Blue));
        let default_cell = raster.get(CELL_WIDTH + 2, 2);
        assert_ne!(default_cell, raster.get(2, 2));
        assert_eq!(raster.get(CELL_WIDTH, 0), default_cell);
        assert_eq!(raster.get(CELL_WIDTH - 1, 0), raster.get(0, 0));
    }

    #[test]
    fn a_letter_or_other_character_is_a_dot_of_the_foreground_in_the_middle() {
        let raster = render(&display_with('A', Colour::BrightGreen, Colour::Black));
        let fg = Colour::BrightGreen.rgb();
        let fg = [fg.0, fg.1, fg.2];
        assert_eq!(raster.get(CELL_WIDTH / 2, CELL_HEIGHT / 2), fg);
        assert_eq!(raster.get(0, 0), [0, 0, 0]);
    }

    #[test]
    fn an_empty_display_is_an_empty_picture() {
        let raster = render(&Display::new(0, 0));
        assert_eq!((raster.width, raster.height), (0, 0));
        assert!(raster.pixels.is_empty());
    }

    #[test]
    fn the_arrow_sprite_is_a_clean_rectangle_of_known_characters() {
        for row in ARROW {
            assert_eq!(row.chars().count(), 12, "{row:?}");
            assert!(row.chars().all(|c| matches!(c, '#' | '.' | ' ')), "{row:?}");
        }
        assert!(ARROW[0].starts_with('#'), "the tip is the top left");
    }

    #[test]
    fn the_cursor_tip_sits_exactly_on_the_mouse_position() {
        let mut raster = Raster::new(60, 60, [10, 20, 30]);
        draw_cursor(
            &mut raster,
            Mouse {
                x: 20.0,
                y: 30.0,
                ..Mouse::default()
            },
        );
        assert_eq!(raster.get(20, 30), [0, 0, 0], "outline at the tip");
        assert_eq!(raster.get(21, 32), [255, 255, 255], "white inside");
        assert_eq!(raster.get(19, 30), [10, 20, 30], "nothing left of the tip");
        assert_eq!(raster.get(20, 29), [10, 20, 30], "nothing above the tip");
    }

    #[test]
    fn the_cursor_fill_shows_which_button_is_held() {
        let fill_at = |left, right| {
            let mut raster = Raster::new(60, 60, [10, 20, 30]);
            draw_cursor(
                &mut raster,
                Mouse {
                    x: 20.0,
                    y: 30.0,
                    left,
                    right,
                },
            );
            raster.get(21, 32)
        };
        let (idle, left, right) = (
            fill_at(false, false),
            fill_at(true, false),
            fill_at(false, true),
        );
        assert_eq!(idle, [255, 255, 255]);
        assert!(
            left[0] > 200 && left[1] > 200 && left[2] < 100,
            "yellow: {left:?}"
        );
        assert!(
            right[0] < 100 && right[1] > 200 && right[2] > 200,
            "cyan: {right:?}"
        );
    }

    #[test]
    fn a_cursor_partly_or_wholly_off_the_picture_is_clipped_without_panicking() {
        for (x, y) in [
            (-50.0, -50.0),
            (-5.0, 10.0),
            (58.0, 58.0),
            (500.0, 500.0),
            (f32::NAN, 5.0),
            (5.0, f32::INFINITY),
        ] {
            let mut raster = Raster::new(60, 60, [10, 20, 30]);
            draw_cursor(
                &mut raster,
                Mouse {
                    x,
                    y,
                    ..Mouse::default()
                },
            );
        }
        let mut raster = Raster::new(60, 60, [10, 20, 30]);
        draw_cursor(
            &mut raster,
            Mouse {
                x: 55.0,
                y: 55.0,
                ..Mouse::default()
            },
        );
        assert_eq!(raster.get(55, 55), [0, 0, 0], "the visible part is drawn");
    }

    #[test]
    fn a_ring_is_a_circle_that_fades_toward_the_picture() {
        let background = [0, 0, 0];
        let mut raster = Raster::new(80, 80, background);
        draw_ring(&mut raster, 40.0, 40.0, 20.0, [255, 255, 0], 1.0);
        assert_eq!(raster.get(60, 40), [255, 255, 0], "on the ring");
        assert_eq!(raster.get(20, 40), [255, 255, 0]);
        assert_eq!(raster.get(40, 40), background, "the middle is untouched");
        assert_eq!(raster.get(40, 5), background, "outside is untouched");

        let mut faint = Raster::new(80, 80, background);
        draw_ring(&mut faint, 40.0, 40.0, 20.0, [200, 100, 0], 0.5);
        assert_eq!(faint.get(60, 40), [100, 50, 0]);

        let mut none = Raster::new(80, 80, background);
        draw_ring(&mut none, 40.0, 40.0, 20.0, [255, 255, 0], 0.0);
        assert_eq!(none, Raster::new(80, 80, background));
    }

    #[test]
    fn rings_off_the_picture_or_with_odd_numbers_do_not_panic() {
        let mut raster = Raster::new(40, 40, [0, 0, 0]);
        for (x, y, r, s) in [
            (-100.0, -100.0, 5.0, 1.0),
            (500.0, 5.0, 50.0, 1.0),
            (20.0, 20.0, -3.0, 1.0),
            (20.0, 20.0, f32::NAN, 1.0),
            (20.0, 20.0, 1e9, 1.0),
            (20.0, 20.0, 10.0, f32::NAN),
            (20.0, 20.0, 10.0, 7.0),
        ] {
            draw_ring(&mut raster, x, y, r, [255, 0, 0], s);
        }
    }

    #[test]
    fn a_few_colours_are_kept_exactly() {
        let mut a = Raster::new(4, 2, [1, 2, 3]);
        a.set(0, 0, [9, 9, 9]);
        let b = Raster::new(4, 2, [9, 9, 9]);
        let (palette, frames) = quantize(&[a.clone(), b.clone()]);
        assert_eq!(palette.len(), 2);
        for (raster, indices) in [&a, &b].into_iter().zip(&frames) {
            let rebuilt: Vec<Rgb> = indices.iter().map(|&i| palette[i as usize]).collect();
            assert_eq!(rebuilt, raster.pixels);
        }
    }

    #[test]
    fn more_than_256_colours_are_reduced_to_the_nearest_popular_ones() {
        // 400 different greys; the first 300 appear once and 100 appear many times
        let mut raster = Raster::new(40, 40, [0, 0, 0]);
        for i in 0..400usize {
            let grey = (i * 255 / 399) as u8;
            raster.set((i % 40) as i32, (i / 40) as i32, [grey, grey, grey]);
        }
        for i in 0..40 {
            for y in 10..40 {
                raster.set(i, y, [(i * 6) as u8, 0, 255]);
            }
        }
        let (palette, frames) = quantize(&[raster.clone()]);
        assert!(palette.len() <= 256, "{} colours", palette.len());
        assert!(frames[0].iter().all(|&i| (i as usize) < palette.len()));
        // every pixel is close to what it was
        for (&i, original) in frames[0].iter().zip(&raster.pixels) {
            let got = palette[i as usize];
            let distance: i32 = (0..3)
                .map(|c| (got[c] as i32 - original[c] as i32).pow(2))
                .sum();
            assert!(distance < 30 * 30, "{original:?} became {got:?}");
        }
    }

    #[test]
    fn quantizing_nothing_gives_nothing() {
        let (palette, frames) = quantize(&[]);
        assert!(palette.is_empty() && frames.is_empty());
        let (palette, frames) = quantize(&[Raster::new(0, 0, [0, 0, 0])]);
        assert!(palette.is_empty());
        assert_eq!(frames, vec![Vec::<u8>::new()]);
    }

    #[test]
    fn the_same_pictures_give_the_same_palette() {
        let mut raster = Raster::new(30, 30, [0, 0, 0]);
        for i in 0..300usize {
            raster.set(
                (i % 30) as i32,
                (i / 30) as i32,
                [(i % 256) as u8, (i / 3 % 256) as u8, 7],
            );
        }
        assert_eq!(quantize(&[raster.clone()]), quantize(&[raster]));
    }
}

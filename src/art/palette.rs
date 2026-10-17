//! Palettes: turning a number into a colour.

use terminal_graphics::Colour;

/// Shade blocks from empty to full, used to blend between two colours.
const BLOCKS: [char; 5] = [' ', '░', '▒', '▓', '█'];

/// A ramp of colours from dark to bright. A number from 0.0 to 1.0 picks a
/// point along the ramp.
///
/// A terminal has only 16 colours, so a point between two colours of the ramp
/// is drawn as a shade block (`░`, `▒`, `▓` or `█`) in the brighter colour over
/// the darker one. That gives about four times as many steps as the colours
/// alone.
///
/// ```
/// use terminal_graphics_interactive::{Colour, Palette};
///
/// let palette = Palette::new(&[Colour::Black, Colour::Red, Colour::Yellow]);
/// let (block, foreground, background) = palette.shade(0.0);
/// assert_eq!((block, foreground, background), (' ', Colour::Red, Colour::Black));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    ramp: Vec<Colour>,
}

impl Palette {
    /// A palette from your own colours, dark to bright. With no colours you
    /// get [`Palette::mono`], and with one colour you get black up to it, so
    /// this never panics.
    pub fn new(colours: &[Colour]) -> Palette {
        match colours {
            [] => Palette::mono(),
            [only] => Palette::from_ramp(&[Colour::Black, *only]),
            _ => Palette::from_ramp(colours),
        }
    }

    /// Callers guarantee at least two colours, which `shade` relies on.
    fn from_ramp(ramp: &[Colour]) -> Palette {
        Palette {
            ramp: ramp.to_vec(),
        }
    }

    /// Black through blue and cyan to white.
    pub fn ocean() -> Palette {
        use Colour::*;
        Palette::from_ramp(&[Black, Blue, BrightBlue, Cyan, BrightCyan, BrightWhite])
    }

    /// Black through red and yellow to white.
    pub fn fire() -> Palette {
        use Colour::*;
        Palette::from_ramp(&[Black, Red, BrightRed, Yellow, BrightYellow, BrightWhite])
    }

    /// Every hue in turn, ending on the colour it started with.
    pub fn rainbow() -> Palette {
        use Colour::*;
        Palette::from_ramp(&[Red, Yellow, Green, Cyan, Blue, Magenta, Red])
    }

    /// Black through greys to white.
    pub fn mono() -> Palette {
        use Colour::*;
        Palette::from_ramp(&[Black, BrightBlack, White, BrightWhite])
    }

    /// The colours of the ramp, dark to bright.
    pub fn colours(&self) -> &[Colour] {
        &self.ramp
    }

    /// The character, foreground and background colour for `value`, where 0.0
    /// is the start of the ramp and 1.0 the end. Values outside that range are
    /// clamped, and NaN counts as 0.0.
    pub fn shade(&self, value: f32) -> (char, Colour, Colour) {
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        };
        let steps = self.ramp.len() - 1;
        let position = value * steps as f32;
        let index = (position.floor() as usize).min(steps - 1);
        let fraction = position - index as f32;
        let block = BLOCKS[((fraction * 4.999) as usize).min(BLOCKS.len() - 1)];
        (block, self.ramp[index + 1], self.ramp[index])
    }

    /// Like [`Palette::shade`] for a value from -1.0 to 1.0, which is what
    /// `sin` and `cos` give.
    pub fn shade_signed(&self, value: f32) -> (char, Colour, Colour) {
        self.shade((value + 1.0) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Colour::*;

    #[test]
    fn every_preset_has_at_least_two_colours() {
        for palette in [
            Palette::ocean(),
            Palette::fire(),
            Palette::rainbow(),
            Palette::mono(),
        ] {
            assert!(palette.colours().len() >= 2);
        }
    }

    #[test]
    fn the_dark_presets_start_black_and_end_bright() {
        for palette in [Palette::ocean(), Palette::fire(), Palette::mono()] {
            assert_eq!(palette.colours().first(), Some(&Black));
            assert_eq!(palette.colours().last(), Some(&BrightWhite));
        }
    }

    #[test]
    fn the_rainbow_ends_where_it_starts_so_it_loops() {
        let colours = Palette::rainbow().colours().to_vec();
        assert_eq!(colours.first(), colours.last());
        assert!(colours.len() >= 6);
    }

    #[test]
    fn a_palette_from_your_own_colours_keeps_them() {
        let palette = Palette::new(&[Blue, Green, Yellow]);
        assert_eq!(palette.colours(), &[Blue, Green, Yellow]);
    }

    #[test]
    fn a_palette_with_no_colours_is_repaired_to_mono() {
        assert_eq!(Palette::new(&[]), Palette::mono());
    }

    #[test]
    fn a_palette_with_one_colour_goes_from_black_up_to_it() {
        assert_eq!(Palette::new(&[Red]).colours(), &[Black, Red]);
    }

    #[test]
    fn the_start_of_the_ramp_is_the_first_colour_and_the_end_is_the_last() {
        let palette = Palette::new(&[Black, Red, Yellow]);
        assert_eq!(palette.shade(0.0), (' ', Red, Black));
        assert_eq!(palette.shade(1.0), ('█', Yellow, Red));
    }

    #[test]
    fn halfway_between_two_colours_is_a_mid_shade_block() {
        let palette = Palette::new(&[Black, Red]);
        let (block, foreground, background) = palette.shade(0.5);
        assert_eq!((foreground, background), (Red, Black));
        assert!(block == '▒' || block == '▓', "got {block:?}");
    }

    #[test]
    fn values_outside_zero_to_one_are_clamped_and_nan_is_zero() {
        let palette = Palette::ocean();
        assert_eq!(palette.shade(-3.0), palette.shade(0.0));
        assert_eq!(palette.shade(7.0), palette.shade(1.0));
        assert_eq!(palette.shade(f32::NAN), palette.shade(0.0));
        assert_eq!(palette.shade(f32::INFINITY), palette.shade(1.0));
        assert_eq!(palette.shade(f32::NEG_INFINITY), palette.shade(0.0));
    }

    #[test]
    fn brightness_never_goes_down_as_the_value_goes_up() {
        // ocean has no repeated colours, so a colour's place in the ramp is unambiguous
        let palette = Palette::ocean();
        let level = |value: f32| {
            let (block, _, background) = palette.shade(value);
            let step = palette
                .colours()
                .iter()
                .position(|c| *c == background)
                .unwrap();
            step * BLOCKS.len() + BLOCKS.iter().position(|b| *b == block).unwrap()
        };
        let mut last = level(0.0);
        for i in 1..=1000 {
            let now = level(i as f32 / 1000.0);
            assert!(now >= last, "went down at {i}/1000: {last} then {now}");
            last = now;
        }
        assert!(level(1.0) > level(0.0));
    }

    #[test]
    fn every_shade_uses_a_known_block_and_colours_from_the_ramp() {
        let palette = Palette::fire();
        for i in 0..=200 {
            let (block, foreground, background) = palette.shade(i as f32 / 200.0);
            assert!(BLOCKS.contains(&block));
            assert!(palette.colours().contains(&foreground));
            assert!(palette.colours().contains(&background));
        }
    }

    #[test]
    fn signed_values_map_minus_one_to_the_start_and_one_to_the_end() {
        let palette = Palette::fire();
        assert_eq!(palette.shade_signed(-1.0), palette.shade(0.0));
        assert_eq!(palette.shade_signed(0.0), palette.shade(0.5));
        assert_eq!(palette.shade_signed(1.0), palette.shade(1.0));
        assert_eq!(palette.shade_signed(f32::NAN), palette.shade_signed(-1.0));
    }
}

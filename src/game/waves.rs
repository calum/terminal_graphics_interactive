//! [`Waves`]: one sea function that both the picture and the physics read.

use super::math;
use std::f32::consts::TAU;

/// A sea made of travelling waves. Define it once, then draw the water with
/// [`Waves::shade`] and float a boat on [`Waves::surface_y`], so the boat
/// cannot sink into the picture or hover above it.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let sea = Waves::calm();
/// let level = 10.0; // the row where still water would be
/// let row = sea.surface_y(3.0, 0.5, level); // the boat sits on this row
/// assert!((row - level).abs() < 2.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waves {
    layers: [Layer; MAX_LAYERS],
    count: usize,
}

/// A layer can hold this many waves. Extra calls to [`Waves::layer`] are ignored.
const MAX_LAYERS: usize = 4;

/// Slopes smaller than this (up or down) count as level, so a boat does not
/// flicker between pictures on gentle swells.
const TILT_DEAD_ZONE: f32 = 0.25;

/// Which way a [`Waves`] surface leans at one point. See [`Waves::tilt`].
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let lean = Waves::new().tilt(0.0, 0.0);
/// assert_eq!(lean, Tilt::Level); // flat water never leans
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tilt {
    /// The surface falls to the right, so a boat leans left.
    Left,
    /// The surface is flat enough to ignore.
    Level,
    /// The surface rises to the right, so a boat leans right.
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Layer {
    amplitude: f32,
    wavelength: f32,
    speed: f32,
}

impl Default for Waves {
    /// The same as [`Waves::new`]: flat water.
    fn default() -> Waves {
        Waves::new()
    }
}

impl Waves {
    /// A sea with no waves: flat water.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Waves;
    ///
    /// assert_eq!(Waves::new().height(12.0, 3.0), 0.0);
    /// ```
    pub fn new() -> Waves {
        Waves {
            layers: [Layer {
                amplitude: 0.0,
                wavelength: 1.0,
                speed: 0.0,
            }; MAX_LAYERS],
            count: 0,
        }
    }

    /// A gentle sea: two slow swells, a few cells high at most.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Waves;
    ///
    /// let sea = Waves::calm();
    /// assert!(sea.height(5.0, 1.0).abs() <= 0.2 + 1e-5);
    /// ```
    pub fn calm() -> Waves {
        Waves::new().layer(0.15, 9.0, 0.08).layer(0.05, 4.0, 0.15)
    }

    /// A rough sea: three long, high swells running in different directions.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Waves;
    ///
    /// let sea = Waves::stormy();
    /// let mut biggest = 0.0_f32;
    /// for i in 0..100 {
    ///     biggest = biggest.max(sea.height(i as f32, 0.0).abs());
    /// }
    /// assert!(biggest > 0.5, "a storm throws the boat about");
    /// ```
    pub fn stormy() -> Waves {
        Waves::new()
            .layer(0.6, 11.0, 0.12)
            .layer(0.3, 5.0, -0.2)
            .layer(0.15, 2.2, 0.35)
    }

    /// Adds a travelling wave to the sea. `amplitude` is how high it swells
    /// (in rows), `wavelength` is the distance from crest to crest (in
    /// columns), and `speed` is how many wavelengths it travels a second
    /// (a crest moves `wavelength * speed` columns a second to the right).
    /// At most four layers count; more are ignored, and so is a layer with a
    /// wavelength that is not a positive finite number.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Waves;
    ///
    /// let sea = Waves::new().layer(0.5, 8.0, 0.1);
    /// assert!(sea.height(2.0, 0.0).abs() <= 0.5);
    /// ```
    pub fn layer(mut self, amplitude: f32, wavelength: f32, speed: f32) -> Waves {
        let usable = wavelength.is_finite()
            && wavelength > 0.0
            && amplitude.is_finite()
            && speed.is_finite();
        if self.count == MAX_LAYERS || !usable {
            return self;
        }
        self.layers[self.count] = Layer {
            amplitude,
            wavelength,
            speed,
        };
        self.count += 1;
        self
    }

    /// The height of the surface at column `x` and time `t`. Positive is up.
    pub fn height(&self, x: f32, t: f32) -> f32 {
        if !x.is_finite() || !t.is_finite() {
            return 0.0;
        }
        let total: f32 = self.layers[..self.count]
            .iter()
            .map(|layer| {
                layer.amplitude * math::sin(TAU * (x / layer.wavelength - layer.speed * t))
            })
            .filter(|value| value.is_finite())
            .sum();
        if total.is_finite() {
            total
        } else {
            0.0
        }
    }

    /// Which way the surface leans at column `x` and time `t`, so a boat
    /// sprite can pick one of three pictures. Slopes within 0.25 of flat
    /// count as [`Tilt::Level`].
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let sea = Waves::calm();
    /// let lean = sea.tilt(3.0, 0.5);
    /// assert!(matches!(lean, Tilt::Left | Tilt::Level | Tilt::Right));
    /// ```
    pub fn tilt(&self, x: f32, t: f32) -> Tilt {
        let slope = self.slope(x, t);
        if slope > TILT_DEAD_ZONE {
            Tilt::Right
        } else if slope < -TILT_DEAD_ZONE {
            Tilt::Left
        } else {
            Tilt::Level
        }
    }

    /// How far the point at row `y` is from the water, for an art formula: a
    /// number from -1 to 1, negative in the sky above the water, positive in
    /// the deep below it, and 0 on the surface. A [`Palette`](crate::Palette) takes it
    /// directly through `Palette::shade_signed`. A non-finite row or level
    /// gives 0.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let sea = Waves::calm();
    /// let level = 10.0;
    /// let surface = sea.surface_y(4.0, 0.0, level);
    /// assert!(sea.shade(4.0, surface - 3.0, 0.0, level) < 0.0); // in the sky
    /// assert!(sea.shade(4.0, surface + 3.0, 0.0, level) > 0.0); // in the deep
    /// ```
    pub fn shade(&self, x: f32, y: f32, t: f32, level: f32) -> f32 {
        if !y.is_finite() || !level.is_finite() {
            return 0.0;
        }
        ((y - self.surface_y(x, t, level)) * 0.25).clamp(-1.0, 1.0)
    }

    /// The screen row of the water surface at column `x` and time `t`, when
    /// the still water would be at row `level`. Rows grow downwards, so a
    /// high swell has a smaller row.
    pub fn surface_y(&self, x: f32, t: f32, level: f32) -> f32 {
        level - self.height(x, t)
    }

    /// How steeply the surface climbs at column `x` and time `t`: the exact
    /// derivative of [`height`](Waves::height) with respect to `x`. Positive
    /// means the surface rises to the right.
    pub fn slope(&self, x: f32, t: f32) -> f32 {
        if !x.is_finite() || !t.is_finite() {
            return 0.0;
        }
        let total: f32 = self.layers[..self.count]
            .iter()
            .map(|layer| {
                let phase = TAU * (x / layer.wavelength - layer.speed * t);
                layer.amplitude * (TAU / layer.wavelength) * math::cos(phase)
            })
            .filter(|value| value.is_finite())
            .sum();
        if total.is_finite() {
            total
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn height_and_slope_use_the_deterministic_maths_so_every_machine_agrees() {
        // one layer, so the sum is a single term and can be rebuilt exactly
        let sea = Waves::new().layer(0.7, 9.0, 0.3);
        for i in 0..2000 {
            let (x, t) = (i as f32 * 0.37, i as f32 * 0.011);
            let phase = TAU * (x / 9.0 - 0.3 * t);
            assert_eq!(
                sea.height(x, t).to_bits(),
                (0.7 * crate::game::math::sin(phase)).to_bits(),
                "height at x={x} t={t}"
            );
            assert_eq!(
                sea.slope(x, t).to_bits(),
                (0.7 * (TAU / 9.0) * crate::game::math::cos(phase)).to_bits(),
                "slope at x={x} t={t}"
            );
        }
    }

    #[test]
    fn the_default_sea_is_flat_water() {
        assert_eq!(Waves::default(), Waves::new());
    }

    #[test]
    fn flat_water_is_zero_everywhere() {
        let flat = Waves::new();
        for i in -20..=20 {
            let x = i as f32 * 1.7;
            let t = i as f32 * 0.3;
            assert_eq!(flat.height(x, t), 0.0, "at x={x} t={t}");
        }
    }

    #[test]
    fn a_single_layer_repeats_every_wavelength() {
        let sea = Waves::new().layer(0.5, 6.0, 0.2);
        for i in 0..40 {
            let x = i as f32 * 0.37;
            let here = sea.height(x, 1.3);
            let one_on = sea.height(x + 6.0, 1.3);
            assert!((here - one_on).abs() < 1e-4, "x={x}: {here} vs {one_on}");
        }
    }

    #[test]
    fn a_wave_travels_to_the_right_at_wavelength_times_speed() {
        // a crest sits at x = 1 at t = 0 (sin(pi/2) = 1); speed 0.25 cycles a
        // second on a wavelength of 4 moves it 4 * 0.25 = 1 cell a second
        let sea = Waves::new().layer(1.0, 4.0, 0.25);
        assert!((sea.height(1.0, 0.0) - 1.0).abs() < 1e-5);
        assert!(
            (sea.height(2.0, 1.0) - 1.0).abs() < 1e-5,
            "the crest moved right"
        );
        // a wave moving left would put the crest at x = 0; it is the trough here
        assert!((sea.height(0.0, 1.0) + 1.0).abs() < 1e-5, "and not left");
    }

    #[test]
    fn a_fifth_layer_is_ignored() {
        let four = Waves::new()
            .layer(0.1, 3.0, 0.1)
            .layer(0.1, 5.0, 0.2)
            .layer(0.1, 7.0, 0.3)
            .layer(0.1, 9.0, 0.4);
        let five = four.layer(5.0, 2.0, 1.0);
        assert_eq!(five, four);
    }

    #[test]
    fn a_layer_with_a_wavelength_that_is_not_positive_and_finite_is_ignored() {
        let flat = Waves::new();
        assert_eq!(flat.layer(1.0, 0.0, 0.1), flat);
        assert_eq!(flat.layer(1.0, -2.0, 0.1), flat);
        assert_eq!(flat.layer(1.0, f32::NAN, 0.1), flat);
        assert_eq!(flat.layer(1.0, f32::INFINITY, 0.1), flat);
    }

    #[test]
    fn a_layer_with_a_non_finite_amplitude_or_speed_is_ignored() {
        let flat = Waves::new();
        assert_eq!(flat.layer(f32::NAN, 2.0, 0.1), flat);
        assert_eq!(flat.layer(f32::INFINITY, 2.0, 0.1), flat);
        assert_eq!(flat.layer(1.0, 2.0, f32::NAN), flat);
        assert_eq!(flat.layer(1.0, 2.0, f32::NEG_INFINITY), flat);
    }

    #[test]
    fn shade_is_negative_above_the_water_positive_below_and_clamped() {
        let flat = Waves::new();
        let level = 10.0;
        assert_eq!(flat.shade(0.0, 10.0, 0.0, level), 0.0, "on the surface");
        assert!(flat.shade(0.0, 9.0, 0.0, level) < 0.0, "above is the sky");
        assert!(flat.shade(0.0, 11.0, 0.0, level) > 0.0, "below is the deep");
        assert!((flat.shade(0.0, 12.0, 0.0, level) - 0.5).abs() < 1e-6);
        assert_eq!(flat.shade(0.0, -1000.0, 0.0, level), -1.0);
        assert_eq!(flat.shade(0.0, 1000.0, 0.0, level), 1.0);
    }

    #[test]
    fn shade_of_a_non_finite_row_or_level_is_zero() {
        let sea = Waves::new().layer(0.5, 6.0, 0.2);
        assert_eq!(sea.shade(1.0, f32::NAN, 0.0, 10.0), 0.0);
        assert_eq!(sea.shade(1.0, f32::INFINITY, 0.0, 10.0), 0.0);
        assert_eq!(sea.shade(1.0, 5.0, 0.0, f32::NAN), 0.0);
        // a column that is not a number reads as flat water
        assert_eq!(
            sea.shade(f32::NAN, 5.0, 0.0, 10.0),
            Waves::new().shade(0.0, 5.0, 0.0, 10.0)
        );
    }

    #[test]
    fn stormy_water_swells_more_than_calm_water() {
        let swell = |sea: Waves| {
            let mut biggest = 0.0_f32;
            for i in 0..400 {
                biggest = biggest.max(sea.height(i as f32 * 0.25, 0.0).abs());
            }
            biggest
        };
        let calm = swell(Waves::calm());
        let stormy = swell(Waves::stormy());
        assert!(calm > 0.0, "calm water still moves");
        assert!(stormy > 2.0 * calm, "calm {calm}, stormy {stormy}");
    }

    #[test]
    fn tilt_picks_left_level_or_right_with_a_dead_zone() {
        // at t = 0 this layer's slope is cos(x): 1 at x = 0, -1 at x = pi
        let sea = Waves::new().layer(1.0, TAU, 0.0);
        assert_eq!(sea.tilt(0.0, 0.0), Tilt::Right);
        assert_eq!(sea.tilt(PI, 0.0), Tilt::Left);
        assert_eq!(sea.tilt(PI / 2.0, 0.0), Tilt::Level);

        let gentle = Waves::new().layer(0.2, TAU, 0.0);
        assert_eq!(
            gentle.tilt(0.0, 0.0),
            Tilt::Level,
            "slope 0.2 is inside the dead zone"
        );
        let steep = Waves::new().layer(0.3, TAU, 0.0);
        assert_eq!(steep.tilt(0.0, 0.0), Tilt::Right, "slope 0.3 is outside it");
        let steep_down = Waves::new().layer(-0.3, TAU, 0.0);
        assert_eq!(steep_down.tilt(0.0, 0.0), Tilt::Left);
    }

    #[test]
    fn the_surface_is_the_rest_row_minus_the_height() {
        let sea = Waves::new().layer(1.0, TAU, 0.0);
        // at x = pi/2 the height is 1, so the surface is one row above the rest row
        assert!((sea.surface_y(PI / 2.0, 0.0, 10.0) - 9.0).abs() < 1e-5);
        assert_eq!(
            Waves::new().surface_y(3.0, 2.0, 7.5),
            7.5,
            "flat water sits at the rest row"
        );
    }

    #[test]
    fn the_slope_matches_a_numeric_difference_of_the_height() {
        let sea = Waves::new().layer(0.5, 6.0, 0.2).layer(0.25, 2.5, -0.7);
        let e = 1e-3_f32;
        for i in 0..60 {
            let x = i as f32 * 0.41 - 7.0;
            let t = i as f32 * 0.13;
            let numeric = (sea.height(x + e, t) - sea.height(x - e, t)) / (2.0 * e);
            let analytic = sea.slope(x, t);
            assert!(
                (analytic - numeric).abs() < 2e-3,
                "x={x} t={t}: analytic {analytic} numeric {numeric}"
            );
        }
    }

    #[test]
    fn the_height_never_exceeds_the_sum_of_the_amplitudes() {
        let sea = Waves::new()
            .layer(0.5, 6.0, 0.2)
            .layer(0.25, 2.5, -0.7)
            .layer(0.1, 9.0, 0.05);
        let bound = 0.5 + 0.25 + 0.1;
        for i in 0..200 {
            for j in 0..25 {
                let h = sea.height(i as f32 * 0.31, j as f32 * 0.23);
                assert!(h.abs() <= bound + 1e-5, "height {h} at {i},{j}");
            }
        }
    }

    #[test]
    fn a_huge_phase_never_gives_a_nan_height() {
        // x / wavelength is about 1e60, which overflows f32 to infinity
        let sea = Waves::new().layer(0.5, 1e-30, 0.0);
        assert!(sea.height(1e30, 0.0).is_finite());
    }

    #[test]
    fn a_non_finite_position_or_time_gives_zero_height() {
        let sea = Waves::new().layer(0.5, 6.0, 0.2);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(sea.height(bad, 1.0), 0.0, "x = {bad}");
            assert_eq!(sea.height(1.0, bad), 0.0, "t = {bad}");
        }
    }
}

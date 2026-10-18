//! Deterministic `sin`, `cos` and `exp`.
//!
//! Why this exists: a multiplayer game runs the same simulation on every
//! machine, and the simulation must agree bit for bit. `f32::sin` and friends
//! call the platform's maths library, which may round differently on Windows,
//! macOS arm64, Linux or wasm. These functions use only IEEE basic operations
//! (`+ - * /`, comparisons, `as` casts, `floor`, `abs`, bit casts), which every
//! machine computes identically, so the same input gives the same bits
//! everywhere. Never use `mul_add`: it may fuse operations differently.
//!
//! Accuracy: `sin` and `cos` agree with the true value to within `1e-6` for
//! inputs up to about 1000 in size; `exp` has relative error below `2e-6` for
//! inputs in `[-20, 20]`. Accuracy is worse for huge inputs, but the results
//! stay finite and in range.
//!
//! ```
//! use terminal_graphics_interactive::game::math;
//!
//! assert_eq!(math::sin(0.0), 0.0);
//! assert!((math::cos(0.0) - 1.0).abs() < 1e-6);
//! assert!((math::exp(1.0) - std::f32::consts::E).abs() < 1e-5);
//! ```

use std::f32::consts::{FRAC_PI_2, PI};

// Cody-Waite split of 2*pi and ln 2. The high halves have few significant
// bits, so `k * HI` is exact for the reduction counts used here, and the low
// halves carry the remainder.
const TAU: f32 = 2.0 * PI;
const TAU_HI: f32 = 6.28125;
const TAU_LO: f32 = 0.001_935_307_2;
const LN2: f32 = std::f32::consts::LN_2;
/// ln 2 with its low 12 significant bits cleared: exactly 0.693115234375 (bits
/// 0x3f317000). The long literal is deliberate; a shorter one rounds to other bits.
#[allow(clippy::excessive_precision)]
const LN2_HI: f32 = 0.693_115_234_375;
const LN2_LO: f32 = 3.194_618_3e-5;

/// Sine of `x` (radians), the same on every machine. Non-finite input gives `0.0`.
///
/// ```
/// use terminal_graphics_interactive::game::math;
///
/// assert!((math::sin(std::f32::consts::FRAC_PI_2) - 1.0).abs() < 1e-6);
/// assert_eq!(math::sin(f32::NAN), 0.0);
/// ```
pub fn sin(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    // Work on |x| so that sin(-x) == -sin(x) bit for bit.
    let s = sin_abs(x.abs());
    if x.is_sign_negative() {
        -s
    } else {
        s
    }
}

/// Cosine of `x` (radians), the same on every machine. Non-finite input gives `0.0`.
///
/// ```
/// use terminal_graphics_interactive::game::math;
///
/// assert_eq!(math::cos(0.0), 1.0);
/// assert_eq!(math::cos(f32::INFINITY), 0.0);
/// ```
pub fn cos(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    // cos is even, so reduce |x| to r in about [-pi, pi] and use |r|.
    let r = reduce_tau(x.abs()).abs();
    let (sign, t) = if r > FRAC_PI_2 {
        (-1.0, PI - r)
    } else {
        (1.0, r)
    };
    // Even Taylor series of cos(t) for t in [0, pi/2], degree 14, in Horner form.
    let t2 = t * t;
    let p = 1.0
        + t2 * (-0.5
            + t2 * (1.0 / 24.0
                + t2 * (-1.0 / 720.0
                    + t2 * (1.0 / 40_320.0
                        + t2 * (-1.0 / 3_628_800.0
                            + t2 * (1.0 / 479_001_600.0 + t2 * (-1.0 / 87_178_291_200.0)))))));
    (sign * p).clamp(-1.0, 1.0)
}

/// `e` raised to `x`, the same on every machine.
///
/// The input is clamped to `[-87, 88]`, so very large inputs give `exp(88)`
/// and very negative inputs give `exp(-87)`. `NaN` gives `0.0`, `+inf` gives
/// `f32::MAX`, and `-inf` gives `0.0`.
///
/// ```
/// use terminal_graphics_interactive::game::math;
///
/// assert_eq!(math::exp(0.0), 1.0);
/// assert!((math::exp(2.0) - 7.389_056).abs() < 1e-4);
/// assert_eq!(math::exp(f32::NEG_INFINITY), 0.0);
/// ```
pub fn exp(x: f32) -> f32 {
    if x.is_nan() {
        return 0.0;
    }
    if x == f32::INFINITY {
        return f32::MAX;
    }
    if x == f32::NEG_INFINITY {
        return 0.0;
    }
    let x = x.clamp(-87.0, 88.0);
    // x = k ln2 + r with |r| <= ln2/2, so exp(x) = exp(r) * 2^k.
    let k = (x / LN2 + 0.5).floor();
    let r = x - k * LN2_HI - k * LN2_LO;
    // Taylor series of exp(r), degree 7, in Horner form.
    let p = 1.0
        + r * (1.0
            + r * (0.5
                + r * (1.0 / 6.0
                    + r * (1.0 / 24.0
                        + r * (1.0 / 120.0 + r * (1.0 / 720.0 + r * (1.0 / 5040.0)))))));
    let biased = k as i32 + 127;
    if biased <= 0 {
        return 0.0;
    }
    if biased >= 255 {
        return f32::MAX;
    }
    p * f32::from_bits((biased as u32) << 23)
}

/// Reduces a finite, non-negative `x` to `r` in about `[-pi, pi]` with the same
/// sine as `x`.
fn reduce_tau(x: f32) -> f32 {
    let k = (x / TAU + 0.5).floor();
    (x - k * TAU_HI) - k * TAU_LO
}

/// Sine of `x` for finite, non-negative `x`.
fn sin_abs(x: f32) -> f32 {
    sin_reduced(reduce_tau(x))
}

/// Sine of `r` for `r` in about `[-pi, pi]`: fold into `[-pi/2, pi/2]`, then a
/// degree-13 odd Taylor polynomial in Horner form.
fn sin_reduced(r: f32) -> f32 {
    let folded = if r > FRAC_PI_2 {
        PI - r
    } else if r < -FRAC_PI_2 {
        -PI - r
    } else {
        r
    };
    let r2 = folded * folded;
    let p = 1.0
        + r2 * (-1.0 / 6.0
            + r2 * (1.0 / 120.0
                + r2 * (-1.0 / 5040.0
                    + r2 * (1.0 / 362_880.0
                        + r2 * (-1.0 / 39_916_800.0 + r2 * (1.0 / 6_227_020_800.0))))));
    (folded * p).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_gives_exact_sin_cos_exp() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(exp(0.0), 1.0);
    }

    /// Every sample point of the two sweeps in the brief: [-1000, 1000] step 0.0137
    /// and [-50, 50] step 0.00123, as f32 inputs.
    fn sweep_points() -> impl Iterator<Item = f32> {
        let wide = (0..)
            .map(|i| -1000.0 + 0.0137 * i as f64)
            .take_while(|&x| x <= 1000.0);
        let narrow = (0..)
            .map(|i| -50.0 + 0.00123 * i as f64)
            .take_while(|&x| x <= 50.0);
        wide.chain(narrow).map(|x| x as f32)
    }

    #[test]
    fn sin_matches_f64_within_1e6_over_the_sweeps() {
        for x in sweep_points() {
            let got = sin(x);
            let want = (x as f64).sin();
            assert!(
                (got as f64 - want).abs() <= 1e-6,
                "sin({x}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn cos_matches_f64_within_1e6_over_the_sweeps() {
        for x in sweep_points() {
            let got = cos(x);
            let want = (x as f64).cos();
            assert!(
                (got as f64 - want).abs() <= 1e-6,
                "cos({x}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn exp_has_relative_error_below_2e6_on_minus_20_to_20() {
        let mut x = -20.0f64;
        while x <= 20.0 {
            let xf = x as f32;
            let got = exp(xf) as f64;
            let want = (xf as f64).exp();
            let rel = ((got - want) / want).abs();
            assert!(rel <= 2e-6, "exp({xf}) = {got}, want {want}, rel {rel}");
            x += 0.00123;
        }
    }

    #[test]
    fn exp_handles_non_finite_and_extreme_input() {
        assert_eq!(exp(f32::NAN), 0.0);
        assert_eq!(exp(f32::INFINITY), f32::MAX);
        assert_eq!(exp(f32::NEG_INFINITY), 0.0);
        assert!(exp(1e7).is_finite() && exp(-1e7) >= 0.0);
    }

    /// Golden bits, recorded once from this implementation (not from a libm).
    /// A CI guard: if these change, every simulation on every machine changes too.
    #[test]
    fn golden_bits_do_not_change_across_machines() {
        let cases: [(f32, u32, u32, u32); 12] = [
            (0.1, 0x3dcc_7577, 0x3f7e_b898, 0x3f8d_763e),
            (1.0, 0x3f57_6aa4, 0x3f0a_5140, 0x402d_f854),
            (2.5, 0x3f19_3579, 0xbf4d_17bf, 0x4142_eb7f),
            (100.0, 0xbf01_a12e, 0x3f5c_c0ee, 0x7ef8_82b7),
            (-37.7, 0xba69_065e, 0x3f7f_fff9, 0x2443_6a30),
            (12345.678, 0xbf34_4b08, 0x3f35_be20, 0x7ef8_82b7),
            (0.5, 0x3ef5_7744, 0x3f60_a940, 0x3fd3_094c),
            (-3.0, 0xbe10_81c9, 0xbf7d_7026, 0x3d4b_ed86),
            (7.25, 0x3f52_b56e, 0x3f11_637a, 0x44b0_035b),
            (20.0, 0x3f69_b6c7, 0x3ed0_f024, 0x4de7_5844),
            (-0.75, 0xbf2e_7fe1, 0x3f3b_4ff6, 0x3ef1_da07),
            (999.5, 0x3ee9_7d93, 0x3f63_d480, 0x7ef8_82b7),
        ];
        for (x, s, c, e) in cases {
            assert_eq!(sin(x).to_bits(), s, "sin({x})");
            assert_eq!(cos(x).to_bits(), c, "cos({x})");
            assert_eq!(exp(x).to_bits(), e, "exp({x})");
        }
    }

    #[test]
    fn sin_is_odd_bit_for_bit() {
        for x in sweep_points().step_by(20).take(1000) {
            assert_eq!(sin(-x).to_bits(), (-sin(x)).to_bits(), "x = {x}");
        }
    }

    #[test]
    fn non_finite_input_gives_finite_output_and_huge_input_stays_in_range() {
        for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(sin(x), 0.0);
            assert_eq!(cos(x), 0.0);
        }
        for x in [1e7f32, -1e7, 3.3e6, -9.9e6, 12345.678] {
            let s = sin(x);
            let c = cos(x);
            assert!(s.is_finite() && (-1.0..=1.0).contains(&s), "sin({x}) = {s}");
            assert!(c.is_finite() && (-1.0..=1.0).contains(&c), "cos({x}) = {c}");
        }
    }
}

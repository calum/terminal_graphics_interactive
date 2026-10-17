/// Turns 64 random bits into a float in `[0.0, 1.0)`, using the top 24 bits.
/// 24 bits fit exactly in an `f32`, so the largest result is `1 - 2^-24`
/// and `1.0` can never come out.
fn unit_f32(bits: u64) -> f32 {
    (bits >> 40) as f32 * (1.0 / 16_777_216.0)
}

/// A small deterministic random number generator (splitmix64).
///
/// The same seed gives the same numbers on every machine, which is what
/// replays and lockstep multiplayer need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator that starts from `seed`. The same seed always gives the
    /// same numbers.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(42);
    /// assert_eq!(rng.next_u64(), Rng::new(42).next_u64());
    /// ```
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// Next 64 bits.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(0);
    /// assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    /// ```
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A whole number in `0..n`, with every value equally likely. Values
    /// that would favour the low end are rejected and redrawn. Returns 0 when
    /// `n` is 0 (there is nothing to pick).
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        // 2^64 mod n: the number of draws that must be thrown away so the
        // accepted ones split into whole groups of n.
        let rem = ((u64::MAX % n) + 1) % n;
        loop {
            let v = self.next_u64();
            if v <= u64::MAX - rem {
                return v % n;
            }
        }
    }

    /// Next 32 bits: the high half of [`next_u64`](Rng::next_u64), which
    /// is the better-mixed half.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(0);
    /// assert_eq!(rng.next_u32(), 0xE220_A839);
    /// ```
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A float in `[0.0, 1.0)`. Use it for chances and blends.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let x = rng.f32();
    /// assert!((0.0..1.0).contains(&x));
    /// ```
    pub fn f32(&mut self) -> f32 {
        unit_f32(self.next_u64())
    }

    /// A whole number in the half-open range `lo..hi`: `lo` is possible,
    /// `hi` is not. If `hi <= lo` this returns `lo`. Every value is equally
    /// likely (rejection sampling, no modulo bias).
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let die = rng.range_i32(1, 7); // 1 to 6, like a die
    /// assert!((1..7).contains(&die));
    /// ```
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        // Computed in i64 so that i32::MIN..i32::MAX cannot overflow.
        let span = (i64::from(hi) - i64::from(lo)) as u64;
        (i64::from(lo) + self.below(span) as i64) as i32
    }

    /// True with probability `p` (0.0 is never, 1.0 is always). Values
    /// outside `0.0..=1.0` clamp to the nearest edge; NaN is never.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let mut hits = 0;
    /// for _ in 0..100 {
    ///     if rng.chance(0.25) {
    ///         hits += 1; // roughly a quarter of the time
    ///     }
    /// }
    /// assert!(hits < 100);
    /// ```
    pub fn chance(&mut self, p: f32) -> bool {
        if p.is_nan() || p <= 0.0 {
            return false;
        }
        p >= 1.0 || self.f32() < p
    }

    /// Pick one item at random, or `None` if the slice is empty.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let colour = rng.pick(&["red", "green", "blue"]).copied().unwrap_or("none");
    /// assert!(["red", "green", "blue"].contains(&colour));
    /// ```
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let index = self.below(items.len() as u64) as usize;
        items.get(index)
    }

    /// Shuffle the slice in place (Fisher-Yates). The same seed and the same
    /// slice give the same order.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let mut deck: Vec<u8> = (1..=5).collect();
    /// rng.shuffle(&mut deck);
    /// deck.sort();
    /// assert_eq!(deck, vec![1, 2, 3, 4, 5]);
    /// ```
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            items.swap(i, j);
        }
    }

    /// A new generator with its own stream, seeded from this one. The parent
    /// moves on, so forking twice gives two different streams. Use it to give
    /// a subsystem (such as particles) its own numbers, so adding randomness
    /// elsewhere does not change them.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut world = Rng::new(1234);
    /// let mut particles = world.fork();
    /// let spark = particles.range_f32(0.0, 1.0);
    /// assert!((0.0..1.0).contains(&spark));
    /// ```
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }

    /// The generator's internal state. Hash it each frame to detect when two
    /// machines in a multiplayer game have drifted apart.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let before = rng.state();
    /// rng.next_u64();
    /// assert_ne!(rng.state(), before);
    /// ```
    pub fn state(&self) -> u64 {
        self.state
    }

    /// A float in `[lo, hi)`. If `hi` is not above `lo`, or either bound is
    /// not a finite number, this returns `lo`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Rng;
    ///
    /// let mut rng = Rng::new(1234);
    /// let speed = rng.range_f32(-1.5, 1.5);
    /// assert!((-1.5..1.5).contains(&speed));
    /// ```
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        let span = hi - lo;
        if span.is_nan() || span <= 0.0 || !span.is_finite() {
            return lo;
        }
        let v = lo + self.f32() * span;
        // Rounding can land exactly on `hi`; the range is half-open.
        if v < hi {
            v
        } else {
            lo
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_stays_in_range_and_is_roughly_uniform() {
        let mut rng = Rng::new(7);
        let mut sum = 0.0_f64;
        for _ in 0..10_000 {
            let v = rng.f32();
            assert!((0.0..1.0).contains(&v), "got {v}");
            sum += f64::from(v);
        }
        let mean = sum / 10_000.0;
        assert!((mean - 0.5).abs() < 0.02, "mean was {mean}");
    }

    #[test]
    fn range_f32_stays_between_the_bounds() {
        let mut rng = Rng::new(3);
        for _ in 0..1_000 {
            let v = rng.range_f32(-2.5, 4.0);
            assert!((-2.5..4.0).contains(&v), "got {v}");
        }
    }

    #[test]
    fn range_f32_returns_lo_for_empty_or_nan_ranges() {
        let mut rng = Rng::new(3);
        assert_eq!(rng.range_f32(2.0, 2.0), 2.0);
        assert_eq!(rng.range_f32(5.0, -1.0), 5.0);
        assert_eq!(rng.range_f32(1.0, f32::NAN), 1.0);
        assert!(rng.range_f32(f32::NAN, 1.0).is_nan());
    }

    #[test]
    fn range_i32_covers_every_value_of_a_small_range_and_no_other() {
        let mut rng = Rng::new(11);
        let mut seen = [false; 5];
        for _ in 0..1_000 {
            let v = rng.range_i32(-2, 3);
            assert!((-2..3).contains(&v), "got {v}");
            seen[(v + 2) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "missing values: {seen:?}");
    }

    #[test]
    fn range_i32_returns_lo_when_the_range_is_empty() {
        let mut rng = Rng::new(5);
        assert_eq!(rng.range_i32(4, 4), 4);
        assert_eq!(rng.range_i32(4, -4), 4);
    }

    #[test]
    fn range_i32_over_the_whole_i32_range_does_not_overflow() {
        let mut rng = Rng::new(9);
        for _ in 0..1_000 {
            let v = rng.range_i32(i32::MIN, i32::MAX);
            // i32::MAX is excluded, so the top value can never be returned.
            assert!(v < i32::MAX, "got {v}");
        }
    }

    #[test]
    fn chance_at_the_edges_is_never_or_always() {
        let mut rng = Rng::new(21);
        for _ in 0..1_000 {
            assert!(!rng.chance(0.0));
            assert!(!rng.chance(-3.0));
            assert!(!rng.chance(f32::NAN));
            assert!(rng.chance(1.0));
            assert!(rng.chance(7.5));
        }
    }

    #[test]
    fn pick_from_empty_is_none_and_from_one_is_that_item() {
        let mut rng = Rng::new(4);
        let empty: [u8; 0] = [];
        assert_eq!(rng.pick(&empty), None);
        assert_eq!(rng.pick(&[7]), Some(&7));
    }

    #[test]
    fn shuffle_keeps_the_same_elements_and_is_repeatable() {
        let mut a: Vec<u32> = (0..50).collect();
        let mut b: Vec<u32> = (0..50).collect();
        Rng::new(8).shuffle(&mut a);
        Rng::new(8).shuffle(&mut b);
        assert_eq!(a, b, "same seed must give the same order");
        assert_ne!(a, (0..50).collect::<Vec<u32>>(), "50 items should move");
        a.sort();
        assert_eq!(a, (0..50).collect::<Vec<u32>>());
    }

    #[test]
    fn forked_streams_differ_from_the_parent_and_are_reproducible() {
        let mut parent = Rng::new(15);
        let mut child = parent.fork();
        let child_first = child.next_u64();
        assert_ne!(child_first, parent.next_u64());

        let mut again = Rng::new(15);
        assert_eq!(again.fork().next_u64(), child_first);
    }

    #[test]
    fn state_reports_the_seed_and_changes_as_numbers_are_drawn() {
        let mut rng = Rng::new(99);
        assert_eq!(rng.state(), 99);
        rng.next_u64();
        assert_ne!(rng.state(), 99);
    }

    #[test]
    fn different_seeds_give_different_sequences() {
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn largest_bits_give_a_value_below_one() {
        let largest = unit_f32(u64::MAX);
        assert!(largest < 1.0, "got {largest}");
        assert!(largest > 0.999);
    }

    #[test]
    fn next_u32_is_the_high_half_of_next_u64() {
        let mut a = Rng::new(0);
        let mut b = Rng::new(0);
        assert_eq!(a.next_u32(), (b.next_u64() >> 32) as u32);
    }

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn seed_zero_matches_the_reference_sequence() {
        let mut rng = Rng { state: 0 };
        let expected = [
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F,
            0xF88B_B8A8_724C_81EC,
        ];
        for want in expected {
            assert_eq!(rng.next_u64(), want);
        }
    }
}

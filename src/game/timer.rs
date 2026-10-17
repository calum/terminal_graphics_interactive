/// A cooldown or a spawn clock, counted in seconds of game time.
///
/// Feed it the step time (`dt`) once per update. A [`once`](Timer::once)
/// timer finishes once and stays finished until [`reset`](Timer::reset);
/// an [`every`](Timer::every) timer completes again and again.
#[derive(Clone, Debug, PartialEq)]
pub struct Timer {
    period: f32,
    elapsed: f32,
    repeating: bool,
}

impl Timer {
    /// A timer that completes every `seconds`, forever.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut spawn = Timer::every(0.5);
    /// assert!(spawn.tick(0.5));
    /// assert!(spawn.tick(0.5));
    /// ```
    pub fn every(seconds: f32) -> Timer {
        Timer {
            period: sanitize(seconds),
            elapsed: 0.0,
            repeating: true,
        }
    }

    /// A timer that completes once, after `seconds`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut t = Timer::once(1.0);
    /// assert!(!t.tick(0.5));
    /// assert!(t.tick(0.5));
    /// ```
    pub fn once(seconds: f32) -> Timer {
        Timer {
            period: sanitize(seconds),
            elapsed: 0.0,
            repeating: false,
        }
    }

    /// Advance by `dt` seconds. Returns true if the timer completed.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut t = Timer::every(1.0);
    /// assert!(!t.tick(0.5));
    /// assert!(t.tick(0.5));
    /// ```
    pub fn tick(&mut self, dt: f32) -> bool {
        self.ticks(dt) > 0
    }

    /// Advance by `dt` seconds and count how many times the timer completed.
    /// A once-timer gives at most 1. Time left over after a completion is
    /// kept, so long steps do not drift. At most 1000 completions are counted
    /// per call; any further time is dropped.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut spawn = Timer::every(0.5);
    /// assert_eq!(spawn.ticks(1.25), 2);
    /// assert_eq!(spawn.remaining(), 0.25);
    /// ```
    pub fn ticks(&mut self, dt: f32) -> u32 {
        if self.finished() || dt.is_nan() || dt <= 0.0 || !dt.is_finite() {
            return 0;
        }
        self.elapsed += dt;
        if !self.repeating {
            if self.elapsed >= self.period {
                self.elapsed = self.period;
                return 1;
            }
            return 0;
        }
        let whole = (self.elapsed / self.period).floor().min(MAX_TICKS as f32) as u32;
        self.elapsed = (self.elapsed - whole as f32 * self.period).clamp(0.0, self.period);
        whole
    }

    /// True once a once-timer has completed. It stays true until
    /// [`reset`](Timer::reset). A repeating timer is never finished.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut t = Timer::once(0.1);
    /// assert!(!t.finished());
    /// t.tick(0.2);
    /// assert!(t.finished());
    /// ```
    pub fn finished(&self) -> bool {
        !self.repeating && self.elapsed >= self.period
    }

    /// Time left until the next completion, never negative.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut t = Timer::once(2.0);
    /// t.tick(0.5);
    /// assert_eq!(t.remaining(), 1.5);
    /// ```
    pub fn remaining(&self) -> f32 {
        (self.period - self.elapsed).max(0.0)
    }

    /// How far through the current countdown the timer is, from 0.0 (just
    /// started) to 1.0 (complete). Handy for drawing a cooldown bar.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut reload = Timer::once(2.0);
    /// reload.tick(0.5);
    /// assert_eq!(reload.fraction(), 0.25);
    /// ```
    pub fn fraction(&self) -> f32 {
        (self.elapsed / self.period).clamp(0.0, 1.0)
    }

    /// Change the length of the countdown. Progress made so far is kept, so
    /// a cooldown can be shortened or lengthened mid-way.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut cooldown = Timer::once(1.0);
    /// cooldown.set_seconds(0.5); // a power-up halves it
    /// assert_eq!(cooldown.remaining(), 0.5);
    /// ```
    pub fn set_seconds(&mut self, seconds: f32) {
        self.period = sanitize(seconds);
    }

    /// Start the countdown again from the beginning.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Timer;
    ///
    /// let mut t = Timer::once(0.5);
    /// t.tick(1.0);
    /// t.reset();
    /// assert!(!t.finished());
    /// ```
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }
}

/// The shortest period a timer will use, so zero, negative, NaN and infinite
/// durations cannot hang a loop.
fn sanitize(seconds: f32) -> f32 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds.max(MIN_PERIOD)
    } else {
        MIN_PERIOD
    }
}

const MIN_PERIOD: f32 = 0.001;

/// The most completions one `ticks` call reports, so a huge step cannot hang.
const MAX_TICKS: u32 = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_completes_on_the_tick_that_reaches_its_time() {
        let mut t = Timer::once(0.5);
        assert!(!t.tick(0.25));
        assert!(t.tick(0.25));
    }

    #[test]
    fn every_keeps_the_leftover_time_when_ticked_by_long_steps() {
        let mut t = Timer::every(0.1);
        assert_eq!(t.ticks(0.25), 2);
        assert!(
            (t.remaining() - 0.05).abs() < 1e-4,
            "remaining {}",
            t.remaining()
        );
        // The 0.05 s carried over tips the second step into a third
        // completion. A timer that threw the leftover away would count 4.
        assert_eq!(t.ticks(0.25), 3);
    }

    #[test]
    fn tick_is_true_once_per_call_while_ticks_counts_every_period() {
        let mut t = Timer::every(0.1);
        assert!(t.tick(0.35), "a long step completes the timer");
        assert_eq!(t.ticks(0.0), 0);
        let mut u = Timer::every(0.1);
        assert_eq!(u.ticks(0.35), 3);
    }

    #[test]
    fn a_repeating_timer_never_reports_finished() {
        let mut t = Timer::every(0.1);
        t.tick(10.0);
        assert!(!t.finished());
    }

    #[test]
    fn long_run_has_no_drift() {
        let mut t = Timer::every(0.1);
        let mut total = 0;
        for _ in 0..1_000 {
            total += t.ticks(0.01);
        }
        // 1000 steps of 0.01 s is 10 s, which is 100 periods of 0.1 s.
        assert!((99..=100).contains(&total), "total {total}");
    }

    #[test]
    fn set_seconds_changes_the_period_and_keeps_progress() {
        let mut t = Timer::once(1.0);
        t.tick(0.25);
        t.set_seconds(2.0);
        assert!(
            (t.fraction() - 0.125).abs() < 1e-6,
            "fraction {}",
            t.fraction()
        );
        t.set_seconds(0.25);
        assert!(t.finished(), "progress past the new period finishes it");
    }

    #[test]
    fn degenerate_durations_and_steps_never_hang_or_panic() {
        for seconds in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut t = Timer::every(seconds);
            assert!(t.ticks(1.0) <= 1000);
            assert!(t.ticks(f32::MAX) <= 1000);
            assert!(t.ticks(f32::NAN) == 0);
            assert!(t.ticks(-5.0) == 0);
            assert!(t.remaining() >= 0.0);
            assert!(t.fraction() >= 0.0 && t.fraction() <= 1.0);

            let mut o = Timer::once(seconds);
            assert!(o.tick(f32::MAX));
            assert!(o.finished());
        }
    }

    #[test]
    fn once_stays_finished_until_reset() {
        let mut t = Timer::once(0.2);
        t.tick(1.0);
        assert!(t.finished());
        assert!(
            !t.tick(1.0),
            "a finished once-timer does not complete again"
        );
        t.reset();
        assert!(!t.finished());
        assert!(t.tick(0.5));
    }
}

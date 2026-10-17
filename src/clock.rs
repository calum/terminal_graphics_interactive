//! A fixed-timestep accumulator: turns elapsed real time into whole steps.
//!
//! This module reads no clock. The caller measures the time and passes it in,
//! which keeps the arithmetic pure and testable.

use std::time::Duration;

/// Turns elapsed real time into a whole number of fixed-size steps.
#[derive(Clone, Debug)]
pub(crate) struct FixedStep {
    step: Duration,
    max_catch_up: u32,
    accumulated: Duration,
}

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// Converts nanoseconds back to a `Duration`. Only ever called with a value
/// below a `Duration` that already exists, so the seconds always fit.
fn from_nanos(nanos: u128) -> Duration {
    Duration::new(
        (nanos / NANOS_PER_SEC) as u64,
        (nanos % NANOS_PER_SEC) as u32,
    )
}

impl FixedStep {
    /// Makes a clock that ticks every `step`. A zero `step` becomes one
    /// millisecond and a zero `max_catch_up` becomes one.
    pub(crate) fn new(step: Duration, max_catch_up: u32) -> FixedStep {
        let step = if step.is_zero() {
            Duration::from_millis(1)
        } else {
            step
        };
        FixedStep {
            step,
            max_catch_up: max_catch_up.max(1),
            accumulated: Duration::ZERO,
        }
    }

    /// Adds `elapsed` and returns how many steps to run now. Never more than
    /// `max_catch_up`; any steps beyond that are dropped, not owed later.
    pub(crate) fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulated = self.accumulated.saturating_add(elapsed);
        let step = self.step.as_nanos();
        let owed = self.accumulated.as_nanos() / step;
        let steps = owed.min(u128::from(self.max_catch_up));
        // what is left after the whole steps (and after dropping any excess)
        self.accumulated = from_nanos(self.accumulated.as_nanos() % step);
        steps as u32
    }

    /// How far through the next step we are, from 0.0 up to (not including) 1.0.
    // not used by the art loop; reserved for fixed-rate games that draw between steps
    #[allow(dead_code)]
    pub(crate) fn alpha(&self) -> f32 {
        (self.accumulated.as_nanos() as f64 / self.step.as_nanos() as f64) as f32
    }

    /// The time left before the next step is due.
    pub(crate) fn until_next(&self) -> Duration {
        self.step.saturating_sub(self.accumulated)
    }

    /// The length of one step.
    // not used by the art loop; reserved for fixed-rate games
    #[allow(dead_code)]
    pub(crate) fn step(&self) -> Duration {
        self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn less_than_one_step_of_time_returns_no_steps_and_keeps_the_remainder() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(ms(5)), 0);
        assert_eq!(clock.advance(ms(5)), 1);
    }

    #[test]
    fn exactly_one_step_of_time_returns_one_step() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(ms(10)), 1);
        assert_eq!(clock.advance(Duration::ZERO), 0);
    }

    #[test]
    fn three_and_a_half_steps_returns_three_steps_and_alpha_is_half() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(ms(35)), 3);
        assert!(
            (clock.alpha() - 0.5).abs() < 1e-6,
            "alpha {}",
            clock.alpha()
        );
    }

    #[test]
    fn the_remainder_carries_over_between_calls() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(ms(6)), 0);
        assert_eq!(clock.advance(ms(6)), 1);
        assert!(
            (clock.alpha() - 0.2).abs() < 1e-6,
            "alpha {}",
            clock.alpha()
        );
    }

    #[test]
    fn a_huge_stall_runs_at_most_max_catch_up_steps_and_drops_the_excess() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(Duration::from_secs(10)), 5);
        assert_eq!(
            clock.advance(Duration::ZERO),
            0,
            "the excess is dropped, not owed"
        );
        assert!(clock.alpha() < 1.0);
    }

    #[test]
    fn an_enormous_elapsed_time_does_not_panic() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(Duration::MAX), 5);
        assert_eq!(clock.advance(Duration::MAX), 5);
        assert!(clock.alpha() < 1.0);
    }

    #[test]
    fn a_zero_step_becomes_one_millisecond_and_zero_catch_up_becomes_one() {
        let mut clock = FixedStep::new(Duration::ZERO, 0);
        assert_eq!(clock.step(), ms(1));
        assert_eq!(clock.advance(ms(3)), 1);
    }

    #[test]
    fn until_next_is_the_step_minus_the_accumulated_time() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.until_next(), ms(10));
        clock.advance(ms(4));
        assert_eq!(clock.until_next(), ms(6));
    }

    #[test]
    fn until_next_is_never_more_than_a_step_even_after_a_stall() {
        let mut clock = FixedStep::new(ms(10), 1);
        clock.advance(Duration::from_secs(1));
        assert!(clock.until_next() <= clock.step());
        assert!(clock.until_next() > Duration::ZERO);
    }

    #[test]
    fn advancing_by_zero_returns_zero_steps() {
        let mut clock = FixedStep::new(ms(10), 5);
        assert_eq!(clock.advance(Duration::ZERO), 0);
        assert_eq!(clock.until_next(), ms(10));
    }
}

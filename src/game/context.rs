//! The [`Context`]: what a game's `update` is given each step.

use super::Keys;
use super::PlayerId;
#[cfg(test)]
use super::{Game, Headless, Next};
use crate::Pointer;

/// Everything an update needs, and nothing it should own.
///
/// A `Context` is made by the game loop (or by [`Headless`](super::Headless))
/// once per update. Read it; do not keep it, because the next update gets a
/// new one.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// struct Clock;
/// impl Game for Clock {
///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
///         // game time comes from the tick count, so it is the same on every machine
///         assert!((ctx.time() - ctx.tick as f32 / 60.0).abs() < 1e-5);
///         if ctx.tick >= 120 { Next::quit() } else { Next::stay() }
///     }
/// }
///
/// let mut game = Headless::new(Clock);
/// game.step(1000);
/// assert!(game.quit());
/// ```
pub struct Context<'a> {
    /// Which keys are pressed, held or released.
    pub keys: &'a Keys,
    /// Where the mouse pointer is, and the clicks since the last update.
    pub pointer: &'a Pointer,
    /// The length of one update in seconds: always `1.0 / 60.0`.
    pub dt: f32,
    /// How many updates ran before this one: `0` for the first.
    pub tick: u64,
    /// The size of the screen in cells, `(width, height)`.
    pub size: (u32, u32),
    /// The run's random seed. Build your own `Rng::new(ctx.seed)` once, in
    /// your first scene, and use that for all your randomness.
    pub seed: u64,
    /// How many players are in the game: `1` for local play.
    pub players: usize,
    /// Which player this machine is: `PlayerId(0)` for local play.
    pub me: PlayerId,
}

impl Context<'_> {
    /// The game time in seconds: `tick * dt`. It depends only on the tick
    /// count, so a replay sees exactly the same times.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Wave;
    /// impl Game for Wave {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         let height = (ctx.time() * 2.0).sin(); // a wave that depends on game time
    ///         assert!((-1.0..=1.0).contains(&height));
    ///         Next::stay()
    ///     }
    /// }
    /// Headless::new(Wave).step(10);
    /// ```
    pub fn time(&self) -> f32 {
        self.tick as f32 * self.dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_play_is_one_player_and_i_am_the_first() {
        use std::cell::Cell;
        use std::rc::Rc;

        struct Ask(Rc<Cell<Option<(usize, PlayerId)>>>);
        impl Game for Ask {
            fn update(&mut self, ctx: &mut Context<'_>) -> Next {
                self.0.set(Some((ctx.players, ctx.me)));
                Next::stay()
            }
        }

        let seen = Rc::new(Cell::new(None));
        Headless::new(Ask(Rc::clone(&seen))).step(1);
        assert_eq!(seen.get(), Some((1, PlayerId(0))));
    }

    #[test]
    fn time_is_the_tick_count_times_the_step() {
        let keys = Keys::new();
        let pointer = Pointer::default();
        let ctx = Context {
            keys: &keys,
            pointer: &pointer,
            dt: 0.5,
            tick: 7,
            size: (80, 24),
            seed: 1,
            players: 1,
            me: PlayerId(0),
        };
        assert_eq!(ctx.time(), 3.5);
    }
}

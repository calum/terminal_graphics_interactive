//! [`Sim`]: the rules of a game as a deterministic machine.

use super::{Decode, Encode, Hasher};
use crate::game::Rng;

/// The rules of a game, apart from how it looks. Same state, same inputs and
/// same random numbers give the same result on every machine; that is what
/// lets several machines play one game by sharing only their inputs, and lets
/// a test or a bot play with no terminal.
///
/// Keep everything that affects the rules in the `Sim`, and everything that
/// only affects the picture (sparks, screen shake) outside it. Inside `step`:
///
/// - take random numbers only from the `rng` you are given;
/// - read no clock and no window size (put the size in the state);
/// - use `game::math::{sin, cos, exp}` instead of the `f32` methods, which can
///   differ between machines in the last bit;
/// - iterate `Vec`s and arrays, not `HashMap`s.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// /// Two runners; each input is -1, 0 or 1.
/// struct Race { at: Vec<i32> }
///
/// impl Sim for Race {
///     type Input = i32;
///     fn step(&mut self, inputs: &[i32], _rng: &mut Rng) {
///         for (at, input) in self.at.iter_mut().zip(inputs) {
///             *at += input.clamp(&-1, &1);
///         }
///     }
///     fn hash(&self, h: &mut Hasher) {
///         for at in &self.at {
///             h.i32(*at);
///         }
///     }
/// }
///
/// let mut race = Race { at: vec![0, 0] };
/// let mut rng = Rng::new(1);
/// race.step(&[1, -1], &mut rng);
/// race.step(&[1, 0], &mut rng);
/// assert_eq!(race.at, [2, -1]);
/// ```
pub trait Sim {
    /// What one player does in one step. It is sent over the network, so it
    /// is small data, not key codes. `Default` is "no input": it stands in for
    /// the first few steps while real inputs are still on their way.
    type Input: Encode + Decode + Clone + Default;

    /// Advances the game by one step. `inputs[i]` is player `i`'s input.
    fn step(&mut self, inputs: &[Self::Input], rng: &mut Rng);

    /// Feeds everything that matters to the rules into `h`. Machines compare
    /// these hashes to notice that they have drifted apart, so include every
    /// field `step` reads or writes, and nothing that is only for the picture.
    fn hash(&self, h: &mut Hasher);
}

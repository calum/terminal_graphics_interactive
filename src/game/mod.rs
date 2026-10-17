//! Terminal Games: building blocks for games. Everything here is plain Rust with no terminal access so it can be tested and used headless.
//!
//! - [`Keys`]: which keys went down, are held, or went up, timed from the
//!   caller's clock.
//! - [`Controls`]: named actions bound to keys, so players can rebind them.

mod controls;
mod hit;
mod keys;
mod pool;
mod rng;
mod screen;
mod sprite;
mod timer;

pub use crate::{Event, Key, KeyCode};
pub use controls::Controls;
pub use hit::Hit;
pub use keys::Keys;
pub use pool::Pool;
pub use rng::Rng;
pub use screen::Screen;
pub use sprite::Sprite;
pub use timer::Timer;

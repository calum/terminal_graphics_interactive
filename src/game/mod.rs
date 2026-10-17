//! Terminal Games: a scene-based game loop with keys, mouse, drawing and
//! random numbers. Everything except [`run`] is plain Rust with no terminal
//! access, so games can be tested and played headless.
//!
//! A game is a [`Game`] trait object. `update` runs 60 times a second and
//! returns a [`Next`] (stay, switch, push a pause, pop, quit); `draw` paints a
//! [`Screen`]. Here is a menu that switches to a game, played headless so the
//! example runs without a terminal. In a real program, `run(Menu)` plays it.
//!
//! ```
//! use terminal_graphics_interactive::game::*;
//!
//! struct Menu;
//! struct Play { score: u32 }
//!
//! impl Game for Menu {
//!     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
//!         if ctx.keys.pressed(KeyCode::Enter) { Next::switch(Play { score: 0 }) } else { Next::stay() }
//!     }
//!     fn draw(&self, screen: &mut Screen) {
//!         screen.text_centred(screen.height() / 2, "press Enter", Colour::White);
//!     }
//! }
//!
//! impl Game for Play {
//!     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
//!         self.score += 1;
//!         Next::stay()
//!     }
//!     fn draw(&self, screen: &mut Screen) {
//!         screen.text(0, 0, &format!("score {}", self.score), Colour::Yellow);
//!     }
//! }
//!
//! // In a real program: fn main() -> std::io::Result<()> { run(Menu) }
//! let mut game = Headless::new(Menu).size(20, 3);
//! assert!(game.text().contains("press Enter"));
//! game.tap(KeyCode::Enter);
//! game.step(60);
//! assert!(game.text().contains("score 61"));
//! ```
//!
//! The pieces:
//!
//! - [`Game`], [`Next`] and [`Context`]: a screen of your game, what happens
//!   next, and what an update is given (keys, pointer, size, seed).
//! - [`run`]: the real loop on a terminal. Ctrl+C always quits; `q` and Esc
//!   belong to the game.
//! - [`Headless`]: the same loop with no terminal, for tests and AI agents.
//!   It reads what the game drew as text.
//! - [`Screen`] and [`Sprite`]: drawing. Text, fills, bars and pictures, all
//!   clipped to the screen.
//! - [`Keys`] and [`Controls`]: which keys are pressed, held or released, and
//!   named actions that players can rebind.
//! - [`Rng`], [`Timer`], [`Pool`] and [`Hit`]: random numbers, cooldowns,
//!   fixed-size object lists and collision tests.
//! - [`Tuning`]: numbers a game exposes so they can be changed while it runs
//!   (sliders on the website).
//!
//! The art modules are in scope too (`use terminal_graphics_interactive::game::*`
//! brings in `Palette`, `Pointer`, `sketch_frame`, `World`, `Display`, `Colour`
//! and the rest of the prelude).

mod context;
mod controls;
mod headless;
mod hit;
mod keys;
mod particles;
mod pixel_art;
mod pool;
mod rng;
mod run;
mod scene;
mod screen;
mod sprite;
mod timer;
mod tuning;
mod waves;

pub use crate::prelude::*;
pub use crate::{Event, Key, KeyCode};
pub use context::Context;
pub use controls::Controls;
pub use headless::Headless;
pub use hit::Hit;
pub use keys::Keys;
pub use particles::{Particle, Particles};
pub use pixel_art::PixelArt;
pub use pool::Pool;
pub use rng::Rng;
pub use run::run;
pub use scene::{Game, Next};
pub use screen::Screen;
pub use sprite::Sprite;
pub use timer::Timer;
pub use tuning::{Knob, Tuning};
pub use waves::{Tilt, Waves};

//! An easy way to make art and fun graphics: write a formula or describe a
//! little world, and the library does the terminal, the frame loop, the mouse
//! and quitting.
//!
//! - [`sketch`] fills the window every frame from a formula of the cell's
//!   position, the time and the mouse.
//! - [`World`] is a small physics playground described with chained settings.
//! - [`Palette`] turns numbers into colours; make your own with
//!   [`Palette::new`].

mod palette;
mod sketch;
mod stage;
mod world;

pub use palette::Palette;
pub(crate) use sketch::sketch_region;
pub use sketch::{sketch, sketch_frame, sketch_with, Cell};
pub use stage::Pointer;
pub use world::{Simulation, World};

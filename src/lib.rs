//! Keyboard and mouse input for [`terminal_graphics`].
#![deny(unsafe_code)]
#![warn(missing_docs)]

mod event;
mod input;
mod parser;
mod raw_mode;
mod terminal;

pub use event::{Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};
pub use parser::Parser;
pub use raw_mode::{RawMode, MOUSE_OFF, MOUSE_ON};
pub use terminal::Terminal;
pub use terminal_graphics::*;

/// Everything in one import: `use terminal_graphics_interactive::prelude::*;`
///
/// This is the `terminal_graphics` prelude (`Display`, `Colour`, `Graphics`,
/// `Rect`, `Ball`, `Text`, `Container`, ...) plus the interactive types
/// (`Terminal`, `Event`, `Key`, `KeyCode`, `Mouse`, `MouseKind`, ...).
///
/// ```
/// use terminal_graphics_interactive::prelude::*;
///
/// let mut display = Display::new(10, 3);
/// display.set_pixel(1, 1, '#', Colour::Red, Colour::Black);
/// assert!(Key::new(KeyCode::Char('q')).is_char('q'));
/// ```
pub mod prelude {
    pub use crate::{
        Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind, Parser, RawMode, Terminal,
    };
    pub use terminal_graphics::prelude::*;
}

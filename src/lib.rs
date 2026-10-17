//! Keyboard and mouse input for [`terminal_graphics`].
#![deny(unsafe_code)]
#![warn(missing_docs)]

mod event;
mod parser;
mod raw_mode;

pub use event::{Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};
pub use parser::Parser;
pub use raw_mode::{RawMode, MOUSE_OFF, MOUSE_ON};
pub use terminal_graphics::*;

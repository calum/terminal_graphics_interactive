//! The input events a terminal can send: key presses and mouse activity.

/// Modifier keys held down during an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift was held.
    pub shift: bool,
    /// Alt (Option on macOS) was held.
    pub alt: bool,
    /// Ctrl was held.
    pub ctrl: bool,
}

impl Modifiers {
    /// No modifiers held.
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        alt: false,
        ctrl: false,
    };

    /// Whether no modifier is held.
    pub fn is_none(self) -> bool {
        self == Modifiers::NONE
    }
}

/// Which key was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key. Ctrl+C arrives as `Char('c')` with `ctrl` set.
    Char(char),
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Escape.
    Esc,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Insert.
    Insert,
    /// Delete.
    Delete,
    /// A function key, `F(1)` to `F(12)`.
    F(u8),
}

/// A key press: which key, and which modifiers were held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// The key.
    pub code: KeyCode,
    /// The modifiers held.
    pub modifiers: Modifiers,
}

impl Key {
    /// A key press with no modifiers.
    pub fn new(code: KeyCode) -> Key {
        Key {
            code,
            modifiers: Modifiers::NONE,
        }
    }

    /// Whether this is the character `c` with no modifiers held.
    pub fn is_char(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.modifiers.is_none()
    }

    /// Whether this is Ctrl plus the character `c`, for example Ctrl+C.
    pub fn is_ctrl(&self, c: char) -> bool {
        self.code == KeyCode::Char(c) && self.modifiers.ctrl
    }
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The left button.
    Left,
    /// The middle button (usually the wheel).
    Middle,
    /// The right button.
    Right,
}

/// What the mouse did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseKind {
    /// A button was pressed.
    Down(MouseButton),
    /// A button was released.
    Up(MouseButton),
    /// The mouse moved with a button held.
    Drag(MouseButton),
    /// The mouse moved with no button held.
    Moved,
    /// The wheel scrolled up (away from you).
    ScrollUp,
    /// The wheel scrolled down.
    ScrollDown,
}

/// A mouse event at a position on the terminal screen.
///
/// `x` and `y` are cells counted from `(0, 0)` at the top left of the
/// *terminal*, with `x` to the right and `y` down, the same orientation as
/// [`Display`](terminal_graphics::Display) coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mouse {
    /// What happened.
    pub kind: MouseKind,
    /// Column, from 0.
    pub x: isize,
    /// Row, from 0.
    pub y: isize,
    /// The modifiers held.
    pub modifiers: Modifiers,
}

/// One input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// A key was pressed.
    Key(Key),
    /// The mouse did something.
    Mouse(Mouse),
}

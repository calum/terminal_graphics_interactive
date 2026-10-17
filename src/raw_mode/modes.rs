//! The pure parts of raw mode: flag arithmetic, escape sequences and error text.
//! Safe, and unit tested, because the real console calls cannot be tested
//! without a terminal.

use std::io;

/// Escape sequence that asks the terminal to report mouse buttons, dragging
/// and scrolling in SGR format (`1002` is button and drag tracking, `1006` is
/// the SGR encoding, which works for terminals wider than 223 columns).
pub const MOUSE_ON: &str = "\x1b[?1002h\x1b[?1006h";

/// Escape sequence that turns mouse reporting back off.
pub const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1002l";

/// Windows console flags (`SetConsoleMode`).
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) mod win_flags {
    pub const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
    pub const ENABLE_LINE_INPUT: u32 = 0x0002;
    pub const ENABLE_ECHO_INPUT: u32 = 0x0004;
    pub const ENABLE_MOUSE_INPUT: u32 = 0x0010;
    pub const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
    pub const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
    pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
    pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
}

/// The Windows console input mode that gives raw, VT-encoded input: no line
/// editing, no echo, no Ctrl+C handling (it arrives as a key instead), and no
/// quick edit, which would swallow mouse clicks to select text.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn windows_input_mode(original: u32) -> u32 {
    use win_flags::*;
    let off = ENABLE_PROCESSED_INPUT
        | ENABLE_LINE_INPUT
        | ENABLE_ECHO_INPUT
        | ENABLE_MOUSE_INPUT
        | ENABLE_QUICK_EDIT_MODE;
    (original & !off) | ENABLE_EXTENDED_FLAGS | ENABLE_VIRTUAL_TERMINAL_INPUT
}

/// The Windows console output mode that understands ANSI escape sequences.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn windows_output_mode(original: u32) -> u32 {
    original | win_flags::ENABLE_VIRTUAL_TERMINAL_PROCESSING
}

/// The error returned when stdin is not a terminal.
pub(super) fn not_a_terminal() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "stdin is not a terminal, so keyboard and mouse input cannot be read. \
         Run the program directly in a terminal (over SSH use `ssh -t`), \
         not with input redirected or piped.",
    )
}

#[cfg(test)]
mod tests {
    use super::win_flags::*;
    use super::*;

    #[test]
    fn mouse_on_enables_button_drag_and_sgr_reporting() {
        assert!(MOUSE_ON.contains("\x1b[?1002h"));
        assert!(MOUSE_ON.contains("\x1b[?1006h"));
    }

    #[test]
    fn mouse_off_undoes_exactly_what_mouse_on_did() {
        assert!(MOUSE_OFF.contains("\x1b[?1002l"));
        assert!(MOUSE_OFF.contains("\x1b[?1006l"));
    }

    #[test]
    fn windows_input_mode_asks_for_vt_input_and_drops_line_editing() {
        let mode = windows_input_mode(ENABLE_PROCESSED_INPUT | 0x2 | 0x4 | 0x40);
        assert_eq!(mode & 0x200, 0x200, "VT input on");
        assert_eq!(mode & 0x1, 0, "processed input (Ctrl+C handling) off");
        assert_eq!(mode & 0x2, 0, "line input off");
        assert_eq!(mode & 0x4, 0, "echo off");
        assert_eq!(mode & 0x40, 0, "quick edit off, it swallows mouse clicks");
        assert_eq!(
            mode & 0x80,
            0x80,
            "extended flags on, needed to change quick edit"
        );
    }

    #[test]
    fn windows_input_mode_keeps_unrelated_flags() {
        let mode = windows_input_mode(0x100);
        assert_eq!(mode & 0x100, 0x100);
    }

    #[test]
    fn windows_output_mode_turns_on_ansi_processing_and_keeps_the_rest() {
        let mode = windows_output_mode(0x1);
        assert_eq!(mode & 0x4, 0x4, "virtual terminal processing on");
        assert_eq!(mode & 0x1, 0x1, "existing flags kept");
    }

    #[test]
    fn not_a_terminal_error_says_what_to_do_next() {
        let error = not_a_terminal();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        let message = error.to_string();
        assert!(message.contains("not a terminal"), "{message}");
        assert!(message.contains("ssh -t"), "{message}");
    }
}

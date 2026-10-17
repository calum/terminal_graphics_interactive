//! Raw mode on Linux, macOS and the BSDs: three libc calls.
//!
//! This file and `windows.rs` are the only places in the crate that use
//! `unsafe`. `struct termios` differs in size and layout between systems, so
//! rather than redefining it per platform we hand libc a zeroed, aligned
//! buffer that is larger than any of them and never look inside it.
#![allow(unsafe_code)]

use super::modes::not_a_terminal;
use std::io;

const STDIN: i32 = 0;
/// `TCSANOW` is 0 on Linux, macOS and the BSDs.
const TCSANOW: i32 = 0;

/// Opaque storage for a C `struct termios` (the largest, on macOS, is 72 bytes).
#[repr(C, align(16))]
#[derive(Clone)]
struct Termios([u8; 256]);

extern "C" {
    fn isatty(fd: i32) -> i32;
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    fn cfmakeraw(termios: *mut Termios);
}

/// The terminal settings to put back when raw mode ends.
pub(super) struct Saved(Termios);

pub(super) fn enable() -> io::Result<Saved> {
    // SAFETY: isatty only inspects the file descriptor number it is given.
    if unsafe { isatty(STDIN) } != 1 {
        return Err(not_a_terminal());
    }

    let mut original = Termios([0; 256]);
    // SAFETY: `original` is a writable, 16-byte aligned buffer larger than any
    // platform's `struct termios`, which is all tcgetattr writes to.
    if unsafe { tcgetattr(STDIN, &mut original) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut raw = original.clone();
    // SAFETY: `raw` holds a `termios` that tcgetattr just filled in.
    unsafe { cfmakeraw(&mut raw) };
    // SAFETY: `raw` is a valid `termios`, as above.
    if unsafe { tcsetattr(STDIN, TCSANOW, &raw) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Saved(original))
}

pub(super) fn restore(saved: &Saved) {
    // SAFETY: `saved.0` is the `termios` tcgetattr filled in when raw mode began.
    // Nothing useful can be done if this fails while shutting down.
    unsafe { tcsetattr(STDIN, TCSANOW, &saved.0) };
}

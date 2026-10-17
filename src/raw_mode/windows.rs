//! Raw mode on Windows (PowerShell, Command Prompt, Windows Terminal): three
//! `kernel32` calls.
//!
//! This file and `unix.rs` are the only places in the crate that use `unsafe`.
//! The flag arithmetic lives in `modes.rs`, where it is unit tested.
#![allow(unsafe_code)]

use super::modes::{not_a_terminal, windows_input_mode, windows_output_mode};
use std::io;

/// `GetStdHandle` ids: `STD_INPUT_HANDLE` is -10 and `STD_OUTPUT_HANDLE` is -11.
const STD_INPUT_HANDLE: u32 = -10i32 as u32;
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const INVALID_HANDLE_VALUE: isize = -1;

#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(id: u32) -> isize;
    fn GetConsoleMode(handle: isize, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: isize, mode: u32) -> i32;
}

/// The console modes to put back when raw mode ends.
pub(super) struct Saved {
    input: (isize, u32),
    /// Absent when stdout is not a console (for example when it is piped).
    output: Option<(isize, u32)>,
}

/// The handle and current mode of a console handle, or `None` if it is not one.
fn console(id: u32) -> Option<(isize, u32)> {
    // SAFETY: GetStdHandle takes a plain id and returns a handle or an invalid value.
    let handle = unsafe { GetStdHandle(id) };
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut mode = 0u32;
    // SAFETY: `mode` is a valid, writable u32 for the call to fill in.
    if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return None;
    }
    Some((handle, mode))
}

pub(super) fn enable() -> io::Result<Saved> {
    let input = console(STD_INPUT_HANDLE).ok_or_else(not_a_terminal)?;
    let output = console(STD_OUTPUT_HANDLE);

    // SAFETY: `input.0` is a console handle that GetConsoleMode accepted above.
    if unsafe { SetConsoleMode(input.0, windows_input_mode(input.1)) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if let Some((handle, mode)) = output {
        // SAFETY: `handle` is a console handle that GetConsoleMode accepted above.
        // Failing is not fatal: older consoles just keep showing escape codes.
        unsafe { SetConsoleMode(handle, windows_output_mode(mode)) };
    }
    Ok(Saved { input, output })
}

pub(super) fn restore(saved: &Saved) {
    // SAFETY: the handles and modes were read from the console when raw mode
    // began. Nothing useful can be done if this fails while shutting down.
    unsafe { SetConsoleMode(saved.input.0, saved.input.1) };
    if let Some((handle, mode)) = saved.output {
        // SAFETY: as above.
        unsafe { SetConsoleMode(handle, mode) };
    }
}

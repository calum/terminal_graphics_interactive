//! End-to-end tests on Windows, against a real console host.
//!
//! Each test runs the `events` example attached to a pseudo console (ConPTY,
//! the same machinery behind Windows Terminal and PowerShell), writes raw key
//! and mouse escape sequences to it, and checks what the program prints.
//!
//! This proves our half of the contract: raw mode really delivers input
//! immediately and unechoed, mouse and key sequences parse to the right
//! events, and the program quits cleanly. It cannot prove that a physical
//! mouse click becomes an escape sequence; that is the terminal emulator's job.
//!
//! The pseudo console needs hand-written `kernel32` calls, so this file has
//! `unsafe`. It is test-only: nothing here is shipped with the crate.
#![cfg(windows)]
#![allow(unsafe_code, non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type HANDLE = isize;

#[repr(C)]
#[derive(Clone, Copy)]
struct COORD {
    x: i16,
    y: i16,
}

#[repr(C)]
struct STARTUPINFOW {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_input: HANDLE,
    std_output: HANDLE,
    std_error: HANDLE,
}

#[repr(C)]
struct STARTUPINFOEXW {
    startup_info: STARTUPINFOW,
    attribute_list: *mut c_void,
}

#[repr(C)]
struct PROCESS_INFORMATION {
    process: HANDLE,
    thread: HANDLE,
    process_id: u32,
    thread_id: u32,
}

const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const WAIT_TIMEOUT: u32 = 0x102;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const INVALID_HANDLE_VALUE: HANDLE = -1;

#[link(name = "kernel32")]
extern "system" {
    fn CreatePipe(read: *mut HANDLE, write: *mut HANDLE, attrs: *const c_void, size: u32) -> i32;
    fn CreatePseudoConsole(
        size: COORD,
        input: HANDLE,
        output: HANDLE,
        flags: u32,
        console: *mut HANDLE,
    ) -> i32;
    fn ClosePseudoConsole(console: HANDLE);
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *const c_void,
        size: usize,
        previous: *mut c_void,
        returned: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command_line: *mut u16,
        process_attrs: *const c_void,
        thread_attrs: *const c_void,
        inherit_handles: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const STARTUPINFOEXW,
        info: *mut PROCESS_INFORMATION,
    ) -> i32;
    fn WaitForSingleObject(handle: HANDLE, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: HANDLE, code: *mut u32) -> i32;
    fn TerminateProcess(process: HANDLE, code: u32) -> i32;
    fn ReadFile(
        file: HANDLE,
        buffer: *mut u8,
        len: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        file: HANDLE,
        buffer: *const u8,
        len: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: HANDLE) -> i32;
}

/// A child process attached to a pseudo console.
struct Pty {
    console: HANDLE,
    process: HANDLE,
    input: Arc<Mutex<HANDLE>>,
    output: Arc<Mutex<Vec<u8>>>,
}

fn events_example() -> PathBuf {
    // target/debug/deps/<test>.exe -> target/debug/examples/events.exe
    // `cargo test --test e2e_windows` does not build examples, so build it here
    static BUILD: std::sync::Once = std::sync::Once::new();
    BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = std::process::Command::new(cargo)
            .args(["build", "--example", "events"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("could not run cargo");
        assert!(status.success(), "building the events example failed");
    });

    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push("events.exe");
    assert!(path.exists(), "{} was not built", path.display());
    path
}

impl Pty {
    /// Starts `program` in a 200 column by 24 row pseudo console.
    fn spawn(program: &PathBuf) -> Pty {
        // SAFETY (whole function): plain Win32 calls with valid, live pointers;
        // every handle created here is closed in `Drop` or handed to the reader.
        unsafe {
            let (mut in_read, mut in_write, mut out_read, mut out_write) = (0, 0, 0, 0);
            assert_ne!(CreatePipe(&mut in_read, &mut in_write, null(), 0), 0);
            assert_ne!(CreatePipe(&mut out_read, &mut out_write, null(), 0), 0);

            let mut console = 0;
            let size = COORD { x: 200, y: 24 };
            assert_eq!(
                CreatePseudoConsole(size, in_read, out_write, 0, &mut console),
                0,
                "CreatePseudoConsole failed (needs Windows 10 1809 or later)"
            );
            // the pseudo console holds its own references to these ends
            CloseHandle(in_read);
            CloseHandle(out_write);

            // an attribute list that attaches the child to the pseudo console
            let mut list_size = 0usize;
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut list_size);
            let mut list_buffer = vec![0u8; list_size];
            let list = list_buffer.as_mut_ptr() as *mut c_void;
            assert_ne!(
                InitializeProcThreadAttributeList(list, 1, 0, &mut list_size),
                0
            );
            assert_ne!(
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                    console as *const c_void,
                    std::mem::size_of::<HANDLE>(),
                    null_mut(),
                    null_mut(),
                ),
                0
            );

            let mut startup: STARTUPINFOEXW = std::mem::zeroed();
            startup.startup_info.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            startup.attribute_list = list;
            // Without this the child inherits our own (redirected) standard
            // handles instead of using the pseudo console, as happens under
            // `cargo test`. Invalid handles force it onto the pseudo console.
            startup.startup_info.flags = STARTF_USESTDHANDLES;
            startup.startup_info.std_input = INVALID_HANDLE_VALUE;
            startup.startup_info.std_output = INVALID_HANDLE_VALUE;
            startup.startup_info.std_error = INVALID_HANDLE_VALUE;

            let mut command: Vec<u16> =
                std::ffi::OsString::from(format!("\"{}\"", program.display()))
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
            let mut info: PROCESS_INFORMATION = std::mem::zeroed();
            let created = CreateProcessW(
                null(),
                command.as_mut_ptr(),
                null(),
                null(),
                0,
                EXTENDED_STARTUPINFO_PRESENT,
                null(),
                null(),
                &startup,
                &mut info,
            );
            DeleteProcThreadAttributeList(list);
            assert_ne!(created, 0, "CreateProcessW failed");
            CloseHandle(info.thread);

            let input = Arc::new(Mutex::new(in_write));
            let output = Arc::new(Mutex::new(Vec::new()));

            // drain the console output, and answer the cursor position request
            // that a pseudo console sends at startup, or it can stall the child
            let (reader_output, reader_input) = (Arc::clone(&output), Arc::clone(&input));
            thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                loop {
                    let mut read = 0u32;
                    let ok = ReadFile(
                        out_read,
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut read,
                        null_mut(),
                    );
                    if ok == 0 || read == 0 {
                        break;
                    }
                    let chunk = &buffer[..read as usize];
                    if chunk.windows(4).any(|w| w == b"\x1b[6n") {
                        write_to(*reader_input.lock().unwrap(), b"\x1b[1;1R");
                    }
                    reader_output.lock().unwrap().extend_from_slice(chunk);
                }
                CloseHandle(out_read);
            });

            Pty {
                console,
                process: info.process,
                input,
                output,
            }
        }
    }

    fn send(&self, bytes: &[u8]) {
        write_to(*self.input.lock().unwrap(), bytes);
    }

    /// Everything printed so far, with escape sequences removed.
    fn text(&self) -> String {
        strip_escapes(&self.output.lock().unwrap())
    }

    /// Waits until the printed text contains `needle`, or fails the test.
    fn expect(&self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.text().contains(needle) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for {needle:?}; the program printed {:?}",
            self.text()
        );
    }

    /// Waits for the program to exit and returns its exit code.
    fn exit_code(&self) -> u32 {
        // SAFETY: `process` is a live process handle owned by this struct.
        unsafe {
            let status = WaitForSingleObject(self.process, 10_000);
            assert_ne!(
                status,
                WAIT_TIMEOUT,
                "the program did not exit; it printed {:?}",
                self.text()
            );
            let mut code = 0u32;
            assert_ne!(GetExitCodeProcess(self.process, &mut code), 0);
            code
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // SAFETY: the handles are valid and owned by this struct; closing the
        // pseudo console ends the reader thread's pipe.
        unsafe {
            if WaitForSingleObject(self.process, 0) == WAIT_TIMEOUT {
                TerminateProcess(self.process, 1);
            }
            ClosePseudoConsole(self.console);
            CloseHandle(self.process);
            CloseHandle(*self.input.lock().unwrap());
        }
    }
}

fn write_to(handle: HANDLE, bytes: &[u8]) {
    let mut written = 0u32;
    // SAFETY: `bytes` is a valid buffer for the length given.
    unsafe {
        WriteFile(
            handle,
            bytes.as_ptr(),
            bytes.len() as u32,
            &mut written,
            null_mut(),
        );
    }
}

/// Removes ANSI escape sequences so assertions see only the text on screen.
fn strip_escapes(bytes: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        match bytes.get(i) {
            Some(b'[') => {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i += 1;
            }
            Some(b']') => {
                while i < bytes.len()
                    && bytes[i] != 0x07
                    && !(bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\'))
                {
                    i += 1;
                }
                i += if bytes.get(i) == Some(&0x07) { 1 } else { 2 };
            }
            _ => i += 1,
        }
    }
    String::from_utf8_lossy(&out).replace("\r\n", "\n")
}

fn started() -> Pty {
    let pty = Pty::spawn(&events_example());
    pty.expect("q or Ctrl+C quits");
    pty
}

#[test]
fn the_example_starts_in_a_real_console() {
    // RawMode::enable succeeded, or the banner would never print
    started();
}

#[test]
fn a_key_arrives_immediately_without_pressing_enter() {
    let pty = started();
    pty.send(b"a");
    pty.expect("Char('a')");
}

#[test]
fn input_is_not_echoed_by_the_console() {
    let pty = started();
    pty.send(b"zzz");
    pty.expect("Char('z')");
    // the events are printed, but the typed text must not appear on its own
    assert!(!pty.text().contains("zzz"), "{:?}", pty.text());
}

#[test]
fn a_mouse_click_is_parsed_with_zero_based_coordinates() {
    let pty = started();
    pty.send(b"\x1b[<0;12;5M");
    pty.expect("kind: Down(Left), x: 11, y: 4");
}

#[test]
fn a_mouse_drag_and_release_are_reported() {
    let pty = started();
    pty.send(b"\x1b[<32;3;4M\x1b[<0;3;4m");
    pty.expect("kind: Drag(Left), x: 2, y: 3");
    pty.expect("kind: Up(Left), x: 2, y: 3");
}

#[test]
fn arrow_keys_arrive_as_arrow_events() {
    let pty = started();
    pty.send(b"\x1b[A");
    pty.expect("code: Up");
}

#[test]
fn q_quits_with_a_zero_exit_code() {
    let pty = started();
    pty.send(b"q");
    assert_eq!(pty.exit_code(), 0);
}

#[test]
fn ctrl_c_is_delivered_as_a_key_and_the_program_exits_cleanly() {
    let pty = started();
    pty.send(&[0x03]);
    pty.expect("ctrl: true");
    assert_eq!(pty.exit_code(), 0);
}

//! End-to-end tests on Linux and macOS, against a real pseudo terminal.
//!
//! Each test runs the `events` example with its stdin and stdout attached to a
//! pty, writes raw key and mouse escape sequences to it, and checks what the
//! program prints. The scenarios are shared with Windows (`tests/support`).
//!
//! This proves our half of the contract: raw mode delivers input immediately
//! and unechoed, sequences parse to the right events, and the program quits
//! cleanly. It cannot prove that a physical mouse click becomes an escape
//! sequence; that is the terminal emulator's job.
//!
//! The pty needs a few libc calls, so this file has `unsafe`. It is test-only:
//! nothing here is shipped with the crate.
#![cfg(any(target_os = "linux", target_os = "macos"))]
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, CStr};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

extern "C" {
    fn posix_openpt(flags: c_int) -> c_int;
    fn grantpt(fd: c_int) -> c_int;
    fn unlockpt(fd: c_int) -> c_int;
    fn ptsname(fd: c_int) -> *const c_char;
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
}

const O_RDWR: c_int = 2;
#[cfg(target_os = "linux")]
const O_NOCTTY: c_int = 0o400;
#[cfg(target_os = "macos")]
const O_NOCTTY: c_int = 0x20000;
const F_SETFD: c_int = 2;
const FD_CLOEXEC: c_int = 1;

/// The size this fake terminal reports, matching the Windows harness.
const PTY_COLUMNS: u32 = 200;
const PTY_ROWS: u32 = 24;
const SIZE_REPLY: &[u8] = b"\x1b[24;200R";

/// The questions the program asks the terminal, all answered by `reply`.
const QUERIES: [&[u8]; 3] = [b"\x1b[6n", b"\x1b[?u", b"\x1b[c"];

/// The longest query minus one: the bytes that may start a query split across reads.
const QUERY_TAIL: usize = 3;

/// What kind of terminal the pseudo terminal pretends to be.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FakeTerminal {
    /// An ordinary terminal such as xterm: answers device attributes, not the kitty query.
    Plain,
    /// A terminal that supports the kitty keyboard protocol (flags 0).
    Kitty,
    /// A terminal that answers the cursor query (so the program starts) and nothing else.
    Silent,
}

/// The reply `terminal` gives to one query, or nothing.
fn reply(terminal: FakeTerminal, query: &[u8]) -> &'static [u8] {
    match (terminal, query) {
        (_, b"\x1b[6n") => SIZE_REPLY,
        (FakeTerminal::Kitty, b"\x1b[?u") => b"\x1b[?0u",
        (FakeTerminal::Plain | FakeTerminal::Kitty, b"\x1b[c") => b"\x1b[?62;c",
        _ => b"",
    }
}

/// Answers, in the order asked, every complete query in `pending` and removes
/// it, so a query is answered once. Keeps only a tail that might be the start
/// of a query whose bytes are still arriving. Returns the bytes to write back.
fn answer_queries(terminal: FakeTerminal, pending: &mut Vec<u8>) -> Vec<u8> {
    let mut replies = Vec::new();
    loop {
        let earliest = QUERIES
            .iter()
            .filter_map(|query| {
                let start = pending.windows(query.len()).position(|w| w == *query)?;
                Some((start, *query))
            })
            .min_by_key(|&(start, _)| start);
        let Some((start, query)) = earliest else {
            break;
        };
        replies.extend_from_slice(reply(terminal, query));
        pending.drain(..start + query.len());
    }
    if pending.len() > QUERY_TAIL {
        let keep_from = pending.len() - QUERY_TAIL;
        pending.drain(..keep_from);
    }
    replies
}

/// `ptsname` returns a pointer to static storage, so only one thread may use it.
static PTY_LOCK: Mutex<()> = Mutex::new(());

/// A child process attached to a pseudo terminal.
struct Pty {
    child: Mutex<Child>,
    master: Mutex<File>,
    output: Arc<Mutex<Vec<u8>>>,
}

fn example(name: &str) -> PathBuf {
    // `cargo test --test e2e_unix` does not build examples, so build it here
    static BUILD: std::sync::Once = std::sync::Once::new();
    BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "--examples"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("could not run cargo");
        assert!(status.success(), "building the examples failed");
    });

    // target/debug/deps/<test> -> target/debug/examples/events
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push(name);
    assert!(path.exists(), "{} was not built", path.display());
    path
}

/// Opens a new pty and returns its master and slave ends.
fn open_pty() -> (File, File) {
    let _guard = PTY_LOCK.lock().unwrap();
    // SAFETY: these libc calls take a file descriptor or flags and return an
    // error code, a descriptor or a pointer to a NUL-terminated name in
    // static storage, which is copied before the lock is released. The
    // descriptor is owned by exactly one `File` afterwards.
    unsafe {
        let fd = posix_openpt(O_RDWR | O_NOCTTY);
        assert!(fd >= 0, "posix_openpt failed");
        assert_eq!(grantpt(fd), 0, "grantpt failed");
        assert_eq!(unlockpt(fd), 0, "unlockpt failed");
        // do not leak the master into the child process
        assert_eq!(fcntl(fd, F_SETFD, FD_CLOEXEC), 0, "fcntl failed");
        let name = ptsname(fd);
        assert!(!name.is_null(), "ptsname failed");
        let path = CStr::from_ptr(name).to_str().unwrap().to_owned();
        let master = File::from_raw_fd(fd);
        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NOCTTY)
            .open(path)
            .expect("could not open the pty slave");
        (master, slave)
    }
}

impl Pty {
    fn spawn(program: &Path) -> Pty {
        Pty::spawn_with(program, &[], FakeTerminal::Plain)
    }

    fn spawn_with(program: &Path, args: &[&str], terminal: FakeTerminal) -> Pty {
        let (master, slave) = open_pty();
        let child = {
            let mut command = Command::new(program);
            command
                .args(args)
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave));
            command.spawn().expect("could not start the program")
            // `command` is dropped here, closing our copies of the slave
        };

        let output = Arc::new(Mutex::new(Vec::new()));
        let mut reader = master.try_clone().unwrap();
        let mut replier = master.try_clone().unwrap();
        let sink = Arc::clone(&output);
        thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            let mut pending = Vec::new();
            // reading ends with an error (Linux) or 0 (macOS) once the child exits
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                let chunk = &buffer[..read];
                // act as the terminal emulator: answer the questions the
                // program asks (cursor position, kitty keyboard, attributes)
                pending.extend_from_slice(chunk);
                let replies = answer_queries(terminal, &mut pending);
                if !replies.is_empty() {
                    let _ = replier.write_all(&replies);
                }
                sink.lock().unwrap().extend_from_slice(chunk);
            }
        });

        Pty {
            child: Mutex::new(child),
            master: Mutex::new(master),
            output,
        }
    }

    fn send(&self, bytes: &[u8]) {
        let mut master = self.master.lock().unwrap();
        master.write_all(bytes).unwrap();
        master.flush().unwrap();
    }

    /// Everything printed so far, with escape sequences removed.
    fn text(&self) -> String {
        strip_escapes(&self.output.lock().unwrap())
    }

    /// Everything printed so far, exactly as the program wrote it.
    fn raw(&self) -> Vec<u8> {
        self.output.lock().unwrap().clone()
    }

    /// Waits until the raw output contains `needle`, or fails the test.
    fn expect_raw(&self, needle: &[u8]) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if contains(&self.raw(), needle) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for {needle:?} in the raw output");
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

    /// Whether the program has already exited.
    fn has_exited(&self) -> bool {
        self.child.lock().unwrap().try_wait().unwrap().is_some()
    }

    /// Waits for the program to exit and returns its exit code.
    fn exit_code(&self) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(status) = self.child.lock().unwrap().try_wait().unwrap() {
                return status.code().map_or(u32::MAX, |code| code as u32);
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("the program did not exit; it printed {:?}", self.text());
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let child = self.child.get_mut().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[macro_use]
mod support;
use support::strip_escapes;

e2e_scenarios!();

/// Whether `haystack` contains `needle`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    last_index(haystack, needle).is_some()
}

/// Where `needle` last appears in `haystack`.
fn last_index(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// The key release tests: `events --key-release` on a fake kitty, plain or silent terminal.
#[test]
fn a_kitty_terminal_turns_on_key_release_and_the_release_arrives_as_key_up() {
    let pty = Pty::spawn_with(&example("events"), &["--key-release"], FakeTerminal::Kitty);
    pty.expect("key release: on");
    assert!(contains(&pty.raw(), b"\x1b[>11u"), "{:?}", pty.raw());

    pty.send(b"\x1b[97;1:1u");
    pty.expect("Char('a')");
    pty.send(b"\x1b[97;1:3u");
    pty.expect("KeyUp(Key { code: Char('a')");

    pty.send(b"q");
    assert_eq!(pty.exit_code(), 0);
    pty.expect_raw(b"\x1b[?1049l");
    let raw = pty.raw();
    let off = last_index(&raw, b"\x1b[<u").expect("turns key release off");
    let leave = last_index(&raw, b"\x1b[?1049l").expect("leaves the alternate screen");
    assert!(
        off < leave,
        "key release must be turned off while still on the alternate screen"
    );
}

#[test]
fn a_plain_terminal_gets_no_key_release_and_nothing_is_sent_to_turn_it_on() {
    let pty = Pty::spawn_with(&example("events"), &["--key-release"], FakeTerminal::Plain);
    pty.expect("key release: off");
    assert!(
        contains(&pty.raw(), b"\x1b[?u"),
        "the program should ask the terminal"
    );

    pty.send(b"a");
    pty.expect("Char('a')");

    pty.send(b"q");
    assert_eq!(pty.exit_code(), 0);
    pty.expect_raw(b"\x1b[?1049l");
    let raw = pty.raw();
    assert!(!contains(&raw, b"\x1b[>11u"), "{raw:?}");
    assert!(!contains(&raw, b"\x1b[<u"), "{raw:?}");
}

#[test]
fn a_terminal_that_never_answers_the_query_still_starts_within_a_second_or_so() {
    let started = Instant::now();
    let pty = Pty::spawn_with(&example("events"), &["--key-release"], FakeTerminal::Silent);
    pty.expect("key release: off");
    // the wait is 300 ms; the bound is generous so a slow CI machine does not fail it
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );

    pty.send(b"a");
    pty.expect("Char('a')");
    pty.send(b"q");
    assert_eq!(pty.exit_code(), 0);
    assert!(!contains(&pty.raw(), b"\x1b[>11u"));
}

#[test]
fn arrow_key_release_and_ctrl_c_in_key_release_mode_still_work() {
    let pty = Pty::spawn_with(&example("events"), &["--key-release"], FakeTerminal::Kitty);
    pty.expect("key release: on");

    pty.send(b"\x1b[1;1:3A");
    pty.expect("KeyUp(Key { code: Up");

    pty.send(b"\x1b[99;5u");
    assert_eq!(pty.exit_code(), 0);
}

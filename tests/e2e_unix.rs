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
        let (master, slave) = open_pty();
        let child = {
            let mut command = Command::new(program);
            command
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
            // reading ends with an error (Linux) or 0 (macOS) once the child exits
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                let chunk = &buffer[..read];
                // act as the terminal emulator: answer the cursor position
                // query that `Terminal::start` uses to find the window size
                if chunk.windows(4).any(|w| w == b"\x1b[6n") {
                    let _ = replier.write_all(SIZE_REPLY);
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

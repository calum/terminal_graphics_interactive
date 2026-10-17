# Changelog

## Unreleased

- `Terminal`: an interactive session. `Terminal::start()` enters raw mode,
  turns on mouse reporting and hides the cursor, and reads input on a
  background thread; `poll(timeout)` and `wait()` return events. Everything is
  undone on drop. A lone Escape byte becomes the Escape key after 25 ms.
- `Parser`: incremental parser for terminal input (characters, UTF-8, control
  keys, arrows, navigation and function keys, Shift/Alt/Ctrl modifiers, SGR
  mouse press, release, drag, move and scroll). `Parser::has_pending`.
- `RawMode`: raw terminal mode guard for Unix and Windows, restored on drop.
  `MOUSE_ON` and `MOUSE_OFF` escape sequences.
- `cargo run --example events` prints events live.
- `cargo bench` parser benchmarks.
- Re-exports `terminal_graphics`.
- End-to-end tests on Windows (`tests/e2e_windows.rs`): the `events` example runs in a ConPTY pseudo console and is fed raw key and mouse sequences.
- Shared end-to-end scenarios (`tests/support`) and a Unix pty harness (`tests/e2e_unix.rs`), plus a CI workflow running the suite on Ubuntu, macOS and Windows and a Rust 1.70 check.
- `Terminal` now uses the alternate screen and finds the window size by asking the terminal (`Terminal::size`), falling back to 80x24. `Terminal::draw(&display)` shows a `Display` at the top left, so `Mouse::cell_in(&display)` maps a mouse event straight to a cell. `Parser::expect_cursor_position` / `take_cursor_position` handle the size reply, which is otherwise indistinguishable from Ctrl+F3.
- `cargo run --example paint`: a small paint program (strokes are joined into lines so fast drags leave no gaps, right button erases, 1-8 or scroll picks a colour).
- End-to-end scenarios for the paint example and the window size on Windows, Linux and macOS.
- `llms.txt`: a page for AI coding agents (what it is, when to use it and when not to, the API, easy mistakes, and a complete example). Its example is `examples/quickstart.rs`, which cargo compiles; `tests/llms.rs` checks the page against the API and the example against the file, and an end-to-end scenario runs the example in a real terminal.
- `terminal_graphics_interactive::prelude::*` brings in the graphics and interactive types together.
- `cargo run --example screensaver`: bouncing dots with fading trails (click adds a dot, any key quits). It shows the frame-timer loop: draw, handle input until the next frame is due, advance the animation.
- **Breaking (0.x):** pasted text now arrives as one `Event::Paste(String)` instead of key presses, using bracketed paste (the terminal is asked to wrap pastes in markers; `Terminal` turns it on and off). `Event` is no longer `Copy` and is `#[non_exhaustive]`, so a `match` on it needs a `_ =>` arm. Fixes right-click paste in terminals that paste on right click: pasting `cargo run ...` into `paint` used to press `c` (clear) and could press `q` (quit). Pastes over 1 MiB are cut. `paint` and `screensaver` ignore pastes.
- CI: a `publish` job releases to crates.io when `master` has a new version (token from the `CARGO_REGISTRY_TOKEN` secret; it warns and skips if the secret is missing). `.github` and `CLAUDE.md` are left out of the published crate.

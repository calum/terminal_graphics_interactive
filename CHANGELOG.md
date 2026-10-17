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

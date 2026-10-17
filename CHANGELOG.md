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

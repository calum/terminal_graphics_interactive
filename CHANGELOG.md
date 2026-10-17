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
- **Easy art API** (`art` module, in the prelude). `sketch(formula)` / `sketch_with(palette, formula)` fill the window every frame from a formula (a `Cell` with the column, the stretched row, the time and the mouse; return -1 to 1). `Palette` (`ocean`, `fire`, `rainbow`, `mono`, or `Palette::new`) turns numbers into colours using shade blocks for about 4x the steps. `World::new().balls(5).size(3.5).gravity(0.02).mouse_pulls().trails().run()` is a physics playground whose balls bounce off the walls and each other. Headless use for tests and benchmarks: `sketch_frame`, `World::simulation`, `Simulation`, `Pointer`.
- `Terminal::report_mouse_movement()` turns on pointer movement reporting with no button held (off by default: it is a lot of events over a slow link).
- New examples `plasma` and `balls`, benchmarks (`cargo bench --bench art`), `CONTRIBUTING.md` (how to add a palette, a setting or an example), and end-to-end scenarios for both examples on Windows, Linux and macOS.
- `llms.txt` has a section on making art, whose examples are the real `examples/plasma.rs` and `examples/balls.rs`.
- The design note is `docs/superpowers/specs/2026-10-17-art-api-design.md`.
- README rewritten: a short pitch with GIFs of the plasma and the physics world (a scripted mouse arrow and click rings drawn over the real examples), install, the examples, and how to add mouse and keyboard to anything. `tests/readme.rs` checks the GIFs exist and are real GIFs of a sensible size, every GIF is shown, and the README shows the real example files.
- `cargo run --release --example make_gifs` rebuilds `assets/*.gif`. It uses a pure Rust GIF encoder, a rasteriser, and a strict mini Rust interpreter (`examples/site/script`) that runs the real example sources and refuses what rustc would refuse (for example `cell.x * 2`). None of it is published to crates.io.
- **A website** with a live editor, built from Rust: `examples/web_art.rs` is the art engine compiled to WebAssembly (it runs the real example sources through a strict mini Rust interpreter, `examples/site/script`), `examples/build_site.rs` writes the page into `target/site`, and `examples/check_site.rs` loads it in a headless browser and drives its buttons, editor and mouse (`--require` fails if no browser is found). `.github/workflows/pages.yml` builds and checks the site on every pull request and deploys it to GitHub Pages when `master` changes (Settings, Pages, Source = "GitHub Actions"). Examples `rings`, `tunnel`, `waves` and `bubbles` join the gallery.

# terminal_graphics_interactive

Keyboard and mouse input for [`terminal_graphics`](https://github.com/calum/terminal_graphics_api), so you can build interactive terminal games, art, animations, screensavers, maths and physics demos, and drawing tools. One dependency (`terminal_graphics`, re-exported), almost no `unsafe`.

**Status: early (0.1).** Done: the input parser, raw terminal mode, `Terminal` (an interactive session with `poll(timeout)`, the window size and `draw`), mouse-to-cell mapping, a paint example, and [`llms.txt`](llms.txt) for AI coding agents. Next: a game example and terminal resize events.

## Try it

```sh
cargo run --example paint        # drag to paint, right-drag to erase, 1-8 or scroll for colour, c clears, q quits
cargo run --example screensaver   # bouncing dots with fading trails; click to add one, any key quits
cargo run --example events       # prints every key and mouse event
```

It works in Windows PowerShell and Windows Terminal, Linux and macOS terminals, and over SSH if you allocate a terminal (`ssh -t host ...`).

## How it works

1. `Terminal::start()` switches to the alternate screen (your scrollback comes back on exit), puts the terminal in raw mode, turns on mouse reporting, hides the cursor and finds the window size. Dropping it undoes all of that, even on panic. `terminal.poll(timeout)` returns the next event, or `None` when the time runs out (your cue to draw the next frame); `terminal.wait()` blocks. (`RawMode`, `Parser`, `MOUSE_ON` and `MOUSE_OFF` are public too if you want the pieces.)
2. Events are `Event::Key` and `Event::Mouse`. A lone Escape byte becomes the Escape key after 25 ms, so arrow keys over a slow SSH link are not mistaken for it.
3. `terminal.draw(&display)` shows a `Display` at the top left of the window, so mouse coordinates (0-based cells, `(0, 0)` at the top left) line up with display cells: `mouse.cell_in(&display)` gives the cell under the pointer. Make the display at most `terminal.size()` wide and one row shorter than it is high.

## `unsafe`

Raw mode cannot be done in safe Rust, so it is the only place `unsafe` appears: three libc calls on Unix (`src/raw_mode/unix.rs`) and three `kernel32` calls on Windows (`src/raw_mode/windows.rs`), each with a `// SAFETY:` comment. Everything else is safe Rust, and `tests/unsafe_audit.rs` fails the build if `unsafe` appears anywhere else.

## Performance

`cargo bench` runs `benches/parser.rs`, which uses only `std`. Median of 15 samples on one machine (Windows 11, release build), parsing 10,000 events fed in chunks:

| Stream | Chunk | Before | Now |
|---|---|---:|---:|
| mouse drag (SGR) | 64 B | 44 ns/event | 41 ns/event |
| mouse drag (SGR) | 4 KB | 134 ns/event | 37 ns/event |
| mouse drag (SGR) | 1 MB paste | 1630 ns/event | 85 ns/event |
| keyboard | 64 B | 102 ns/event | 28 ns/event |
| keyboard | 1 MB paste | 415 ns/event | 27 ns/event |

"Before" had two problems that the benchmark exposed: parsed bytes were removed from the front of the buffer after every event (quadratic on large pastes), and key sequences allocated a `Vec`. A terminal produces nowhere near 10 million events a second, so parsing is far from being a bottleneck.

## Platform checks so far

| Platform | Built and unit tested | Run against a real terminal |
|---|---|---|
| Windows | yes | yes: 8 end-to-end tests in a ConPTY pseudo console (`tests/e2e_windows.rs`) |
| Linux, macOS | type-checked and linted (`cargo check`/`clippy --target`) | written (`tests/e2e_unix.rs`, same scenarios) but **not yet run**; first run is in CI |

## License

Licensed under either of MIT or Apache-2.0 at your option.

# terminal_graphics_interactive

Keyboard and mouse input for [`terminal_graphics`](https://github.com/calum/terminal_graphics_api), so you can build interactive terminal games, art, animations, screensavers, maths and physics demos, and drawing tools. One dependency (`terminal_graphics`, re-exported), almost no `unsafe`.

**Status: early (0.1).** Done: the input parser, raw terminal mode, and `Terminal`, an interactive session with `poll(timeout)`. Next: mapping mouse positions to `Display` cells (alternate screen, terminal size without extra unsafe), a paint-program example, and `llms.txt`.

## Try it

```sh
cargo run --example events
```

Click, drag, scroll and type in the window; each event is printed. `q` or Ctrl+C quits. It works in Windows PowerShell and Windows Terminal, Linux and macOS terminals, and over SSH if you allocate a terminal (`ssh -t host ...`).

## How it works

1. `Terminal::start()` puts the terminal in raw mode, turns on mouse reporting and hides the cursor. Dropping it undoes all of that, even on panic. `terminal.poll(timeout)` returns the next event, or `None` when the time runs out (your cue to draw the next frame); `terminal.wait()` blocks. (`RawMode`, `Parser`, `MOUSE_ON` and `MOUSE_OFF` are public too if you want the pieces.)
2. Events are `Event::Key` and `Event::Mouse`. A lone Escape byte becomes the Escape key after 25 ms, so arrow keys over a slow SSH link are not mistaken for it.
3. Mouse coordinates are 0-based cells with `(0, 0)` at the top left, the same orientation as `Display`.

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
| Windows | yes | yes: 8 end-to-end tests in a ConPTY pseudo console (`tests/e2e_windows.rs`), run in CI and locally. Not yet tried by a person in a real PowerShell or Windows Terminal window |
| Linux, macOS | yes | yes: the same 8 end-to-end tests in a pty (`tests/e2e_unix.rs`), run in CI on Ubuntu and macOS |

## License

Licensed under either of MIT or Apache-2.0 at your option.

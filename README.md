# terminal_graphics_interactive

Keyboard and mouse input for [`terminal_graphics`](https://github.com/calum/terminal_graphics_api), so you can build interactive terminal games, art, animations, screensavers, maths and physics demos, and drawing tools. One dependency (`terminal_graphics`, re-exported), almost no `unsafe`.

**Status: early (0.1).** Done: the input parser, raw terminal mode, `Terminal` (an interactive session with `poll(timeout)`, the window size and `draw`), mouse-to-cell mapping, an easy [art API](#make-art-in-a-few-lines) (formula animations and a physics world), and [`llms.txt`](llms.txt) for AI coding agents. Next: a game example and terminal resize events. Want to add a palette, an effect or an example? See [CONTRIBUTING.md](CONTRIBUTING.md).

## Try it

```sh
cargo run --example plasma        # an animated plasma in one formula; move the mouse to make ripples
cargo run --example balls         # five big bouncing balls; the mouse pulls them, right button pushes, click adds one
cargo run --example paint         # drag to paint, right-drag to erase, 1-8 or scroll for colour, c clears, q quits
cargo run --example screensaver   # bouncing dots with fading trails; click to add one, any key quits
cargo run --example events        # prints every key and mouse event
```

It works in Windows PowerShell and Windows Terminal, Linux and macOS terminals, and over SSH if you allocate a terminal (`ssh -t host ...`). Press `q` to quit.

## Make art in a few lines

Write the interesting part and nothing else. The library starts the terminal, runs the frame loop, tracks the mouse, quits on `q` and puts everything back.

**A formula becomes an animation.** The closure runs for every cell every frame and returns a number from -1 to 1, which is what `sin` and `cos` give:

```rust,no_run
use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Move the mouse to send ripples across it. Press q to quit.
    sketch_with(Palette::fire(), |cell| {
        let ripple = cell.dist_to_mouse() * 0.25 - cell.t * 3.0;
        ((cell.x * 0.15 + cell.t).sin() + (cell.y * 0.2 - cell.t * 0.7).sin() + ripple.sin()) / 3.0
    })
}
```

`cell` has `x`, `y`, `t` (seconds), the mouse position and button, and the window size. `y` is stretched so a circle looks round. A terminal has only 16 colours, so numbers between two colours are drawn with shade blocks (`░▒▓█`) for about four times as many steps. Pick a `Palette` (`ocean`, `fire`, `rainbow`, `mono`) or make your own with `Palette::new(&[...])`.

**A physics world in a few settings.** Balls bounce off the walls and each other, and a click adds one:

```rust,no_run
use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // The mouse pulls the balls in, the right button pushes them away, a click
    // adds a ball. Press q to quit.
    World::new()
        .balls(5)
        .size(3.5)
        .gravity(0.02)
        .mouse_pulls()
        .trails()
        .run()
}
```

To test or measure art without a terminal, `sketch_frame(&mut display, &palette, t, &pointer, formula)` draws one frame into a `Display`, and `world.simulation(seed)` gives a `Simulation` you advance yourself.

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

`cargo bench --bench art` measures the art maths (the frame budget at 30 frames a second is 33,000 µs; the cost of printing the frame is in `terminal_graphics`' own benchmarks). Same machine, release build:

| Per frame | 80x24 | 200x60 |
|---|---:|---:|
| plasma (3 sines and a distance per cell) | 93 µs | 786 µs |
| one sine per cell | 80 µs | 523 µs |
| 5 big balls with trails | 15 µs | 71 µs |
| 64 small balls with trails | 30 µs | 83 µs |

Even a 200 by 60 window with three sines per cell uses about 2% of a frame, so there is plenty of room for a heavier formula.

## Platform checks so far

| Platform | Built and unit tested | Run against a real terminal |
|---|---|---|
| Windows | yes | yes: 16 end-to-end tests in a ConPTY pseudo console (`tests/e2e_windows.rs`), run in CI and locally. Not yet tried by a person in a real PowerShell or Windows Terminal window |
| Linux, macOS | yes | yes: the same 16 end-to-end tests in a pty (`tests/e2e_unix.rs`), run in CI on Ubuntu and macOS |

## License

Licensed under either of MIT or Apache-2.0 at your option.

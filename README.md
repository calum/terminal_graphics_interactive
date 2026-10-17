# terminal_graphics_interactive

[![CI](https://github.com/calum/terminal_graphics_interactive/actions/workflows/ci.yml/badge.svg)](https://github.com/calum/terminal_graphics_interactive/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/terminal_graphics_interactive.svg)](https://crates.io/crates/terminal_graphics_interactive)
[![docs.rs](https://docs.rs/terminal_graphics_interactive/badge.svg)](https://docs.rs/terminal_graphics_interactive)
[![licence](https://img.shields.io/crates/l/terminal_graphics_interactive.svg)](#licence)

**Make art, animations, games and drawing tools in your terminal, with the mouse and keyboard, in a few lines of Rust.** It works in Windows PowerShell, Linux, macOS and over SSH. Its only dependency is [`terminal_graphics`](https://github.com/calum/terminal_graphics_api) (re-exported), and almost nothing in it is `unsafe`.

| A formula becomes an animation | A physics world in five lines |
|:---:|:---:|
| ![An animated plasma. A mouse arrow glides across it and ripples follow](https://raw.githubusercontent.com/calum/terminal_graphics_interactive/master/assets/plasma.gif) | ![Five bouncing balls. A mouse arrow pulls them, a click adds one, and the right button pushes them away](https://raw.githubusercontent.com/calum/terminal_graphics_interactive/master/assets/balls.gif) |
| move the mouse to make ripples | the mouse pulls the balls, the right button pushes them, a click adds one |

*These are recordings of the real examples below, run against the real library, with a scripted mouse arrow drawn on top (`cargo run --release --example make_gifs` rebuilds them).*

### [Try it in your browser](https://calum.github.io/terminal_graphics_interactive/)

Write some Rust in the page, press run, and watch it in a terminal. Nothing to install.

## Install

```sh
cargo add terminal_graphics_interactive
```

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

## Add the mouse and keyboard to anything

Under the art API is `Terminal`: a full-screen session that gives you key presses, mouse clicks, drags, scrolling and pastes as events, and a window to draw a `Display` in. `poll` doubles as the frame timer, and `mouse.cell_in(&display)` turns a mouse position into a cell:

```rust,no_run
use std::time::Duration;
use terminal_graphics_interactive::{Colour, Display, Event, MouseKind, Terminal};

fn main() -> std::io::Result<()> {
    let mut terminal = Terminal::start()?;

    // `draw` needs a display at most one row shorter than the window
    let (width, height) = terminal.size();
    let mut display = Display::new(width, height.saturating_sub(1));

    loop {
        terminal.draw(&display)?;

        // wait up to 50 ms for input; `None` means a frame passed with no input
        match terminal.poll(Duration::from_millis(50))? {
            Some(Event::Key(key)) if key.is_char('q') || key.is_ctrl('c') => return Ok(()),
            Some(Event::Mouse(mouse)) if matches!(mouse.kind, MouseKind::Down(_)) => {
                if let Some((x, y)) = mouse.cell_in(&display) {
                    display.set_pixel(x, y, 'X', Colour::BrightYellow, Colour::Blue);
                }
            }
            _ => {}
        }
    }
}
```

Ctrl+C arrives as a key (`key.is_ctrl('c')`) while a `Terminal` exists, so check for it and leave your loop. Pasted text arrives as one `Event::Paste`, so pasting a command can never trigger your key bindings.

## More examples

```sh
cargo run --example plasma        # the formula above
cargo run --example balls         # the physics world above
cargo run --example paint         # drag to paint, right-drag to erase, 1-8 or scroll for colour, c clears
cargo run --example screensaver   # bouncing dots with fading trails; click to add one, any key quits
cargo run --example events        # prints every key and mouse event
```

## For AI coding agents

[`llms.txt`](llms.txt) is a short page written for you: what this crate is, when to use it and when not to, the whole API, the mistakes that are easy to make (Ctrl+C is an event, the display must be one row shorter than the window, use `\r\n` in raw mode) and complete examples that are checked against the code.

## Platforms

| | Built and tested | Run in a real terminal by the tests |
|---|:---:|:---:|
| Windows (PowerShell, Windows Terminal) | yes | yes, in a ConPTY pseudo console |
| Linux | yes | yes, in a pty |
| macOS | yes | yes, in a pty |

Over SSH it works when a terminal is allocated (`ssh -t host ...`). The tests prove that raw mode, key and mouse sequences, pastes and quitting work on each system. They cannot prove that a physical mouse click becomes an escape sequence: that is your terminal emulator's job.

## Under the hood

- **Almost no `unsafe`.** Raw mode cannot be done in safe Rust, so the only `unsafe` is a few libc calls on Unix and a few `kernel32` calls on Windows (`src/raw_mode/`), each with a `// SAFETY:` comment. `tests/unsafe_audit.rs` fails the build if `unsafe` appears anywhere else.
- **Fast.** `cargo bench` runs benchmarks that use only `std`. A 200 by 60 plasma costs about 0.8 ms of a 33 ms frame, and parsing input takes tens of nanoseconds per event.
- **Tested.** Unit tests for the parser, the palette and the simulation, and end-to-end tests that run every example in a real pseudo terminal on all three systems.

## Contributing

Small, friendly pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) has short recipes for adding a palette, a `World` setting or an example, and [CLAUDE.md](CLAUDE.md) explains the principles and process.

## Links

- Website with a live editor: https://calum.github.io/terminal_graphics_interactive/
- crates.io: https://crates.io/crates/terminal_graphics_interactive
- API documentation: https://docs.rs/terminal_graphics_interactive
- The graphics crate it builds on: https://github.com/calum/terminal_graphics_api

## Licence

Licensed under either of MIT or Apache-2.0 at your option.

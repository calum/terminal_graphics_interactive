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

## Terminal Games

**Make games in your terminal.** A game is a set of screens (a title, a play screen, a pause, a game over). Each one is a `Game` that updates 60 times a second, reads held keys and the mouse, and draws on a `Screen`. The crate also brings random numbers, timers, object pools and collision shapes, and `Headless` runs a game with no terminal, so tests can play it.

Play the examples. `sail` is the flagship: a sailboat on the open sea, under a sky that turns from dawn to night. Steer left and right to dodge the fireballs that fall from the sky; the farther you sail, the rougher the sea and the more fireballs come.

```sh
cargo run --example sail        # the flagship: a sailboat on the sea; steer to dodge the fireballs
cargo run --example glow        # a light drifts through a plasma sky; touch the motes
cargo run --example quickgame   # steer a @ and collect the * stars for 20 seconds
cargo run --example sail_together -- host 2        # sail with a friend: one hosts (waits for 1 more) ...
cargo run --example sail_together -- join HOST:7777   # ... and the other joins
```

**It makes multiplayer games too.** This is `sail_together`: two players, two boats, one sea. Each machine runs the whole game and they share only their key presses, so it needs almost no bandwidth.

![Two sailboats, a red one and a blue one, sailing side by side on the same sea while fireballs fall. The top row shows each player's lives](https://raw.githubusercontent.com/calum/terminal_graphics_interactive/master/assets/sail_together.gif)

*A recording of the real game, from the first player's screen. The two players here are two games joined by an in-process link, so the recording is the same every time; the example itself plays over TCP between computers (`cargo run --release --example make_gifs` rebuilds it).*

This is the shape of `quickgame`: a title that switches to the play screen on Enter. The rest is in [examples/quickgame.rs](examples/quickgame.rs):

```rust
use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(Title { score: None })
}

impl Game for Title {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed))
        } else {
            Next::stay()
        }
    }
}
```

What is and is not verified. Held keys need key release events, which only some terminals send: kitty, WezTerm, Ghostty, foot, Alacritty, iTerm2 and recent Windows Terminal support the kitty keyboard protocol. Where a terminal does not, a held key is timed instead, so a key stays held for a moment after its last repeat. The glow game runs in a pseudo terminal in our tests on Linux, and quickgame is tested headless. Both games are not yet run by a person on Windows or macOS, and the games also run on the website (`/games/`, the real games as WebAssembly), which a headless browser check loads but no person has looked at yet. Multiplayer: `Sim`, `Lockstep` and the `Local` and `Tcp` transports are built and tested (including two machines playing `sail_together` over real TCP on loopback, and lossy, reordered and dropped messages over a fake link). The game's numbers are checked to be bit-identical across machines only by a recorded state hash that CI runs on Linux, macOS and Windows; no one has yet played `sail_together` between two real computers or terminals, and there is no WebSocket transport for the browser yet.

Play between machines is lockstep: every machine runs the same deterministic `Sim` and they share only each player's input, so a match needs almost no bandwidth and a game that drifts apart is reported with the step it happened at. See `examples/sail_together.rs`.

To test a game without a terminal, use `Headless`. It runs the same updates as `run`: you press keys, step time and read back what was drawn, as `tests/game_api.rs` and the tests in `examples/quickgame.rs` do.

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

## Rendering cost

`Terminal::draw` sends only the cells that changed since the last frame (`Renderer`, also public for any `Write`). The first frame draws everything. `cargo bench --bench render` compares it with redrawing the whole frame each time, on the same frames, with `std` timing:

| Scenario | Size | Full redraw: us per frame | Full redraw: bytes per frame | Changed cells only: us per frame | Changed cells only: bytes per frame |
|---|---|---:|---:|---:|---:|
| still (same frame) | 80x24 | 79 to 97 | 17,717 | 2.4 to 3.4 | 0 |
| still | 200x60 | 526 to 564 | 109,145 | 16 to 19 | 0 |
| a 3x3 block moving | 80x24 | 5.4 to 6.6 | 2,597 | 3.2 to 5.5 | 88 |
| a 3x3 block moving | 200x60 | 30 to 33 | 13,673 | 18 to 20 | 89 |
| plasma (every cell changes) | 80x24 | 80 to 91 | 17,717 | 98 to 163 | 17,259 |
| plasma (every cell changes) | 200x60 | 503 to 616 | 109,145 | 597 to 629 | 107,943 |

Machine: Intel Xeon at 2.10 GHz, 4 threads, Linux x86_64. Each time is a median of 15 samples. The ranges are the lowest and highest of the runs made: five for the full redraw (two of them from before the renderer existed) and three for the changed cells only. The byte counts are exact and do not vary between runs. The first full frame is left out of the changed-cells bytes.

- **Still and moving pictures** send far fewer bytes, which is what matters over a slow SSH link: none at all for a still picture, and under 100 bytes for a small moving block instead of several kilobytes. They also take less time, at 200 by 60 about 19 us against about 31 us for the moving block, and far less for the still picture.
- **Plasma, where every cell changes,** saves only 2 to 3 per cent of the bytes. Its time is not better: the changed-cells runs were slower than the full redraw in both runs at both sizes, but at 80 by 24 the spread between runs is as large as the difference, so these runs do not show whether it is a real cost.
- At 60 frames a second a frame has about 16.7 ms, and both ways fit easily at these sizes. The gain is in the bytes, not the CPU.

## Contributing

Small, friendly pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) has short recipes for adding a palette, a `World` setting or an example, and [CLAUDE.md](CLAUDE.md) explains the principles and process.

## Links

- Website with a live editor and playable games: https://calum.github.io/terminal_graphics_interactive/ (art at `/art/`, games at `/games/`)
- crates.io: https://crates.io/crates/terminal_graphics_interactive
- API documentation: https://docs.rs/terminal_graphics_interactive
- The graphics crate it builds on: https://github.com/calum/terminal_graphics_api

## Licence

Licensed under either of MIT or Apache-2.0 at your option.

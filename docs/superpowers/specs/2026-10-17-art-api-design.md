# An easy way to make art and fun graphics

## Why

The people (and AI agents) this crate is for do not want to write a terminal
program. They want to type a few lines, a formula or a handful of settings, and
see something move and look good. Today even `quickstart` is 20 lines of loop,
timing and quitting. The goal is that the interesting part is all they write.

The moment we designed for: someone tries a few short scripts. One is a formula
that becomes an animation. Another is a few lines of physics that react to the
mouse. It should feel fun, be easy to guess, and an empty script should already
look nice.

## What it is

A new `art` module in `terminal_graphics_interactive`, also in the prelude.

**Formula art**

```rust
sketch_with(Palette::fire(), |cell| {
    let ripple = cell.dist_to_mouse() * 0.25 - cell.t * 3.0;
    ((cell.x * 0.15 + cell.t).sin() + (cell.y * 0.2 - cell.t * 0.7).sin() + ripple.sin()) / 3.0
})
```

The closure runs for every cell every frame and returns a number from -1 to 1
(what `sin` and `cos` give). The library turns it into colour.

**Physics play**

```rust
World::new().balls(5).size(3.5).gravity(0.02).mouse_pulls().trails().run()
```

Chained settings describe a world; the library does the motion.

Both handle terminal setup, the frame loop, quitting (`q`, Esc, Ctrl+C), the
mouse and restoring the terminal. Both work in PowerShell, Linux, macOS and over
SSH because they sit on `Terminal`.

## Goals

- A working script is 1 to 6 lines, and an empty one (`World::new().run()`)
  already looks good.
- Both styles share vocabulary (`run`, `palette`, the mouse always works).
- Easy for a person to tinker with and easy for an AI agent to get right the
  first time: few names, sensible defaults, no setup to forget.
- **Easy to extend by pull request.** A new palette is one small function with
  one test. A new effect is a new example or a new `World` setting. Nothing
  needs touching in the frame loop.
- Fast enough: a 200 by 60 window at 30 frames a second with a formula per cell.

## Non-goals

- 24-bit colour, images, sound, a scripting language, async, a GUI.
- A general game engine. `World` stays small; richer physics is a future PR.
- Changing what `Terminal`, `Display` or `Parser` do for existing users.

## Decisions (mine unless marked)

- **Where:** a module in this crate, not a new crate, because it needs the
  mouse and the frame loop and the user wants one dependency. *(my call)*
- **Units:** `x` is a column. `y` is a row stretched by two, so a circle looks
  round (terminal cells are about twice as tall as wide). The mouse position is
  in the same units. *(from the spike, which the user liked)*
- **Return range:** -1 to 1, so `sin` and `cos` work with no rescaling. NaN and
  out of range values are clamped, never a crash. *(my call)*
- **Colour depth:** only 16 colours exist, so a value between two palette
  colours is drawn as a shade block (`░▒▓█`) with one colour in front and one
  behind. That gives about 4x the steps. *(from the spike)*
- **Palette is a struct, not an enum**, so anyone can build their own
  (`Palette::new(&[...])`) and adding a preset is a pull request that adds one
  function. Fewer than 2 colours is repaired, not a panic. *(changed from the
  spike to support extension)*
- **Pointer movement:** the library normally reports the mouse only while a
  button is held. `Terminal::report_mouse_movement()` turns on movement
  reporting; `art` uses it so the pointer can steer things without a click.
- **Physics:** balls bounce off the walls by their edge, and off each other as
  equal masses. The mouse pulls, the right button pushes, a click adds a ball
  (capped at 64). Gravity is per frame. *(from the spike plus the user's tweak
  to 5 big balls)*
- **Testability:** the simulation and the per cell fill are pure functions of
  their inputs, so they are unit tested without a terminal. Only the loop that
  calls them needs a real terminal, and that is covered end to end.
- **Names in the prelude:** `sketch`, `sketch_with`, `Cell`, `Palette`, `World`.
  *(my call; easy to rename before a release)*

## Open questions

None that block. Later: more presets, more forces (wind, springs, attractors),
a `Particles` effect, saving a frame as text.

## How we will know it works

- Unit tests for palettes, the fill, and every part of the simulation.
- End to end scenarios running both examples in a real terminal on Windows,
  Linux and macOS: they draw, take mouse input, and quit cleanly.
- A benchmark for the fill and the physics step, with numbers in the README.
- `llms.txt` shows an art example that is a real, compiled file.

## What changed while building it

- **Headless API.** The simulation and the per cell fill needed to be reachable
  by tests and benchmarks, so they are public: `sketch_frame`, `World::simulation`,
  `Simulation` and `Pointer`. This also lets an AI agent test its own art with no
  terminal. *(my call)*
- **A bug the tests found, also present in the spike.** Pushing overlapping balls
  apart after the walls had clamped them could shove a ball back out of the
  window. Positions are now clamped once more after collisions.
- **Benchmarks.** A 200 by 60 plasma costs about 0.8 ms and the physics under
  0.1 ms per frame, about 2% of a 33 ms frame at most, so nothing was optimised.
- **The spike's names.** The demos are now `examples/plasma.rs` and
  `examples/balls.rs`; `art_plasma` and `art_physics` only existed on a local
  throwaway branch.

# Contributing

Thank you! This is a hobby project that wants to stay fun, so small, friendly
pull requests are very welcome. Read [CLAUDE.md](CLAUDE.md) first: it explains
the principles (easy for people *and* AI agents, fast, almost no `unsafe`, no
dependencies) and the process (tests first).

The easiest way in is the art module, which is built so that adding to it is a
small, self-contained change.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --all-targets      # must have no warnings
cargo test --all-targets && cargo test --doc
```

Add a line to [CHANGELOG.md](CHANGELOG.md) under "Unreleased".

## Add a palette

A palette is a ramp of colours from dark to bright. In
[src/art/palette.rs](src/art/palette.rs):

1. Write the test first: it has at least two colours, and it starts and ends
   where you say (see the tests for `ocean` and `fire`).
2. Add a function next to `mono`:

   ```rust
   /// Black through green to white.
   pub fn forest() -> Palette {
       use Colour::*;
       Palette::from_ramp(&[Black, Green, BrightGreen, BrightWhite])
   }
   ```

   `from_ramp` needs at least two colours; people building their own with
   `Palette::new` get that checked for them.
3. Add it to the list of presets in [llms.txt](llms.txt) so AI agents find it.

## Add a setting to `World`

A setting is a builder method on `World` and a rule in `Simulation::step` in
[src/art/world.rs](src/art/world.rs).

1. Write the test first, using the `sim()` and `ball()` helpers in that file's
   tests. Put a ball somewhere, set the setting, call `step`, and check where
   the ball went. Also check it cannot push a ball out of the window or produce
   a NaN.
2. Add the field to `World`, its default in `World::new`, and a builder method
   with a doc comment, as `gravity` does.
3. Use it in `step`. Keep the maths in square units (rows count double) so balls
   stay round.
4. Mention it in [llms.txt](llms.txt) (the `tests/llms.rs` check will tell you
   if a method you listed does not exist).

## Add an example

An example is a short program in `examples/`, a few lines of body, that people
can copy and change.

1. Put it in `examples/NAME.rs`. Use `terminal_graphics_interactive::prelude::*`.
2. Add a scenario for it in [tests/support/mod.rs](tests/support/mod.rs), which
   runs on Windows, Linux and macOS: it should wait for something to be drawn,
   send some input, and check that `q` quits with exit code 0. Check that your
   scenario can fail by breaking the code on purpose, then undo that.
3. List it in the README.

## Update the GIFs

The animations in the README are made by a Rust program, so anyone can rebuild
them. They run the real source of `examples/plasma.rs` and `examples/balls.rs`
against the real library with a scripted mouse, and draw the arrow and click
rings on top. If you change one of those examples, or the art API's look,
regenerate them and commit the result:

```sh
cargo run --release --example make_gifs
```

The program is [examples/make_gifs.rs](examples/make_gifs.rs). To add a GIF,
add a `Take` there (the example to run and where the mouse goes) and show it in
the README.

## Formula art

For a new look, you usually do not need to change the library at all: write a
new example with a different formula. `sketch_frame` (in
[src/art/sketch.rs](src/art/sketch.rs)) lets you test a formula without a
terminal.

## Questions

Open an issue. If you are not sure whether an idea fits, ask first; the answer
is usually "yes, try it".

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

## Add a game example

A game example is a short program in `examples/` that plays with `Game`, `Next`
and friends from `game::*`. Start from [examples/glow.rs](examples/glow.rs), or
[examples/quickgame.rs](examples/quickgame.rs) for the smallest one.

1. Put it in `examples/NAME.rs` and import only
   `use terminal_graphics_interactive::game::*;` (it brings in the art prelude
   too).
2. Give it a `#[cfg(test)] mod tests` that plays it headless with `Headless`:
   the title shows how to start, a held key moves the player, and the round
   ends. `Headless::new(game).size(40, 12)`, `tap`, `press`, `step` and `text`
   are enough.
3. Declare it in [Cargo.toml](Cargo.toml) with `test = true`, as `glow` is:

   ```toml
   [[example]]
   name = "glow"
   path = "examples/glow.rs"
   test = true
   ```

   Then `cargo test --example NAME` runs its tests.
4. A new platform behaviour (raw mode, key release, the window size) is not
   supported until a pty test covers it. Add one to
   [tests/e2e_unix.rs](tests/e2e_unix.rs), next to the `glow` tests, and to
   `tests/e2e_windows.rs` for Windows.
5. Add a line to [CHANGELOG.md](CHANGELOG.md) under "Unreleased", list the game
   in the [README](README.md), and in [llms.txt](llms.txt) if it teaches
   something new.

## Add a game building block

A building block is a small helper type in `src/game/`, such as `Timer` or
`Pool`. It must stay pure: no terminal, no clock except the time it is given,
and no allocation in steady use where it can be avoided. Then:

1. Write its unit tests first, in the same file, for the edge cases (zero,
   negative, empty, full).
2. Give every public item a doc comment with a runnable example: a doc example,
   so the documentation is the test as well.
3. Declare the module in [src/game/mod.rs](src/game/mod.rs) and re-export the
   type there, so `game::*` brings it in.
4. Add it to the games section of [llms.txt](llms.txt) with one line on what it
   is for. `tests/llms.rs` checks that the functions it lists exist.

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

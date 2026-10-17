# terminal_graphics_interactive

The interactive layer for [`terminal_graphics`](https://github.com/calum/terminal_graphics_api):
keyboard and mouse input, a terminal session guard, and an event loop, so people
(and AI agents) can build games, art, animations and drawing tools in a terminal.
It re-exports `terminal_graphics`, so users add one dependency.

## Principles (why this project exists)

1. **AI-first.** The end consumer is most likely an AI agent writing code. Make
   the crate easy to pick and easy to use correctly:
   - `llms.txt` at the root: what it is, when to use it, when not to, one complete
     example. `tests/llms.rs` keeps it in sync with the API.
   - One import (`use terminal_graphics_interactive::prelude::*;`), few concepts,
     predictable names, no hidden global state.
   - Error messages and docs say what to do next ("stdin is not a terminal; run
     with `ssh -t`"), not just what went wrong.
   - Every public item has a doc comment with a runnable example.
   - Behaviour that is easy to get wrong (raw mode, cleanup, coordinates) is
     handled by the library, not left to the caller.
2. **Human fun first.** It started as a hobby project: keep it playful. Examples
   should be things people enjoy making (a paint program, a bouncing-ball
   screensaver, a tiny game), and getting a first one running should take minutes.
3. **High performance, clean Rust.** No per-event allocation where avoidable,
   no busy loops, small modules, no clever code.
4. **Exceptionally little unsafe.** Only the raw-mode/console-mode calls need it
   (`src/raw_mode/`, a few libc/kernel32 calls). Everything else is safe Rust
   (`#![deny(unsafe_code)]`, enforced by `tests/unsafe_audit.rs`). Each `unsafe`
   block has a `// SAFETY:` comment. Never add unsafe elsewhere.
5. **Zero dependencies besides `terminal_graphics`.** No other crates, ever
   (including dev-dependencies), and no non-Rust helper scripts.
6. **Works in Windows PowerShell as well as Linux/macOS terminals**, and over SSH
   (needs a pty; say so clearly when there isn't one).

## Performance

Performance is tested, not assumed (zero dependencies, so benchmarks are plain
`std::time` programs under `benches/` with `harness = false`, run by `cargo bench`).
- Every hot path has a benchmark: the parser (bytes per second, events per
  second, allocations per event), and later the event loop's latency.
- Record a baseline **before** optimising, change one thing at a time, and put
  the before/after numbers in the README so users can judge for themselves.
- Targets: parsing a stream of mouse events should be far faster than a terminal
  can produce them, and steady-state parsing should not allocate per event.
- Report numbers honestly, with the machine they came from. Do not claim a
  speed-up that is inside the noise.

## Process

- **TDD, always:** no production code without a failing test first. RED: write
  one test for one behaviour and watch it fail for the expected reason. GREEN:
  the minimum code to pass. REFACTOR with the suite green. Bug fixes start with
  a test that reproduces the bug. If a test turns out to be wrong, say so in the
  commit and fix the test, never bend the code around it.
- Test triangle: many unit tests beside the code (the parser is pure and heavily
  tested), some integration tests in `tests/`, very few end-to-end.
- Library code must not panic on bad input: unknown or truncated escape sequences
  are consumed and ignored, never wedge the parser.
- Raw mode and console modes need a real terminal, which CI and agent shells
  do not have. Keep everything that can be pure (flag arithmetic, error text,
  escape sequences, parsing) in safe functions with unit tests, and keep the
  `unsafe` shims too thin to need tests of their own. Say plainly what was and
  was not verified on a real console, per platform.
- **End-to-end tests run the real `events` example inside a pseudo terminal**
  and feed it raw key and mouse escape sequences: `tests/e2e_windows.rs` uses
  ConPTY (`kernel32`); a Unix equivalent (`posix_openpt`) belongs in
  `tests/e2e_unix.rs` and runs in the CI matrix (ubuntu, macos, windows). The pty
  harnesses are test-only, so their `unsafe` is allowed but must stay in those
  files. A new platform behaviour is not "supported" until an e2e test covers it.
  They cannot prove a physical mouse click becomes an escape sequence; that is
  the terminal emulator's job, so say so rather than overclaim.
- Before trusting an e2e test, mutate the library (for example drop a console
  flag) and check the test fails. Revert the mutation after.
- `tests/unsafe_audit.rs` fails the build if the word `unsafe` appears in
  `src/` outside `src/raw_mode/`. `lib.rs` uses `#![deny(unsafe_code)]` and only
  the platform shims under `src/raw_mode/` carry `#[allow(unsafe_code)]`.
- Warnings are errors in CI. `cargo clippy --all-targets`, `cargo fmt --all`.
- Every user-visible change goes in `CHANGELOG.md`.

## Releasing

CI publishes to crates.io when `master` has a `version` in `Cargo.toml` that is
not on crates.io yet (the `publish` job in `.github/workflows/ci.yml`). To
release, bump `version` and move the `## Unreleased` changelog entries under it
in a PR. Publishing cannot be undone, so:

- the token is the repository **secret** `CARGO_REGISTRY_TOKEN`, never a plain
  variable (variables are not masked in logs); `tests/ci_workflow.rs` checks this;
- it only runs on a push to `master`, after the tests and the MSRV check pass;
- run `cargo publish --dry-run` first.

## Layout

- `src/parser.rs`, `src/input.rs`, `src/terminal.rs`, `src/raw_mode/` - input, the session, raw mode (the only `unsafe`)
- `src/art/` - the easy art API: `palette.rs` (colours), `sketch.rs` (formula art), `world.rs` (physics), `stage.rs` (the frame loop and pointer). The maths is pure and unit tested; only `stage::run_loop` needs a terminal
- `examples/` - short programs people copy: `plasma`, `balls`, `paint`, `screensaver`, `events`, `quickstart`
- `tests/support/mod.rs` - the end-to-end scenarios shared by `e2e_windows.rs` and `e2e_unix.rs`
- `llms.txt` - the page for AI agents; `tests/llms.rs` keeps it, the README and the examples in step
- `CONTRIBUTING.md` - how to add a palette, a `World` setting or an example; `tests/contributing.rs` keeps it honest
- `docs/superpowers/specs/` - design notes
- `examples/make_gifs.rs` and `examples/site/` - the README GIFs: a pure Rust GIF encoder (`gif.rs`), a rasteriser with the mouse arrow (`raster.rs`), and a strict mini Rust interpreter (`script/`) that runs the real example sources, shared with the website editor. Regenerate with `cargo run --release --example make_gifs` after changing `plasma.rs` or `balls.rs`
- `examples/web_art.rs`, `examples/build_site.rs`, `examples/check_site.rs` - the website: the art engine as WebAssembly, the generator that writes `target/site` (the page, style and script are Rust strings, so the repo stays pure Rust), and a headless-browser check of the built page. `.github/workflows/pages.yml` builds, checks and deploys it to GitHub Pages on a push to `master`; `tests/pages_workflow.rs` keeps that safe. Build and try it locally with `cargo build --release --example web_art --target wasm32-unknown-unknown && cargo run --example build_site && cargo run --release --example check_site`
- `assets/` - the generated GIFs (not published to crates.io; the README points at their raw GitHub URLs)

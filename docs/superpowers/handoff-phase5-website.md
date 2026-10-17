# Hand-off prompt: build the website for Terminal Games (phase 5)

Paste everything below the line into a fresh coding agent.

---

You are continuing work on a Rust project in `calum/terminal_graphics_interactive` (a GitHub repo; use the GitHub MCP tools, there is no `gh` CLI). Work on branch `ccr-757a0aa2-e8ynbh` (it already exists on the remote; check it out and build on it; do not create a pull request unless I ask, and never push to another branch). Before anything else, read these files completely: `CLAUDE.md` (the project's rules; they override your defaults), `docs/superpowers/specs/2026-10-17-games-design.md` (the design and its Status section), `docs/superpowers/specs/2026-10-17-art-site-design.md` (how the existing art website works), `CONTRIBUTING.md`, `llms.txt`, and `CHANGELOG.md`.

## What this project is

`terminal_graphics_interactive` adds keyboard and mouse input, a terminal session guard and an event loop on top of `terminal_graphics`. It has two sections: **Terminal Art** (`sketch`, `World`, `Palette`: formulas and physics toys) and **Terminal Games** (`terminal_graphics_interactive::game::*`: `Game`, `Next`, `Context`, `Screen`, `PixelArt`, `Sprite`, `Keys`, `Controls`, `Rng`, `Timer`, `Pool`, `Hit`, `Waves`, `Particles`, `Headless`, `run`). Hard rules from the owner: **100% Rust, no dependencies other than `terminal_graphics` (not even dev-dependencies), no non-Rust scripts, almost no `unsafe`** (only `src/raw_mode/` and the pty test harnesses; `tests/unsafe_audit.rs` enforces it).

## State of the work (all committed and pushed; 855 tests pass)

Phases 1 to 4 are done:
1. Fixed-timestep clock (`src/clock.rs`) and a diffing `Renderer` (`Terminal::draw` sends only changed cells).
2. Kitty keyboard protocol: `Event::KeyUp`, `Terminal::enable_key_release`, `game::Keys` (held keys, with a timing fallback) and `game::Controls`.
3. The core game API (`src/game/`), the pure `Headless` runner (plays a game with no terminal and reads the screen as text), the real `run` loop, and docs in `llms.txt`/README/CONTRIBUTING.
4. Art in games: `Screen::backdrop(_in)`, `Waves`, `Particles`, the half-block **pixel layer** (`Screen::pixel/disc/glow/pixel_art/finish_frame/frame/clear_pixels`, `PixelArt`).

Examples: `examples/quickgame.rs`, `examples/glow.rs` (plasma sky and a glowing orb collecting motes) and `examples/sail.rs` (the flagship: a boat on procedural waves, falling fireballs that dissipate underwater, a day/night sky with sun and moon). The owner has played `glow` and `sail` in a real terminal and loves them. Owner preferences you must keep: game objects are **coloured regions** (pixel art, discs, glows), not text characters (text is for the HUD and labels); `q` and Esc belong to the game, only Ctrl+C always quits.

Not done: the website (your task), multiplayer (phase 6, later), real-terminal verification on Windows and macOS.

## Your task: the website (phase 5)

Decisions already made with the owner:
- `/` becomes a **landing page with two doors**: Terminal Art at `/art/` and Terminal Games at `/games/`, all on the same GitHub Pages site. The existing art page moves to `/art/`. Old art share links look like `/#<code>` (the code is in the URL hash), so the landing page must **forward a non-empty `#hash` to `/art/#hash`** (with a test).
- Both pages get one shared top bar (ART | GAMES | GitHub | crates.io) and the same retro look (CRT scanlines, rainbow title, the 16 ANSI colours). `examples/build_site.rs` (about 1,100 lines of Rust strings) must be split so the shared style, nav, rainbow title and colour classes are reused; the page, CSS and JS stay generated from Rust (no hand-written HTML/JS/CSS source files).
- **The games page plays the real compiled games, it does not interpret Rust.** Build a second WebAssembly module, `examples/web_games.rs` (a cdylib example like `examples/web_art.rs`; read that file and `examples/build_site.rs`, `examples/check_site.rs`, `.github/workflows/pages.yml`, `tests/pages_workflow.rs` first), so the art page does not download game code. The wasm module drives a game through `Headless`: JS calls `tick(ms)` each animation frame, an accumulator converts time into fixed 60 Hz `step`s, JS forwards key down/up and pointer events, and reads back the cells (character, foreground, background palette indexes, like `web_art`'s frame buffer) to paint on a canvas. Notes: browsers give true key-up events, so construct the keys in real-release mode; `std::time::Instant` and `SystemTime` panic on `wasm32-unknown-unknown`, so the seed comes from JS and nothing may read a clock (`Headless` already reads none); the games live in `examples/glow.rs`, `examples/sail.rs`, `examples/quickgame.rs`, so include them as modules the way `web_art.rs` includes `site/presets.rs` and expose a tiny `pub fn start() -> impl Game` in each (keep their behaviour and tests unchanged). Half-block characters (`▀ ▄ █ ░ ▒ ▓`) should be drawn as rectangles with transparency (not font glyphs) so they look identical in every browser, as the art page does.
- The games page contains: a gallery (glow, sail, quickgame), a canvas with keyboard focus (click to focus; show "click to play"), **on-screen touch buttons** for phones (left/right/enter/pause), the game's real source shown beside it (read-only, from the repo file, with a test that the embedded text equals the file, as the art page does for its examples), a short "make your own" section (`cargo add terminal_graphics_interactive`, the two-scene example from `llms.txt`), and a small **tuner**: sliders for a few numbers a game chooses to expose (for sail: fireball rate, wave height, boat speed; for glow: mote rate, round length) that change the running game. The tuner must work through a plain, documented hook (for example a `Tuning` struct the game reads), not by interpreting code. Multiplayer is not on the page yet; say so plainly.
- Workflows: `.github/workflows/pages.yml` builds both wasm modules and the site, `check_site` (a headless-browser check, see how it works and how it behaves when no browser is installed) must load `/`, `/art/` and `/games/`, check the old-link forwarding, start a game, press a key and see the frame change, and press the touch buttons. `tests/pages_workflow.rs` must be extended so deploys still happen only on a push to `master` after the site is built and checked. The deploy rules do not change.

## How to work (lessons from this project)

- **TDD, always** (`CLAUDE.md`): one failing test at a time, watch it fail for the expected reason, minimal code. Before trusting a test, mutate the code and see the test fail; revert the mutation. Warnings are errors: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (everything, including `tests/e2e_unix.rs`, `tests/llms.rs`, `tests/readme.rs`, `tests/contributing.rs`, `tests/pages_workflow.rs`, `tests/unsafe_audit.rs`). Every user-visible change goes in `CHANGELOG.md`. The README, `llms.txt` and `CONTRIBUTING.md` are guarded by tests that fail when they drift; `llms.txt`'s Games section is at its line limit, so do not add lines there without removing some.
- **Delegate with Haiku subagents** to save cost (the owner asked for this): give each a detailed written plan with exact files, API, behaviours and tests, tell it which files it owns, and tell it not to commit. Run independent agents in parallel only when they edit different files. **Review every agent report and re-run the checks yourself**; agents have skipped the one-test-at-a-time discipline, left stray files outside the repo (a stray `/glow_keep.rs` still exists at the filesystem root; the owner must delete it, do not try to remove it), and written tests that passed vacuously. Ask them to report honestly what was and was not verified and which tests were red first.
- **Commit and push only green states** to `ccr-757a0aa2-e8ynbh` with `git push -u origin ccr-757a0aa2-e8ynbh` (retry only on network errors, with backoff). End commit messages with the attribution lines your harness gives you. Do not commit while a background agent is mid-edit.
- Be honest in reports: say what was verified, on which machine, and what was not (no person has seen the site in a real browser; real-terminal colours are unchecked). No claims of speed-ups inside the noise.
- A cell is 1 column by 2 half-rows; pixel coordinates in the pixel layer are square. Pixel shapes composite above text when the frame finishes (`Scenes::draw` calls `finish_frame`); panels use `Screen::frame`, which erases pixels beneath it.

## Known small issues (not your task, mention them if relevant)

Motes or fireballs can cover the HUD row for a moment because pixel shapes composite above text; a few night stars can overlap a wave crest in a storm on a short terminal; the games' feel and difficulty are tuned only by the owner's play; Windows (ConPTY) and macOS have never run the games.

## When done

Report: what was built, the test and check results, screenshots or descriptions of the pages from the headless browser check, deviations, and what is not verified. Then ask the owner whether to open a pull request (do not open one unprompted), and propose phase 6 (multiplayer: `Sim` trait, `Encode`/`Decode`, `Transport` with `Local` and `Tcp`, lockstep with a desync hash, then a WebSocket transport for the browser; lockstep over TCP first) as the next step.

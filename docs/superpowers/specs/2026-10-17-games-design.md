# Terminal Games

## Status

Phases 1 to 5 are built. Phase 6 (multiplayer) is not.

- **Phase 1, built:** a shared fixed timestep clock (`clock::FixedStep`, used by the game loop and by the art stage) and a diffing `Renderer` behind `Terminal::draw`, with a benchmark and its numbers in the README.
- **Phase 2, built:** key release events from the kitty keyboard protocol, `Keys` (held, pressed and released; timed where the terminal sends no releases) and `Controls` (named actions that can be rebound).
- **Phase 3, built:** the core game API (`Game`, `Next`, `Context`, `Screen`, `Sprite`, `Rng`, `Timer`, `Pool`, `Hit`), `run`, the `glow` and `quickgame` examples, and the games section of `llms.txt`.

**The pixel layer (added with phase 4).** A `Screen` has a second layer of square pixels, two per cell row, so a cell can show two of them with `▀` or `▄` (or one solid cell when they match). `Screen::pixel`, `disc` and `pixel_art` (a `PixelArt`, a picture in pixels made from text and a legend) draw into that layer, and `finish_frame` composites it when the frame ends, so pixels show above text in the same frame. `Particles` draw as pixels. `glow` is different: it blends a soft light straight into the cells behind it, with shade blocks (`░ ▒ ▓`) that keep the background. The rule for the games section: objects are coloured regions (`PixelArt`, `disc`, `glow`, `Particles`), not text characters; text is for the HUD and labels.

`Headless` was not in the first sketch. It runs the same `Game` code with no terminal, so tests, replays and AI agents can play a game and read back what it drew. It is the first step toward the headless `Sim` and bot play in phase 6.

- **Phase 4, built:** `Screen::backdrop(_in)`, `Waves`, `Particles`, the pixel layer and the flagship `sail`.
- **Phase 5, built:** the site is three pages (landing at `/`, art at `/art/`, games at `/games/`). `examples/web_games.rs` is a second WebAssembly module that drives `glow`, `sail` and `quickgame` through `Headless` (fixed 60 Hz steps from `tick(ms)`, key down and up, pointer). The games page has the gallery, touch buttons (left, right, up, down, enter, pause), the real source beside the game, a "make your own" section and a tuner. The tuner uses `game::Tuning`: a plain hook (`tuning.get(name, fallback)`) that games read, no interpreter. `check_site` loads all three pages in a headless browser. No person has looked at the pages in a real browser yet.

Not done yet:

- **Phase 6, multiplayer:** `Sim`, `Encode`/`Decode`, the `Local` and `Tcp` transports, lockstep and desync hashes.
- **Real-terminal verification on Windows and macOS:** no person has run the games there yet, and `quickgame` has no pty test.
- **Tuning the feel of `glow`:** the speeds, key timeouts and ripple strength are first guesses.

## Why

Terminal Art is "write a formula, see something pretty". Terminal Games is the
next step up: "write the rules, get something you can play". Same audience (people
and AI agents), same promises: one import, few concepts, 100% Rust, almost no
`unsafe`, and fast. The new part is that a game keeps state, reacts to input in a
fixed rhythm, has several screens (menu, play, pause, game over), and one day may
have more than one player.

The moment we design for: someone asks for "a boat sailing on waves, dodging
fireballs, with a title menu". The sea should be an art formula, the fireballs
should be art particles, and the whole thing should be about 100 lines they can
read in one go. A second person should later be able to join it over the network
without the first rewriting anything.

## One crate, two sections

Keep one crate and one dependency line. Terminal Art and Terminal Games are two
**sections** of it, not two crates:

```
terminal_graphics_interactive
  core   Terminal, Event, Parser, RawMode          (exists)
  art    sketch, World, Palette                    (exists)
  game   Game, Scene, Input, Sprite, Rng, net      (new)
```

- `use terminal_graphics_interactive::prelude::*;` stays exactly as it is, so no
  art script changes. The prelude does **not** grow, which keeps the art
  vocabulary small.
- Games use `use terminal_graphics_interactive::game::*;`, which re-exports the
  art prelude too, because games are meant to use art.
- No cargo features: zero dependencies means unused modules cost only compile
  time. `net` uses `std::net` only.

Rejected: a second crate. It splits docs, versions, the CI publish job and
`llms.txt`, and a game cannot use art types without a dependency between them
anyway. If the games section outgrows this we can split later, because `game`
only uses the public art API.

## What is missing today (found by reading the code)

1. **The loop is art-shaped.** `art::stage::run_loop` is a fixed 33 ms frame
   that calls one closure. Games need fixed-timestep updates (so physics and
   network play do not depend on how fast the machine is) separate from drawing,
   plus scene switching. It should move to a shared `loop` module that art also
   uses.
2. **No key-release events.** Terminals send key presses and, with a few seconds
   of repeat delay, repeats. A game that asks "is Left held?" must emulate it.
   Plan: `Input::held(key)` is true from a press until no repeat has arrived for
   a short timeout (about 120 ms, after the initial repeat delay is covered by a
   longer first timeout). Where the terminal supports the kitty keyboard protocol
   we ask for real release events and use them. This is the biggest platform risk
   and gets its own e2e test per OS. Honest limit to document: on terminals
   without the protocol, a held key has a small latency when released.
3. **`Terminal::draw` redraws everything.** `frame_for(display)` builds a whole
   frame string every call. At 60 updates a second over SSH that is wasteful.
   Games need a diffing renderer that only writes changed cells (and merges runs
   with the same colours). It is the first thing to benchmark and the first
   optimisation, with before and after numbers in the README, as `CLAUDE.md` asks.
4. **No deterministic random numbers.** `World` has its own small generator
   (seeded). Games, replays and lockstep multiplayer need a public one.

## The API

Two ideas to learn: a `Game` is a screen of your game, and a `Screen` is where you
draw. The rest are small helpers.

```rust
use terminal_graphics_interactive::game::*;

struct Menu;
struct Sail { boat: Boat, fireballs: Vec<Fireball>, rng: Rng, score: u32 }

impl Game for Menu {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) { return Next::switch(Sail::new(ctx.seed)); }
        Next::stay()
    }
    fn draw(&self, screen: &mut Screen) {
        screen.backdrop(Palette::ocean(), sea);          // an art formula
        screen.title("SAIL!");
        screen.text_centred(screen.height() / 2, "press Enter");
    }
}

fn main() -> std::io::Result<()> { run(Menu) }
```

### The core

| Piece | What it is |
| --- | --- |
| `Game` trait | `update(&mut self, ctx: &mut Context<'_>) -> Next` and `draw(&self, &mut Screen)`. Both have defaults, so a screen can be only a `draw`. |
| `Next` | What happens after an update: `stay()`, `switch(game)`, `push(game)` (pause over play), `pop()`, `quit()`. A stack of boxed `Game`s is the scene system; there is no scene graph. |
| `Context` | Everything an update needs and nothing it should own: `keys`, `pointer`, `dt` (a fixed step, always 1/60), `tick` (frame count), `size`, and `seed` (a field). `time()` is `tick * dt`. |
| `Keys` and `Controls` | `Keys` gives `pressed(key)` (this tick), `held(key)` (see above), `released(key)` and `axis(negative, positive)` -> -1/0/1. `Controls` binds named actions to keys so players can rebind them, and asks `held(&keys, action)`. |
| `Screen` | A `Display` plus drawing helpers: `clear`, `put`, `fill`, `bar`, `sprite`, `text`, `text_centred`, `put_on`, plus `time()` and `tick()`. Everything is clipped to the screen. `backdrop`, `particles` and `half_blocks()` (twice the vertical resolution, the `▀` trick) are phase 4. |
| `Rng` | Small seeded splitmix64 generator (`range_i32`, `f32`, `chance`, `pick`, `shuffle`), no allocation, same output on every platform. |
| `Sprite` | A grid of characters and colours made from a string literal; `paint` recolours it and `flipped` mirrors it. Frame animation is not built yet. |
| `Timer`, `Hit` | Cooldowns and tiny collision tests: `Hit` is an enum with `Hit::rect` and `Hit::circle`. No physics engine. |
| `run(game)` | Starts the `Terminal`, runs the loop, restores everything. Same quitting rules as art (Ctrl+C always works; `q` and Esc are the game's to use, so `run` does **not** swallow them). |

Design rules:

- **No ECS, no macros, no traits you must understand.** State is plain structs
  and `Vec`s. We give a `Pool<T>` (fixed capacity, reuse slots) so fireballs do
  not allocate per frame.
- **Fixed timestep.** `update` runs at 60 Hz regardless of draw speed (at most a
  few catch-up steps, never a death spiral). `draw` gets an `alpha` for smooth
  motion if the game wants it.
- **`draw` takes `&self`.** Drawing cannot change the game. That one rule is what
  makes the networking and testing below possible.

As built, the sketch differs in three places: input is `ctx.keys` (and `ctx.pointer`), `ctx.seed` is a field, and `backdrop` and `title` are phase 4 helpers that do not exist yet. The rest is the API you can use today.

### Using the art modules in games

The art section's pieces become **layers** a game composes. This is the point of
the whole project, so it has to be easy and has to share maths with the rules.

- `screen.backdrop(palette, |cell| ...)` runs a `sketch` formula as the
  background. The same formula body can be a plain function:

  ```rust
  fn wave(x: f32, t: f32) -> f32 { (x * 0.08 + t).sin() * 0.5 + (x * 0.19 - t * 1.3).sin() * 0.25 }
  fn sea(cell: &Cell) -> f32 { ((cell.height / 2.0 + wave(cell.x, cell.t) * 6.0 - cell.y) * 0.25).clamp(-1.0, 1.0) }
  ```

  The boat's `update` calls `wave(boat.x, ctx.time())` to sit on the surface and
  tilt with the slope. **The art and the physics read the same function**, so the
  boat cannot float in the air. This is the pattern to teach in the docs.
- `Particles` (built from the `World` engine): `emit` fire, sparks, spray;
  `screen.particles(&p)`. Fireballs are a sprite plus a trailing emitter.
- `Palette` is used as-is for HUD colours, damage flashes and day/night shifts.
- Any `Simulation` can be a game object: step it in `update`, render it in `draw`.

To support this, `sketch_frame` and `Simulation::render` already take a
`&mut Display`, so they need a region argument (draw into a sub-rectangle) and not
much else. That is the one change to existing art code.

## Multiplayer without limiting anything

We do not build multiplayer first, but three rules now keep it possible:

1. **Separate the simulation from the view.** Anything that affects the rules
   lives in a type implementing `Sim`:

   ```rust
   trait Sim { type Input: Encode + Decode; fn step(&mut self, inputs: &[Self::Input], rng: &mut Rng); }
   ```

   `Game::update` for a simple one-player game is just "read `Input`, call
   `step`". `draw` only reads. A `Sim` is deterministic: same state, same inputs,
   same seed gives the same result on every machine. That one property gives us
   lockstep play, rollback, replays, save games, and **headless tests and AI
   bots** (an agent can play a `Sim` with no terminal at all).
2. **Players are numbered, inputs are data.** `Context` always carries
   `players: usize` and `me: PlayerId`; local play is just one player. Input is
   turned into a small `Encode`/`Decode` value (hand-written little-endian
   bytes, no serde) before the game sees it.
3. **The network is a trait.**

   ```rust
   trait Transport { fn send(&mut self, to: PeerId, bytes: &[u8]) -> io::Result<()>; fn poll(&mut self) -> io::Result<Option<(PeerId, Vec<u8>)>>; }
   ```

   Planned implementations, in order: `Local` (in-process, for tests and
   hot-seat), `Tcp` (`std::net`, non-blocking, no dependency), later `WebSocket`
   (a small safe-Rust implementation, SHA-1 and base64 included, for the browser).

On those foundations these models all fit, and none is baked in:

| Model | Good for | What it needs |
| --- | --- | --- |
| Hot-seat / split keys | Two on one keyboard | Nothing extra |
| Lockstep | Small deterministic games (the first one we ship) | `Sim`, `Rng`, `Transport`, input delay |
| Authoritative server | Many players, cheating matters | A headless `serve(sim)` run (same binary, `--serve`), snapshots via `Encode` |
| Rollback | Fast action games | `Sim: Clone`, which is why state should be plain data |
| Thin client (server draws, clients see cells) | Zero-install play over SSH or telnet | The `Display` -> bytes path already exists |

Desync detection comes free: peers exchange a hash of the state every N ticks;
a mismatch names the tick, which makes "my game differs from yours" debuggable.
Chat and lobbies are out of scope for the library; a `Transport` plus `Sim` is
enough for someone to write them.

## Performance plan

Per `CLAUDE.md`: benchmark first, change one thing, publish numbers with the
machine.

- New `benches/game.rs` (plain `std::time`, `harness = false`): `update` ticks per
  second for 1,000 pooled objects, `draw` into a `Screen` per second, bytes
  written per frame (full vs diffed), and allocations per tick (target: zero in
  steady state, measured with a counting `GlobalAlloc` in the bench only).
- Baseline before the diffing renderer, then after. Target: a full 200x60
  screen at 60 Hz with under a few percent of one core, and a mostly-still
  screen sending tens of bytes a frame, not tens of kilobytes.
- No busy loops: the loop sleeps in `poll(timeout)` until the next tick, as art
  does today.

## `unsafe`

None added. The loop, input emulation, sprites, `Rng`, `Sim` and `net` are safe
Rust (`std::net` is safe). `tests/unsafe_audit.rs` keeps enforcing it. The kitty
keyboard protocol is an escape sequence, not a syscall.

## Process and tests

- TDD as always. The testable core is pure: the fixed-timestep accumulator, held
  key emulation (feed a fake clock), `Next` stack transitions, `Rng` sequences,
  `Pool`, collision, the diffing renderer (old frame + new frame = exact bytes),
  and `Encode`/`Decode` round trips.
- **Replay tests:** a seeded game run with scripted inputs must end at a recorded
  state hash. These double as the desync check and run in milliseconds with no
  terminal.
- **`Local` transport tests:** two `Sim`s fed through a lossy, reordering fake
  link must agree.
- **E2E** (`tests/e2e_unix.rs`, `e2e_windows.rs`, shared scenarios in
  `tests/support`): run the real `sail` example in the pty, press Enter on the
  menu, hold an arrow key, check the boat moved and the screen left no stray
  state on exit. Mutate-and-check as `CLAUDE.md` requires. Not provable by us:
  that a particular terminal sends kitty release events; say so.
- `llms.txt` gets a games section (when to pick games vs art, one full example);
  `tests/llms.rs` keeps it in step. Every public item has a runnable doc example.

## Examples (human fun first)

In build order, each a short file in `examples/`:

1. `glow` - the smallest game, in about 50 lines: steer a light through a
   drifting plasma sky and collect motes that leave glowing trails. Proves the
   core API and the art backdrop in minutes.
2. `sail` - **the flagship**: title menu, boat riding `wave()` on an ocean
   backdrop whose sky shifts from dawn to night, fireballs with particle
   trails, spray, score, pause (`push`), game over. Uses `Pool`, `Particles`,
   `Palette`.
3. `sail_together` - `sail` over `Tcp` lockstep, for the multiplayer milestone.

## The website: two pages, one URL

The site is currently one page. Make it a small site:

```
https://calum.github.io/terminal_graphics_interactive/          landing page: two doors (forwards old #share links to /art/)
https://calum.github.io/terminal_graphics_interactive/art/      Terminal Art   (the current page, moved)
https://calum.github.io/terminal_graphics_interactive/games/    Terminal Games (new)
```

- Both pages get the same top bar: **ART | GAMES | GitHub | crates.io**, and the
  same retro look. `build_site.rs` (1,100 lines, mostly strings) is split so the
  shared style, rainbow title, nav and colour classes live in `examples/site/` and
  each page has its own module. `target/site/` then contains `index.html` and
  `games/index.html`. Pages stay Rust strings, so the repo stays pure Rust.
- **Why not an editor for games too?** The art editor works because a formula is
  a tiny subset of Rust that `examples/site/script` interprets. Games need
  structs, `impl`, `Vec`, traits, which is much more of a language to
  interpret strictly. Decision: **the games page plays the real compiled games**,
  not interpreted ones. `examples/web_games.rs` is a second wasm module (so the
  art page does not download game code) that runs the same `Game` code the
  terminal runs, through a `Screen` -> canvas path like `web_art`'s.
- The games page has: a gallery (glow, sail), a canvas with keyboard focus
  and **on-screen touch buttons** on phones, the game's source shown beside it
  (the real example file, tested to match, as for art), and a **tuner**: sliders for the
  numbers a game chooses to expose (fireball rate, wave height, boat speed) that
  change the running game. That keeps "change a number and watch it" without an
  interpreter. A later phase can extend the interpreter to structs if it proves
  worth it.
- Multiplayer on the page waits for the `WebSocket` transport. Until then the
  page says plainly that two-player games are on the keyboard.
- Workflow: `pages.yml` builds both wasm modules and the site, `check_site` loads
  both pages in the headless browser (games page: boat game starts, a key press
  changes the frame, touch buttons work), and deploys only on `master` as today.
  `tests/pages_workflow.rs` is extended to assert both pages are checked. The
  deploy rules do not change.

## Phases (each shippable, each its own PR)

1. **Foundations (no new user-facing API).** Shared loop module with fixed
   timestep; art moved onto it; diffing renderer with benchmark and README
   numbers. Art behaviour unchanged.
2. **Input.** `Input`, held-key emulation, kitty protocol where available, e2e
   on all three OSes. Ship `Controls`.
3. **Core game API.** `Game`, `Next`, `Context`, `Screen`, `Rng`, `Sprite`,
   `Pool`, `Timer`. Example `glow`. `llms.txt` games section.
4. **Art in games.** Regions for `sketch_frame`, `backdrop`, `Particles`.
   Flagship `sail`.
5. **Website** (only once `sail` works). Shared page modules, `/games/`, `web_games`, tuner, touch
   controls, `check_site` coverage.
6. **Multiplayer.** `Sim`, `Encode`/`Decode`, `Local` and `Tcp` transports,
   lockstep, desync hash, `sail_together`, then server mode, then `WebSocket`.

Phases 0 to 3 are about the game library and can ship as 0.2; the site follows;
multiplayer is 0.3. Version bumps and changelog follow the release rules in
`CLAUDE.md`.

## Non-goals

- A full engine: no ECS, scene editor, asset pipeline, or physics engine beyond
  the art section's.
- Audio (the terminal bell at most).
- Matchmaking, accounts, chat, anti-cheat beyond the authoritative-server model.
- Pixel-perfect graphics: this is cells, with half-blocks as the upper limit.

## Decisions (settled with the owner)

1. Terminal Games stays a section of this crate; the crate is not renamed.
2. In games `q` and Esc belong to the game; only Ctrl+C always quits.
3. The site root `/` becomes a **landing page** with two doors: Terminal Art at
   `/art/` and Terminal Games at `/games/`. The existing art page moves to
   `/art/`; its `#share` links break unless `/` forwards a `#hash` to `/art/`
   (the landing page does that, with a test).
4. Multiplayer starts with lockstep over TCP (my recommendation).
5. The website waits until there is a fully working flagship game. Pong is
   dropped as a flagship: the examples should be unique and arty.
6. 100% Rust and no dependencies other than `terminal_graphics`, ever.

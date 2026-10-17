//! SAIL: a boat rides procedural waves under a sky that turns from dawn to day
//! to dusk to night. Fireballs skim the water and leave trails of embers; dodge
//! them by steering, or hop over the low ones. Three lives, a score for every
//! tenth of a second you survive, and a bonus when one passes close.
//!
//! A and D (or the arrows) steer, Space, W or Up hops, p or Esc pauses, q quits.

use std::f32::consts::TAU;
use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(Title::new(0))
}

/// Seconds of play in one day, from dawn to dawn.
const DAY_SECONDS: f32 = 120.0;

/// How rough the sea is at `seconds` of play: 1.0 when calm, rising to 2.2 at
/// 90 seconds and staying there. Scales the swell in [`sea`].
fn roughness(seconds: f32) -> f32 {
    1.0 + 1.2 * storm(seconds)
}

/// How far the game has got toward its hardest level, from 0 at the start to
/// 1 at 90 seconds of play, and 1 after that. Roughness and the rate of
/// fireballs both follow it.
fn storm(seconds: f32) -> f32 {
    let ramp = if seconds.is_finite() {
        seconds / STORM_SECONDS
    } else {
        0.0
    };
    ramp.clamp(0.0, 1.0)
}

/// Seconds of play over which the game gets from calm to stormy.
const STORM_SECONDS: f32 = 90.0;

/// The sea at `seconds` of play: three travelling swells, scaled by roughness.
fn sea(seconds: f32) -> Waves {
    let k = roughness(seconds);
    Waves::new()
        .layer(0.9 * k, 18.0, 0.07)
        .layer(0.45 * k, 7.0, 0.15)
        .layer(0.2 * k, 3.1, 0.3)
}

/// Seconds between fireballs at the start, and at the storm.
const FIRST_SPAWN: f32 = 1.6;
const STORM_SPAWN: f32 = 0.45;

/// Seconds between fireballs at `seconds` of play. It falls as the game goes on.
fn spawn_interval(seconds: f32) -> f32 {
    FIRST_SPAWN + (STORM_SPAWN - FIRST_SPAWN) * storm(seconds)
}

/// How bright the sky is, from 0 (midnight) to 1 (midday), at `seconds` of play.
/// A day lasts 120 seconds and starts at dawn, so the sky is at its brightest
/// at 30 seconds and darkest at 90.
fn daylight(seconds: f32) -> f32 {
    if !seconds.is_finite() {
        return 0.5;
    }
    let phase = (seconds / DAY_SECONDS).rem_euclid(1.0);
    (phase * TAU).sin() * 0.5 + 0.5
}

/// How dark the sky is, from 0 (no night) to 1 (midnight), given the daylight
/// from [`daylight`]. It is zero from dawn to dusk and grows toward midnight.
/// Stars fade in while it is above 0.4.
fn night(daylight: f32) -> f32 {
    ((0.5 - daylight.clamp(0.0, 1.0)) * 2.0).clamp(0.0, 1.0)
}

/// Where the still water sits, as a share of the screen height from the top.
const WATER_LEVEL: f32 = 0.64;

/// The row of the still water on a screen `rows` high.
fn level(rows: f32) -> f32 {
    WATER_LEVEL * rows
}

/// The boat is this many columns wide and its sprite this many rows tall; its
/// hull is the bottom row.
const BOAT_WIDTH: f32 = 8.0;
const BOAT_ROWS: f32 = 4.0;

/// Columns a second the boat sails, how quickly its speed eases toward that, and
/// the leftmost column it may use. It may go as far right as 0.6 of the width.
const SAIL_SPEED: f32 = 18.0;
const SAIL_EASE: f32 = 4.0;
const LEFT_EDGE: f32 = 6.0;

/// How many rows a hop lifts the boat, and how fast gravity pulls it back, in
/// rows a second each second. A hop lasts about 1.15 seconds.
const HOP_HEIGHT: f32 = 5.0;
const GRAVITY: f32 = 30.0;

/// What the keys can ask for during play.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Left,
    Right,
    Hop,
    Pause,
}

fn controls() -> Controls<Action> {
    Controls::new()
        .bind(Action::Left, &[KeyCode::Left, KeyCode::Char('a')])
        .bind(Action::Right, &[KeyCode::Right, KeyCode::Char('d')])
        .bind(
            Action::Hop,
            &[KeyCode::Char(' '), KeyCode::Up, KeyCode::Char('w')],
        )
        .bind(Action::Pause, &[KeyCode::Char('p'), KeyCode::Esc])
}

/// How fast the boat's hull chases the water, as a spring: stiffness and damping.
const SPRING: f32 = 36.0;
const DAMPING: f32 = 7.0;

/// The boat: `x` is its left column, `y` is the row of its hull's bottom, and
/// `air` is how many rows it is above the water while it hops.
#[derive(Clone, Copy, Debug)]
struct Boat {
    x: f32,
    vx: f32,
    y: f32,
    vy: f32,
    air: f32,
    air_v: f32,
}

impl Boat {
    /// A boat not yet on the water. Its `y` is NaN until the first update
    /// places it, so nothing can read a position before the sea exists.
    fn new() -> Boat {
        Boat {
            x: f32::NAN,
            vx: 0.0,
            y: f32::NAN,
            vy: 0.0,
            air: 0.0,
            air_v: 0.0,
        }
    }

    /// The column of the middle of the hull.
    fn centre(&self) -> f32 {
        self.x + BOAT_WIDTH / 2.0
    }

    /// On the first call, puts the boat a third of the way across, on the
    /// water. After that it does nothing.
    fn place(&mut self, width: f32, surface: impl Fn(f32) -> f32) {
        if self.y.is_nan() {
            self.x = (width * 0.3).max(0.0);
            self.y = surface(self.centre());
            self.vy = 0.0;
        }
    }

    /// Eases the speed toward `wanted` columns a second, then moves the boat
    /// and keeps its left column between [`LEFT_EDGE`] and `far`. Stopping at
    /// an edge also stops the boat, so it does not push against it.
    fn steer(&mut self, wanted: f32, dt: f32, far: f32) {
        let ease = (dt * SAIL_EASE).min(1.0);
        self.vx += (wanted - self.vx) * ease;
        // min then max, not clamp: clamp panics when its bounds cross
        let moved = (self.x + self.vx * dt).min(far).max(LEFT_EDGE);
        if moved != self.x + self.vx * dt {
            self.vx = 0.0;
        }
        self.x = moved;
    }

    /// The hull as a rectangle for collisions: the sprite's rows, with columns
    /// halved to match [`Fireball::shape`].
    fn hull_hit(&self) -> Hit {
        let (top, _) = self.body_rows();
        Hit::rect(self.x * 0.5, top, BOAT_WIDTH * 0.5, BOAT_ROWS)
    }

    /// The top and bottom rows of the sprite (the bottom is exclusive).
    fn body_rows(&self) -> (f32, f32) {
        let top = self.y - (BOAT_ROWS - 1.0);
        (top, top + BOAT_ROWS)
    }

    /// Starts a hop if the boat is on the water. Returns whether it did, so a
    /// hop pressed in mid air does nothing.
    fn hop(&mut self) -> bool {
        if self.air > 0.0 || self.air_v != 0.0 {
            return false;
        }
        self.air_v = (2.0 * GRAVITY * HOP_HEIGHT).sqrt();
        true
    }

    /// Carries a hop on by one step: gravity pulls the boat back down. Returns
    /// true on the step it lands, when the boat is back on the water.
    fn fly(&mut self, dt: f32) -> bool {
        if self.air == 0.0 && self.air_v == 0.0 {
            return false;
        }
        self.air_v -= GRAVITY * dt;
        self.air += self.air_v * dt;
        if self.air <= 0.0 {
            self.air = 0.0;
            self.air_v = 0.0;
            return true;
        }
        false
    }

    /// Pulls the hull toward `target` (a row), with a spring that makes it bob
    /// rather than snap.
    fn settle(&mut self, target: f32, dt: f32) {
        let accel = SPRING * (target - self.y) - DAMPING * self.vy;
        self.vy += accel * dt;
        self.y += self.vy * dt;
    }
}

/// What a fireball has just done, decided each update and acted on at once.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fate {
    Flying,
    Hit,
    Splashed,
    Gone,
}

/// A fireball this far past the left edge (in columns) has left the screen.
const OFF_LEFT: f32 = -3.0;

/// A fireball. `x` and `y` are in cells and move `vx` columns and `vy` rows a
/// second; `gravity` pulls `vy` down each second, so a lobbed one arcs.
/// `passed` is set once it crosses the boat's column, so its near miss counts once.
#[derive(Clone, Copy, Debug)]
struct Fireball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    gravity: f32,
    passed: bool,
    fate: Fate,
}

impl Fireball {
    /// The fireball as a circle for collisions. Columns are halved, so the
    /// circle is round on screen.
    fn shape(&self) -> Hit {
        Hit::circle(self.x * 0.5, self.y, FIREBALL_RADIUS)
    }
}

/// The most fireballs in the air at once, their radius in rows, and how fast a
/// lobbed one falls (rows a second each second).
const FIREBALL_LIMIT: usize = 12;
const FIREBALL_RADIUS: f32 = 1.0;
const FIREBALL_GRAVITY: f32 = 12.0;

/// A fireball that passes the boat within this many rows, without touching it,
/// scores this much.
const NEAR_ROWS: f32 = 3.0;
const NEAR_BONUS: u32 = 5;

/// The boat starts with this many lives, and is safe for this long after a hit.
const LIVES: u32 = 3;
const INVULNERABLE_SECONDS: f32 = 1.5;
/// How long the screen flashes after a hit.
const FLASH_SECONDS: f32 = 0.15;
/// How long each on or off slot of an invulnerable boat's blinking lasts.
const BLINK_SECONDS: f32 = 0.08;

/// A "+5" that rises from the boat and fades.
#[derive(Clone, Copy, Debug)]
struct Floater {
    x: f32,
    y: f32,
    age: f32,
}

/// The most floating texts at once, how many rows a second they rise, and how
/// long they last in seconds.
const FLOATER_LIMIT: usize = 8;
const FLOATER_RISE: f32 = 2.0;
const FLOATER_LIFE: f32 = 0.8;

/// The value of the sky at one cell, from -1 (darkest) to 1 (brightest), for
/// the given daylight, flash and play time. The palette ramps from night colours
/// up through dawn and day, and a cell near the horizon is brighter.
fn sky_value(cell: &Cell, daylight: f32, flash: f32, t: f32) -> f32 {
    // 0 at the top of the sky, 1 at the horizon
    let horizon = if cell.height > 0.0 {
        (cell.y / cell.height).clamp(0.0, 1.0)
    } else {
        1.0
    };
    // two slow sines make soft cloud bands, which drift with play time
    let clouds =
        ((cell.x * 0.09 + t * 0.3).sin() + (cell.x * 0.05 - cell.y * 0.2 + t * 0.2).sin()) * 0.25;
    let value = 0.7 * daylight + 0.3 * horizon + 0.1 * clouds + 0.35 * flash;
    (value * 2.0 - 1.0).clamp(-1.0, 1.0)
}

/// The value of the sea at column `x` and row `y`, from -1 (darkest) to 1
/// (brightest), for a screen `rows` high. The surface is bright with foam and
/// glints, the deep water darker, and the bottom of the screen darkest.
fn sea_value(x: f32, y: f32, waves: &Waves, t: f32, rows: f32, flash: f32) -> f32 {
    let rows = rows.max(1.0); // a zero-height screen must not divide by zero
    let level = level(rows);
    let surface = waves.surface_y(x, t, level);
    let depth = waves.shade(x, y, t, level); // 0 at the surface, 1 well below
    let under = y - surface; // rows below the surface
    let foam = if under < 1.0 { 0.6 } else { 0.0 };
    let shimmer = 0.1 * (x * 0.7 + t * 2.0 + y * 1.3).sin();
    let glint = if under < 2.0 && glints(x as i32, y as i32, t) {
        0.6
    } else {
        0.0
    };
    let dark = ((y - rows * 0.85) / (rows * 0.15)).clamp(0.0, 1.0) * 0.4;
    (0.2 - 0.9 * depth + foam + shimmer + glint + 0.2 * flash - dark).clamp(-1.0, 1.0)
}

/// Whether the water at cell `(x, y)` glints at play time `t`. A hash of the
/// cell and the quarter second decides, so a glint lasts a moment and is rare.
fn glints(x: i32, y: i32, t: f32) -> bool {
    let quarter = (t * 4.0).floor() as i32;
    mix(x, y, quarter) % 997 == 0
}

/// A cheap, deterministic hash of a cell and a salt, using only wrapping
/// arithmetic, so the same inputs always give the same bits and nothing panics.
fn mix(x: i32, y: i32, salt: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ (salt as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// Stars over the sky above `sky_bottom`, twinkling with play time `t`. Only at
/// night: they fade in past a night amount of 0.4 and are fullest at midnight.
fn paint_stars(screen: &mut Screen, night: f32, t: f32, sky_bottom: f32) {
    if night <= STAR_NIGHT {
        return;
    }
    let fullness = ((night - STAR_NIGHT) / (1.0 - STAR_NIGHT)).clamp(0.0, 1.0);
    // out of every thousand sky cells, this many are stars at full night
    let chance = (fullness * 30.0) as u32;
    let colour = if fullness > 0.6 {
        Colour::BrightWhite
    } else {
        Colour::White
    };
    let twinkle = (t * 2.0).floor() as i32;
    for y in 0..sky_bottom.floor() as i32 {
        for x in 0..screen.width() {
            if mix(x, y, 0) % 1000 < chance {
                // a star is a dot or a point, and flicks between them
                let ch = if mix(x, y, twinkle) % 3 == 0 {
                    '*'
                } else {
                    '.'
                };
                screen.put(x, y, ch, colour);
            }
        }
    }
}

/// The night amount above which stars start to show.
const STAR_NIGHT: f32 = 0.4;

/// The name of the time of day at `seconds` of play, for the HUD.
fn time_of_day(seconds: f32) -> &'static str {
    let light = daylight(seconds);
    let phase = (seconds / DAY_SECONDS).rem_euclid(1.0);
    if night(light) > 0.5 {
        "night"
    } else if light >= 0.85 {
        "day"
    } else if phase < 0.25 {
        // the first quarter of the day rises from dawn, the second falls to dusk
        "dawn"
    } else {
        "dusk"
    }
}

/// The boat, leaning left, level and leaning right. The hull is the bottom row.
const BOAT_LEFT: &str = r"
  ##|
 ###|
\_____/
";
const BOAT_LEVEL: &str = r"
   |##
   |###
\_____/
";
const BOAT_RIGHT: &str = r"
   |#
   |###
   |####
\_____/
";

/// The colours and pictures the game draws with. Made once per scene, so that
/// drawing allocates nothing.
struct Look {
    sky: Palette,
    sea: Palette,
    /// Indexed by [`tilt_index`].
    boats: [Sprite; 3],
}

impl Look {
    fn new() -> Look {
        Look {
            sky: Palette::new(&[
                Colour::Black,
                Colour::Blue,
                Colour::Magenta,
                Colour::Red,
                Colour::BrightRed,
                Colour::Yellow,
                Colour::BrightYellow,
                Colour::BrightBlue,
                Colour::Cyan,
                Colour::BrightCyan,
                Colour::BrightWhite,
            ]),
            sea: Palette::ocean(),
            boats: [
                boat_sprite(BOAT_LEFT),
                boat_sprite(BOAT_LEVEL),
                boat_sprite(BOAT_RIGHT),
            ],
        }
    }
}

/// A boat picture: a red hull, a yellow mast and a white sail.
fn boat_sprite(art: &str) -> Sprite {
    Sprite::new(art, Colour::Red)
        .paint('#', Colour::BrightWhite)
        .paint('|', Colour::Yellow)
}

/// Draws the boat with its column `x` and its hull on row `hull` (the bottom
/// row of its picture), in the picture for the way the water leans under it.
fn draw_boat(screen: &mut Screen, look: &Look, x: f32, hull: f32, tilt: Tilt) {
    let sprite = &look.boats[tilt_index(tilt)];
    let top = hull.round() as i32 - (BOAT_ROWS as i32 - 1);
    screen.sprite(x.round() as i32, top, sprite);
}

/// Which of the three boat pictures leans which way.
fn tilt_index(tilt: Tilt) -> usize {
    match tilt {
        Tilt::Left => 0,
        Tilt::Level => 1,
        Tilt::Right => 2,
    }
}

/// Paints the sky over the top `horizon` rows with the sky formula.
fn paint_sky(
    screen: &mut Screen,
    palette: &Palette,
    daylight: f32,
    flash: f32,
    t: f32,
    horizon: f32,
) {
    let width = screen.width();
    let rows = horizon.max(0.0).floor() as i32;
    screen.backdrop_in(0, 0, width, rows, palette, |cell| {
        sky_value(cell, daylight, flash, t)
    });
}

/// Paints the sea below the surface, from a little above the still water to
/// the bottom of the screen. Cells above the surface are left as the sky
/// painted them, so the sea never covers the sky.
fn paint_sea(screen: &mut Screen, palette: &Palette, waves: &Waves, t: f32, rows: f32, flash: f32) {
    let (width, height) = screen.size();
    let water = level(rows);
    let top = (water - 4.0).max(0.0).floor() as i32;
    for y in top..height as i32 {
        for x in 0..width as i32 {
            let centre = y as f32 + 0.5;
            if centre < waves.surface_y(x as f32, t, water) {
                continue;
            }
            let value = sea_value(x as f32, centre, waves, t, rows, flash);
            let (block, fg, bg) = palette.shade_signed(value);
            screen.put_on(x, y, block, fg, bg);
        }
    }
}

/// What the strip along the top shows.
struct Hud {
    score: u32,
    best: u32,
    lives: u32,
    time: &'static str,
}

/// The strip along the top: the score and the best score on the left, the time
/// of day in the middle and one heart per life on the right (red while it
/// lasts, grey once lost).
fn draw_hud(screen: &mut Screen, hud: &Hud) {
    let width = screen.width();
    screen.fill(0, 0, width, 1, ' ', Colour::White, Colour::Black);
    screen.text(1, 0, "score ", Colour::BrightYellow);
    number(screen, 7, 0, hud.score, Colour::BrightYellow);
    screen.text(16, 0, "best ", Colour::White);
    number(screen, 21, 0, hud.best, Colour::White);
    screen.text_centred(0, hud.time, Colour::BrightCyan);
    let first = width - 2 * LIVES as i32;
    for life in 0..LIVES {
        let colour = if life < hud.lives {
            Colour::BrightRed
        } else {
            Colour::BrightBlack
        };
        screen.put_on(first + 2 * life as i32, 0, '♥', colour, Colour::Black);
    }
}

/// Draws `n` in decimal starting at `(x, y)`, one cell a digit, without
/// allocating a string. Returns the column after the last digit.
fn number(screen: &mut Screen, x: i32, y: i32, n: u32, colour: Colour) -> i32 {
    // least significant digit first; a u32 has at most ten digits
    let mut digits = [0u8; 10];
    let mut count = 0;
    let mut rest = n;
    loop {
        digits[count] = b'0' + (rest % 10) as u8;
        count += 1;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    for (column, digit) in digits[..count].iter().rev().enumerate() {
        screen.put(x + column as i32, y, *digit as char, colour);
    }
    x + count as i32
}

/// The letters of the name, in block capitals, five rows high.
const LETTER_S: &str = r"
#####
#
#####
    #
#####
";
const LETTER_A: &str = r"
 ###
#   #
#####
#   #
#   #
";
const LETTER_I: &str = r"
#####
  #
  #
  #
#####
";
const LETTER_L: &str = r"
#
#
#
#
#####
";

/// The first screen: the name in block letters, the sea and the sky, and a
/// boat sailing on its own.
struct Title {
    best: u32,
    look: Look,
    letters: [Sprite; 4],
}

impl Title {
    fn new(best: u32) -> Title {
        Title {
            best,
            look: Look::new(),
            letters: [
                Sprite::new(LETTER_S, Colour::BrightRed),
                Sprite::new(LETTER_A, Colour::BrightYellow),
                Sprite::new(LETTER_I, Colour::BrightGreen),
                Sprite::new(LETTER_L, Colour::BrightCyan),
            ],
        }
    }
}

/// Columns from one block letter to the next: five wide and one space.
const LETTER_STEP: i32 = 6;

impl Game for Title {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed, self.best))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        // the title is not paused, so the screen clock is its time; the sky
        // drifts slowly from dawn toward day
        let t = screen.time();
        let rows = screen.height() as f32;
        let waves = sea(t);
        paint_sky(
            screen,
            &self.look.sky,
            daylight(t * 0.25),
            0.0,
            t,
            level(rows) + 2.0,
        );
        paint_sea(screen, &self.look.sea, &waves, t, rows, 0.0);
        let x = screen.width() as f32 / 3.0;
        let hull = waves.surface_y(x + BOAT_WIDTH / 2.0, t, level(rows));
        draw_boat(
            screen,
            &self.look,
            x,
            hull,
            waves.tilt(x + BOAT_WIDTH / 2.0, t),
        );
        let word = LETTER_STEP * self.letters.len() as i32 - 1;
        let left = (screen.width() - word) / 2;
        // a dark band behind the letters, as glow has behind its word, so they
        // read over the busy sky
        screen.fill(left - 2, 1, word + 4, 7, ' ', Colour::White, Colour::Black);
        for (i, letter) in self.letters.iter().enumerate() {
            screen.sprite(left + LETTER_STEP * i as i32, 2, letter);
        }
        strip(screen, 0, "best ", self.best);
        plate(screen, 9, "press Enter to sail", Colour::BrightWhite);
        plate(
            screen,
            10,
            "A or D steer   Space hop   p pause   q quit",
            Colour::White,
        );
    }
}

/// A line of text centred on a black strip, so it reads over the sky.
fn plate(screen: &mut Screen, row: i32, text: &str, colour: Colour) {
    let width = screen.width();
    screen.fill(0, row, width, 1, ' ', colour, Colour::Black);
    screen.text_centred(row, text, colour);
}

/// A black strip across row `row` that says `label` and a number.
fn strip(screen: &mut Screen, row: i32, label: &str, n: u32) {
    let width = screen.width();
    screen.fill(0, row, width, 1, ' ', Colour::White, Colour::Black);
    screen.text(1, row, label, Colour::White);
    number(
        screen,
        1 + label.chars().count() as i32,
        row,
        n,
        Colour::White,
    );
}

/// Drawn over the game, which stays as it was while paused.
struct Paused;

impl Game for Paused {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Char('p')) || ctx.keys.pressed(KeyCode::Esc) {
            Next::pop()
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        let row = screen.height() / 2 - 2;
        plate(screen, row, "PAUSED  (p to resume)", Colour::BrightWhite);
    }
}

/// The game-over screen, shown when the last life is lost. The sky stays at the
/// time of day the round ended, and the sea keeps moving.
struct Over {
    score: u32,
    best: u32,
    at: f32,
    look: Look,
}

impl Over {
    fn new(score: u32, best: u32, at: f32) -> Over {
        Over {
            score,
            best,
            at,
            look: Look::new(),
        }
    }
}

impl Game for Over {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            let seed = ctx.seed.wrapping_add(ctx.tick);
            Next::switch(Play::new(seed, self.best))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        // the sky keeps its time of day from the round; the sea and stars move
        let t = screen.time();
        let rows = screen.height() as f32;
        let light = daylight(self.at);
        let waves = sea(t);
        paint_sky(screen, &self.look.sky, light, 0.0, t, level(rows) + 2.0);
        paint_stars(screen, night(light), t, level(rows) - 5.0);
        paint_sea(screen, &self.look.sea, &waves, t, rows, 0.0);
        let middle = screen.height() / 2;
        plate(
            screen,
            middle - 3,
            "the sea wins this time",
            Colour::BrightRed,
        );
        centred_number(
            screen,
            middle - 1,
            "score ",
            self.score,
            Colour::BrightYellow,
        );
        centred_number(screen, middle, "best ", self.best, Colour::White);
        plate(
            screen,
            middle + 2,
            "Enter: sail again   q: quit",
            Colour::White,
        );
    }
}

/// A line of `label` and then `n`, centred on the screen.
fn centred_number(screen: &mut Screen, row: i32, label: &str, n: u32, colour: Colour) {
    let width = screen.width();
    let total = label.chars().count() as i32 + digit_count(n);
    let left = (width - total) / 2;
    screen.fill(0, row, width, 1, ' ', colour, Colour::Black);
    screen.text(left, row, label, colour);
    number(screen, left + label.chars().count() as i32, row, n, colour);
}

/// How many decimal digits `n` has.
fn digit_count(n: u32) -> i32 {
    let mut digits = 1;
    let mut rest = n;
    while rest >= 10 {
        rest /= 10;
        digits += 1;
    }
    digits
}

/// The game itself.
struct Play {
    best: u32,
    look: Look,
    elapsed: f32,
    boat: Boat,
    rng: Rng,
    controls: Controls<Action>,
    spray: Particles,
    fireballs: Pool<Fireball>,
    lives: u32,
    embers: Particles,
    /// Play time until which the boat cannot be hit. Negative infinity means never hit.
    invulnerable_until: f32,
    score: u32,
    floaters: Pool<Floater>,
    tenth: Timer,
    steam: Particles,
    spawn: Timer,
    /// Play time until which the screen flashes after a hit.
    flash_until: f32,
    wake: Timer,
}

/// A few spray droplets a second trail from the stern while the boat sails,
/// and they fall back to the water.
const WAKE_SECONDS: f32 = 0.15;
const SPRAY_FALL: f32 = 12.0;

impl Play {
    fn new(seed: u64, best: u32) -> Play {
        Play {
            best,
            look: Look::new(),
            elapsed: 0.0,
            boat: Boat::new(),
            rng: Rng::new(seed),
            controls: controls(),
            spray: Particles::new(SPRAY_LIMIT)
                .gravity(SPRAY_FALL)
                .drag(0.2)
                .palette(Palette::new(&[
                    Colour::Blue,
                    Colour::Cyan,
                    Colour::BrightCyan,
                    Colour::BrightWhite,
                ])),
            wake: Timer::every(WAKE_SECONDS),
            fireballs: Pool::with_capacity(FIREBALL_LIMIT),
            lives: LIVES,
            embers: Particles::new(EMBER_LIMIT).gravity(EMBER_RISE).drag(0.5),
            // finished from the start, so the boat begins vulnerable
            invulnerable_until: f32::NEG_INFINITY,
            score: 0,
            floaters: Pool::with_capacity(FLOATER_LIMIT),
            tenth: Timer::every(0.1),
            steam: Particles::new(STEAM_LIMIT),
            spawn: Timer::every(FIRST_SPAWN),
            flash_until: f32::NEG_INFINITY,
        }
    }

    /// Sends a fireball in from just past the right edge. Most skim the water;
    /// one in three is lobbed from high, and arcs down.
    fn spawn_fireball(&mut self, width: f32, rows: f32, waves: &Waves) {
        let x = width + 2.0;
        let speed = -self.rng.range_f32(22.0, 34.0);
        let surface = waves.surface_y(x, self.elapsed, level(rows));
        let f = if self.rng.chance(0.3) {
            let y = level(rows) - self.rng.range_f32(8.0, 12.0);
            Fireball {
                vy: -self.rng.range_f32(4.0, 8.0),
                gravity: FIREBALL_GRAVITY,
                ..fireball(x, y, speed, FIREBALL_GRAVITY)
            }
        } else {
            let y = surface - self.rng.range_f32(0.0, 5.0);
            fireball(x, y, speed, 0.0)
        };
        let _ = self.fireballs.push(f);
    }

    /// Whether the screen is flashing after a hit.
    fn flashing(&self) -> bool {
        self.elapsed < self.flash_until
    }

    /// Moves every fireball, then strikes the boat with the first one that
    /// touches its hull, and removes the fireballs that struck.
    fn fly_fireballs(&mut self, dt: f32, hull: Hit, shore: impl Fn(f32) -> f32) {
        // only the first touch counts: after a hit the boat is invulnerable
        let vulnerable = self.elapsed >= self.invulnerable_until;
        let mut struck = false;
        let (top, bottom) = self.boat.body_rows();
        let centre = self.boat.centre();
        for f in self.fireballs.iter_mut() {
            f.vy += f.gravity * dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.fate = if vulnerable && !struck && hull.overlaps(&f.shape()) {
                struck = true;
                Fate::Hit
            } else if f.y >= shore(f.x) {
                // it reached the water: a cloud of steam, and it is gone
                self.steam
                    .burst(&mut self.rng, f.x, f.y, STEAM_BURST, 6.0, 1.2);
                Fate::Splashed
            } else if f.x < OFF_LEFT {
                Fate::Gone
            } else {
                Fate::Flying
            };
            // a fireball that crosses the boat's column and misses by at most
            // NEAR_ROWS scores once, and a +5 floats up from the boat
            if !f.passed && f.x <= centre {
                f.passed = true;
                let gap = if f.y < top {
                    top - f.y
                } else if f.y > bottom {
                    f.y - bottom
                } else {
                    0.0
                };
                if f.fate == Fate::Flying && gap <= NEAR_ROWS {
                    self.score += NEAR_BONUS;
                    let _ = self.floaters.push(Floater {
                        x: centre,
                        y: top,
                        age: 0.0,
                    });
                }
            }
        }
        if struck {
            self.lives = self.lives.saturating_sub(1);
            let (x, y) = (self.boat.centre(), self.boat.y - 1.5);
            self.embers.burst(&mut self.rng, x, y, 40, 14.0, 1.0);
            self.invulnerable_until = self.elapsed + INVULNERABLE_SECONDS;
            self.flash_until = self.elapsed + FLASH_SECONDS;
        }
        self.fireballs.retain(|f| f.fate == Fate::Flying);
    }
}

/// The most spray droplets at once, and the most embers.
const SPRAY_LIMIT: usize = 200;
const EMBER_LIMIT: usize = 400;
/// Embers a flying fireball leaves each tick, how long each lives in seconds,
/// and how fast they rise (negative gravity: rows a second each second).
const EMBERS_PER_TICK: usize = 2;
const EMBER_LIFE: f32 = 0.5;
const EMBER_RISE: f32 = -6.0;
const STEAM_LIMIT: usize = 160;
/// Steam droplets in the cloud when a fireball reaches the water.
const STEAM_BURST: usize = 14;

/// A fireball at `x` and `y` that moves `vx` columns a second and falls at `gravity`.
fn fireball(x: f32, y: f32, vx: f32, gravity: f32) -> Fireball {
    Fireball {
        x,
        y,
        vx,
        vy: 0.0,
        gravity,
        passed: false,
        fate: Fate::Flying,
    }
}

impl Game for Play {
    fn draw(&self, screen: &mut Screen) {
        // every value here comes from play time, so a paused game does not move
        let t = self.elapsed;
        let waves = sea(t);
        let light = daylight(t);
        let flash = if self.flashing() { 1.0 } else { 0.0 };
        let rows = screen.height() as f32;
        paint_sky(screen, &self.look.sky, light, flash, t, level(rows) + 2.0);
        paint_stars(screen, night(light), t, level(rows) - 5.0);
        paint_sea(screen, &self.look.sea, &waves, t, rows, flash);
        self.steam.draw(screen);
        self.embers.draw(screen);
        self.spray.draw(screen);
        for f in self.fireballs.iter() {
            screen.put(
                f.x.round() as i32,
                f.y.round() as i32,
                '@',
                Colour::BrightYellow,
            );
        }
        for floater in self.floaters.iter() {
            let colour = if floater.age < 0.4 {
                Colour::BrightYellow
            } else {
                Colour::Yellow
            };
            let x = floater.x.round() as i32 - 1;
            screen.text(x, floater.y.round() as i32, "+5", colour);
        }
        // an invulnerable boat blinks, hidden on alternate slots of 0.08 seconds
        let blink_slot = (t / BLINK_SECONDS).floor() as i64;
        let hidden = self.elapsed < self.invulnerable_until && blink_slot % 2 == 1;
        if self.boat.y.is_finite() && !hidden {
            let tilt = waves.tilt(self.boat.centre(), t);
            draw_boat(screen, &self.look, self.boat.x, self.boat.y, tilt);
        }
        let hud = Hud {
            score: self.score,
            best: self.best,
            lives: self.lives,
            time: time_of_day(t),
        };
        draw_hud(screen, &hud);
    }

    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        // checked before anything moves, so a paused round stands still
        if self.controls.pressed(ctx.keys, Action::Pause) {
            return Next::push(Paused);
        }
        if ctx.keys.pressed(KeyCode::Char('q')) {
            return Next::quit();
        }
        self.elapsed += ctx.dt;
        let (width, rows) = (ctx.size.0 as f32, ctx.size.1 as f32);
        let waves = sea(self.elapsed);
        let water = |x: f32| waves.surface_y(x, self.elapsed, level(rows));
        self.boat.place(width, water);
        let across = self.controls.axis(ctx.keys, Action::Left, Action::Right) as f32;
        self.boat.steer(across * SAIL_SPEED, ctx.dt, 0.6 * width);
        if self.controls.pressed(ctx.keys, Action::Hop) {
            self.boat.hop();
        }
        let surface = waves.surface_y(self.boat.centre(), self.elapsed, level(rows));
        if self.boat.fly(ctx.dt) {
            // landing sprays water up from the hull
            let (x, y) = (self.boat.centre(), surface);
            self.spray.burst(&mut self.rng, x, y, 12, 8.0, 0.6);
        }
        self.boat.settle(surface - self.boat.air, ctx.dt);
        if self.boat.air == 0.0 && self.boat.vx.abs() > 1.0 && self.wake.tick(ctx.dt) {
            // a droplet from the stern, drifting back and up a little
            let stern = self.boat.x;
            self.spray.emit(stern, surface, -2.0, -1.5, 0.5);
        }
        self.spray.step(ctx.dt);
        self.score += self.tenth.ticks(ctx.dt);
        // the interval shrinks as the game goes on, so the timer's period is reset each step
        self.spawn.set_seconds(spawn_interval(self.elapsed));
        for _ in 0..self.spawn.ticks(ctx.dt) {
            self.spawn_fireball(width, rows, &waves);
        }
        let hull = self.boat.hull_hit();
        let (elapsed, shore_waves) = (self.elapsed, waves);
        let shore = move |x: f32| shore_waves.surface_y(x, elapsed, level(rows));
        self.fly_fireballs(ctx.dt, hull, shore);
        // every fireball that is still flying leaves embers behind it
        for f in self.fireballs.iter() {
            for _ in 0..EMBERS_PER_TICK {
                let vx = self.rng.range_f32(0.5, 2.0);
                let vy = self.rng.range_f32(-1.5, 0.0);
                self.embers.emit(f.x + 1.0, f.y, vx, vy, EMBER_LIFE);
            }
        }
        self.embers.step(ctx.dt);
        self.steam.step(ctx.dt);
        for floater in self.floaters.iter_mut() {
            floater.y -= FLOATER_RISE * ctx.dt;
            floater.age += ctx.dt;
        }
        self.floaters.retain(|floater| floater.age < FLOATER_LIFE);
        if self.lives == 0 {
            let best = self.best.max(self.score);
            return Next::switch(Over::new(self.score, best, self.elapsed));
        }
        Next::stay()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Drives a [`Play`] one update at a time, with its own keys and clock.
    struct Rig {
        keys: Keys,
        pointer: Pointer,
        tick: u64,
        size: (u32, u32),
    }

    impl Rig {
        fn new(size: (u32, u32)) -> Rig {
            Rig {
                keys: Keys::new().real_release(true),
                pointer: Pointer::default(),
                tick: 0,
                size,
            }
        }

        /// The time of the next update, as the keys see it.
        fn now(&self) -> Duration {
            Duration::from_secs_f64(self.tick as f64 / 60.0)
        }

        fn press(&mut self, code: KeyCode) {
            let now = self.now();
            self.keys.event(&Event::Key(Key::new(code)), now);
        }

        fn release(&mut self, code: KeyCode) {
            let now = self.now();
            self.keys.event(&Event::KeyUp(Key::new(code)), now);
        }

        /// Runs one update of `play` and returns what it asked for.
        fn update(&mut self, play: &mut Play) -> Next {
            let now = self.now();
            self.keys.tick(now);
            let mut ctx = Context {
                keys: &self.keys,
                pointer: &self.pointer,
                dt: 1.0 / 60.0,
                tick: self.tick,
                size: self.size,
                seed: 7,
            };
            let next = play.update(&mut ctx);
            self.keys.end_tick();
            self.tick += 1;
            next
        }

        fn run(&mut self, play: &mut Play, ticks: u32) {
            for _ in 0..ticks {
                self.update(play);
            }
        }
    }

    /// The row of the water under the boat's middle, at the play's time.
    fn surface_under(play: &Play, rows: f32) -> f32 {
        sea(play.elapsed).surface_y(play.boat.centre(), play.elapsed, level(rows))
    }

    #[test]
    fn a_hop_leaves_the_water_peaks_and_lands_with_a_spray() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120); // settle on the water first
        rig.press(KeyCode::Char(' '));
        let mut peak = 0.0_f32;
        let mut landed = false;
        for _ in 0..120 {
            rig.update(&mut play);
            peak = peak.max(play.boat.air);
            if peak > 0.0 && play.boat.air == 0.0 {
                landed = true;
                break;
            }
        }
        assert!(peak > 4.0 && peak < 6.0, "peaks about 5 rows up: {peak}");
        assert!(landed, "and comes back down");
        assert!(play.spray.len() >= 6, "with a spray: {}", play.spray.len());
    }

    #[test]
    fn a_fireball_skimming_at_the_boats_height_hits_it_and_costs_a_life() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1); // places the boat
        let boat = play.boat;
        let at = fireball(boat.centre(), boat.y - 1.0, -25.0, 0.0);
        assert!(play.fireballs.push(at).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 1, "one life lost");
        assert!(play.fireballs.is_empty(), "the fireball that hit is gone");
    }

    #[test]
    fn after_a_hit_a_second_hit_within_1_5_seconds_does_nothing_but_a_later_one_counts() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let hit = |y: f32| fireball(boat.centre(), y, -25.0, 0.0);
        assert!(play.fireballs.push(hit(boat.y - 1.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 1);

        assert!(play.fireballs.push(hit(boat.y - 1.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(
            play.lives,
            LIVES - 1,
            "invulnerable: the second hit does nothing"
        );

        rig.run(&mut play, 92); // a little over 1.5 seconds after the first hit
        play.fireballs.clear(); // no random ones from the sea's own timer
        assert!(play.fireballs.push(hit(boat.y - 1.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 2, "vulnerable again");
    }

    #[test]
    fn the_third_hit_ends_the_round_and_the_first_two_do_not() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let mut outcomes = Vec::new();
        for _ in 0..3 {
            assert!(play
                .fireballs
                .push(fireball(boat.centre(), boat.y - 1.0, -25.0, 0.0))
                .is_ok());
            outcomes.push(rig.update(&mut play));
            rig.run(&mut play, 92); // a little over 1.5 seconds: out of the invulnerable time
            play.fireballs.clear(); // no random ones from the sea's own timer
        }
        assert!(matches!(outcomes[0], Next::Stay), "the first hit");
        assert!(matches!(outcomes[1], Next::Stay), "the second hit");
        assert!(
            matches!(outcomes[2], Next::Switch(_)),
            "the third ends the round"
        );
        assert_eq!(play.lives, 0);
    }

    #[test]
    fn a_fireball_that_passes_close_above_the_boat_scores_five_once() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        // 2.5 rows above the top of the boat: not a hit, but within 3 rows
        let (top, _) = boat.body_rows();
        let close = fireball(boat.centre(), top - 2.5, -25.0, 0.0);
        assert!(play.fireballs.push(close).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES, "a miss is not a hit");
        assert_eq!(play.score, 5, "the bonus for passing close");
        assert_eq!(play.floaters.len(), 1, "and a +5 floats up");

        // the fireball keeps flying for a while. Five ticks in all is under a
        // tenth of a second, so any extra score here would be a second bonus
        // (six ticks would be exactly a tenth, and a time point would land)
        rig.run(&mut play, 3);
        assert!(play.fireballs.len() == 1, "still flying");
        assert_eq!(play.score, 5, "no second bonus");
    }

    #[test]
    fn the_score_rises_by_one_for_every_tenth_of_a_second_survived() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120); // two seconds, no fireballs yet
        assert!(
            (19..=20).contains(&play.score),
            "two seconds: {}",
            play.score
        );
    }

    #[test]
    fn fireballs_come_in_from_the_right_edge_on_the_spawn_interval() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        let mut first = None;
        for tick in 0..200 {
            rig.update(&mut play);
            if first.is_none() {
                if let Some(f) = play.fireballs.get(0) {
                    first = Some((tick, f.x));
                }
            }
        }
        let (tick, x) = first.expect("a fireball came in");
        assert!(
            (90..=100).contains(&tick),
            "about 1.6 seconds in: tick {tick}"
        );
        assert!(
            (79.0..83.0).contains(&x),
            "from just past the right edge: {x}"
        );
    }

    #[test]
    fn a_hit_flashes_the_screen_for_0_15_seconds() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        assert!(play
            .fireballs
            .push(fireball(boat.centre(), boat.y - 1.0, -25.0, 0.0))
            .is_ok());
        rig.update(&mut play);
        assert!(play.flashing(), "the hit flashes");
        rig.run(&mut play, 5); // 0.09 seconds on
        assert!(play.flashing(), "still flashing");
        rig.run(&mut play, 6); // 0.19 seconds on
        assert!(!play.flashing(), "the flash is over");
    }

    #[test]
    fn p_pauses_by_pushing_the_pause_screen_before_time_moves() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 30);
        let before = play.elapsed;
        rig.press(KeyCode::Char('p'));
        let next = rig.update(&mut play);
        assert!(matches!(next, Next::Push(_)), "p pushes the pause screen");
        assert_eq!(
            play.elapsed, before,
            "and the pausing update does not move time"
        );
    }

    #[test]
    fn the_pools_never_grow_past_their_limits_over_6000_ticks_of_play() {
        let mut play = Play::new(7, 0);
        let mut rig = Rig::new((80, 24));
        let mut script = Rng::new(3);
        let moves = [KeyCode::Left, KeyCode::Right, KeyCode::Char(' ')];
        let (mut most_fireballs, mut most_embers, mut hits) = (0, 0, 0);
        let mut lives = LIVES;
        for tick in 0..6000 {
            if tick % 15 == 0 {
                // a new random key every quarter second, so the boat wanders and hops
                for code in moves {
                    rig.release(code);
                }
                let code = moves[script.range_i32(0, 3) as usize];
                rig.press(code);
            }
            rig.update(&mut play);
            assert!(
                play.fireballs.len() <= FIREBALL_LIMIT,
                "fireballs at {tick}"
            );
            assert!(play.floaters.len() <= FLOATER_LIMIT, "floaters at {tick}");
            assert!(play.embers.len() <= EMBER_LIMIT, "embers at {tick}");
            assert!(play.steam.len() <= STEAM_LIMIT, "steam at {tick}");
            assert!(play.spray.len() <= SPRAY_LIMIT, "spray at {tick}");
            most_fireballs = most_fireballs.max(play.fireballs.len());
            most_embers = most_embers.max(play.embers.len());
            if play.lives < lives {
                hits += 1;
                lives = play.lives;
            }
        }
        assert!(most_fireballs > 0, "fireballs came in");
        assert!(most_embers > 0, "and left embers");
        assert!(
            play.score > 0,
            "the script played for a while: {} hits",
            hits
        );
    }

    #[test]
    fn sailing_trails_a_thin_spray_from_the_stern() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        rig.press(KeyCode::Right);
        rig.run(&mut play, 60); // a second under way
        assert!(play.boat.vx > 1.0, "under way: {}", play.boat.vx);
        assert!(play.spray.len() >= 2, "a wake: {}", play.spray.len());
    }

    /// A sketch cell `y` rows down a sky `height` rows tall, at column `x`.
    fn sky_cell(x: f32, y_rows: f32, height_rows: f32) -> Cell {
        Cell {
            x,
            y: y_rows * 2.0,
            t: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            width: 80.0,
            height: height_rows * 2.0,
        }
    }

    #[test]
    fn the_sky_is_dark_at_midnight_and_bright_at_midday() {
        let cell = sky_cell(10.0, 2.0, 10.0);
        let midnight = sky_value(&cell, 0.0, 0.0, 0.0);
        let midday = sky_value(&cell, 1.0, 0.0, 0.0);
        assert!(midnight < -0.5, "midnight is dark: {midnight}");
        assert!(midday > 0.5, "midday is bright: {midday}");
    }

    #[test]
    fn the_sea_is_brighter_at_the_surface_than_in_the_deep() {
        let waves = sea(0.0);
        let (x, t, rows) = (10.0, 0.0, 24.0);
        let surface = waves.surface_y(x, t, level(rows));
        let near = sea_value(x, surface + 0.5, &waves, t, rows, 0.0);
        let deep = sea_value(x, surface + 6.0, &waves, t, rows, 0.0);
        assert!(near > deep + 0.3, "surface {near}, deep {deep}");
    }

    /// How many cells of `screen` show one of `chars`.
    fn count_chars(screen: &Screen, chars: &[char]) -> usize {
        let (width, height) = screen.size();
        let display = screen.display();
        let mut count = 0;
        for y in 0..height as isize {
            for x in 0..width as isize {
                if let Some(pixel) = display.pixel(x, y) {
                    if chars.contains(&pixel.character()) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    #[test]
    fn stars_show_in_the_night_sky_and_not_by_day() {
        let mut day = Screen::new(80, 24);
        paint_stars(&mut day, 0.0, 0.0, 15.0);
        assert_eq!(count_chars(&day, &['*', '.']), 0, "no stars at midday");
        let mut night = Screen::new(80, 24);
        paint_stars(&mut night, 1.0, 0.0, 15.0);
        let stars = count_chars(&night, &['*', '.']);
        assert!((5..200).contains(&stars), "a sparse sky of stars: {stars}");
    }

    #[test]
    fn the_boat_is_drawn_with_its_hull_on_the_water() {
        let mut game = Headless::new(Play::new(1, 0)).size(80, 24);
        game.step(60);
        let text = game.text();
        let hull = text
            .lines()
            .position(|line| line.contains("\\_____/"))
            .unwrap_or_else(|| panic!("no hull drawn:\n{text}"));
        let water = level(24.0);
        assert!(
            (hull as f32 - water).abs() <= 4.0,
            "the hull is on row {hull}, the water is near row {water}:\n{text}"
        );
    }

    #[test]
    fn the_hud_shows_the_score_the_best_score_and_the_lives_as_hearts() {
        let mut game = Headless::new(Play::new(1, 7)).size(80, 24);
        game.step(2);
        let text = game.text();
        let top = text.lines().next().unwrap_or("");
        assert!(top.contains("score 0"), "{top}");
        assert!(top.contains("best 7"), "{top}");
        assert!(top.contains("♥ ♥ ♥"), "{top}");
        assert!(top.contains("dawn"), "the time of day, centred: {top}");
    }

    #[test]
    fn the_sky_and_the_sea_fill_the_screen_below_the_hud() {
        let mut screen = Screen::new(80, 24);
        screen.clear(Colour::Black); // so that only painting can make a colour
        Play::new(1, 0).draw(&mut screen);
        let coloured = (1..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                screen
                    .display()
                    .pixel(x, y)
                    .is_some_and(|pixel| pixel.background() != Colour::Black)
            })
            .count();
        assert!(coloured >= 1000, "{coloured} coloured cells of 1840");
    }

    #[test]
    fn a_fireball_is_a_bright_at_sign_and_a_bonus_floats_as_plus_five() {
        let mut play = Play::new(1, 0);
        let mut screen = Screen::new(80, 24);
        assert!(play.fireballs.push(fireball(30.0, 5.0, -25.0, 0.0)).is_ok());
        assert!(play
            .floaters
            .push(Floater {
                x: 50.0,
                y: 9.0,
                age: 0.0
            })
            .is_ok());
        play.draw(&mut screen);
        assert_eq!(
            screen.display().pixel(30, 5).unwrap().character(),
            '@',
            "the fireball"
        );
        let row: String = (0..80)
            .map(|x| screen.display().pixel(x, 9).unwrap().character())
            .collect();
        assert!(row.contains("+5"), "the bonus: {row:?}");
    }

    #[test]
    fn p_or_esc_resumes_from_the_pause_screen_and_q_quits_from_it() {
        let mut game = Headless::new(Play::new(1, 0)).size(80, 24);
        game.tap(KeyCode::Char('p'));
        assert!(game.text().contains("PAUSED"));
        game.tap(KeyCode::Char('p'));
        assert!(!game.text().contains("PAUSED"), "p resumes");
        game.tap(KeyCode::Esc);
        assert!(game.text().contains("PAUSED"), "Esc pauses");
        game.tap(KeyCode::Esc);
        assert!(!game.text().contains("PAUSED"), "Esc resumes");
        game.tap(KeyCode::Char('p'));
        game.tap(KeyCode::Char('q'));
        assert!(game.quit(), "q quits from the pause screen");
    }

    #[test]
    fn enter_on_the_game_over_screen_sails_again_and_keeps_the_best() {
        let mut game = Headless::new(Over::new(3, 9, 0.0)).size(80, 24);
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "a fresh round: {text}");
        assert!(text.contains("best 9"), "the best is kept: {text}");
    }

    #[test]
    fn q_quits_from_the_game_over_screen() {
        let mut game = Headless::new(Over::new(3, 9, 0.0));
        game.tap(KeyCode::Char('q'));
        assert!(game.quit());
    }

    #[test]
    fn enter_on_the_title_starts_a_round_with_no_score() {
        let mut game = Headless::new(Title::new(4)).size(80, 24);
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "a round is on screen: {text}");
        assert!(text.contains("best 4"), "with the best score kept: {text}");
    }

    /// Plays 1500 ticks of scripted keys from the title and returns the screen.
    fn scripted_round(seed: u64) -> String {
        let mut game = Headless::new(Title::new(0)).seed(seed).size(80, 24);
        game.tap(KeyCode::Enter);
        for tick in 0..1500 {
            match tick % 90 {
                0 => game.press(KeyCode::Right),
                45 => game.release(KeyCode::Right),
                _ => {}
            }
            if tick % 200 == 100 {
                game.tap(KeyCode::Char(' '));
            }
            game.step(1);
        }
        game.text()
    }

    #[test]
    fn the_same_seed_and_the_same_keys_give_the_same_screen_after_1500_ticks() {
        assert_eq!(scripted_round(7), scripted_round(7));
        assert_ne!(
            scripted_round(7),
            scripted_round(8),
            "another seed is another round"
        );
    }

    #[test]
    fn the_sky_and_sea_do_not_move_while_the_round_is_paused() {
        let mut game = Headless::new(Play::new(1, 0)).size(80, 24);
        game.step(300);
        game.tap(KeyCode::Char('p'));
        let frozen = game.text();
        game.step(300);
        assert_eq!(game.text(), frozen, "five seconds of pause changed nothing");
    }

    #[test]
    fn the_title_letters_sit_on_a_dark_band_so_they_read_over_the_sky() {
        let mut screen = Screen::new(80, 24);
        Title::new(0).draw(&mut screen);
        let left = (80 - 23) / 2; // where the S starts
        let behind = screen.display().pixel(left - 1, 4).unwrap().background();
        assert_eq!(behind, Colour::Black, "just left of the S");
    }

    #[test]
    fn q_quits_from_the_title() {
        let mut game = Headless::new(Title::new(0));
        game.tap(KeyCode::Char('q'));
        assert!(game.quit());
    }

    #[test]
    fn the_title_shows_the_name_in_block_letters_and_how_to_start() {
        let mut game = Headless::new(Title::new(0)).size(80, 24);
        game.step(3);
        let text = game.text();
        assert!(text.contains("press Enter to sail"), "{text}");
        let lettered = text
            .lines()
            .skip(2)
            .take(5)
            .filter(|line| line.contains("#####"))
            .count();
        assert!(lettered >= 3, "block letters on about five rows:\n{text}");
    }

    #[test]
    fn the_game_over_screen_shows_the_score_the_best_and_how_to_go_on() {
        let mut game = Headless::new(Over::new(3, 9, 0.0)).size(80, 24);
        game.step(2);
        let text = game.text();
        assert!(text.contains("score 3"), "{text}");
        assert!(text.contains("best 9"), "{text}");
        assert!(text.contains("Enter: sail again"), "{text}");
    }

    #[test]
    fn the_pause_screen_is_drawn_over_the_round() {
        let mut game = Headless::new(Play::new(1, 0)).size(80, 24);
        game.step(60);
        game.tap(KeyCode::Char('p'));
        let text = game.text();
        assert!(text.contains("PAUSED  (p to resume)"), "{text}");
        assert!(
            text.contains("score"),
            "the round is still behind it: {text}"
        );
    }

    #[test]
    fn a_lost_life_is_a_grey_heart_and_a_kept_life_is_red() {
        let mut play = Play::new(1, 0);
        play.lives = 1;
        let mut screen = Screen::new(80, 24);
        play.draw(&mut screen);
        let first = screen.display().pixel(74, 0).unwrap();
        let lost = screen.display().pixel(78, 0).unwrap();
        assert_eq!(first.colour(), Colour::BrightRed, "the one life left");
        assert_eq!(lost.colour(), Colour::BrightBlack, "a lost life");
    }

    #[test]
    fn a_boat_that_is_invulnerable_blinks_on_and_off() {
        let hull_cells = |elapsed: f32| {
            let mut play = Play::new(1, 0);
            play.boat.x = 20.0;
            play.boat.y = 15.0;
            play.invulnerable_until = INVULNERABLE_SECONDS;
            play.elapsed = elapsed;
            let mut screen = Screen::new(80, 24);
            play.draw(&mut screen);
            count_chars(&screen, &['_'])
        };
        assert!(hull_cells(0.01) >= 5, "shown in the first slot");
        assert_eq!(hull_cells(0.10), 0, "hidden in the next 0.08 s slot");
        assert!(hull_cells(0.17) >= 5, "and shown again");
    }

    #[test]
    fn the_time_of_day_reads_dawn_day_dusk_and_night() {
        assert_eq!(time_of_day(0.0), "dawn");
        assert_eq!(time_of_day(30.0), "day");
        assert_eq!(time_of_day(60.0), "dusk");
        assert_eq!(time_of_day(90.0), "night");
    }

    #[test]
    fn the_sky_is_brighter_when_the_flash_is_on() {
        let cell = sky_cell(10.0, 2.0, 10.0);
        let dim = sky_value(&cell, 0.5, 0.0, 0.0);
        let flashing = sky_value(&cell, 0.5, 1.0, 0.0);
        assert!(flashing > dim, "{dim} then {flashing}");
    }

    #[test]
    fn q_quits_the_round() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 3);
        rig.press(KeyCode::Char('q'));
        assert!(matches!(rig.update(&mut play), Next::Quit), "q quits");
    }

    #[test]
    fn a_floater_rises_and_is_gone_after_0_8_seconds() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        assert!(play
            .floaters
            .push(Floater {
                x: 10.0,
                y: 10.0,
                age: 0.0
            })
            .is_ok());
        rig.run(&mut play, 24); // 0.4 seconds
        let risen = play.floaters.get(0).expect("still floating").y;
        assert!(risen < 10.0, "it rose: {risen}");
        rig.run(&mut play, 30); // 0.9 seconds
        assert!(play.floaters.is_empty(), "gone after 0.8 seconds");
    }

    #[test]
    fn a_flying_fireball_leaves_two_embers_behind_it_each_tick() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        assert!(play.fireballs.push(fireball(60.0, 5.0, -25.0, 0.0)).is_ok());
        rig.run(&mut play, 10);
        assert_eq!(play.fireballs.len(), 1, "still flying");
        assert_eq!(play.embers.len(), 20, "two a tick, and none has died yet");
    }

    #[test]
    fn a_fireball_that_leaves_the_left_edge_is_removed_without_steam() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        // high in the sky, so it neither hits the boat nor reaches the water
        // one tick moves it about 0.4 columns, so start it just inside the margin
        assert!(play.fireballs.push(fireball(-2.9, 2.0, -25.0, 0.0)).is_ok());
        rig.update(&mut play);
        assert!(play.fireballs.is_empty(), "it has left the screen");
        assert!(play.steam.is_empty(), "and it did not splash");
    }

    #[test]
    fn a_fireball_that_reaches_the_water_bursts_into_steam_and_is_removed() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let x = 60.0;
        let water = sea(play.elapsed).surface_y(x, play.elapsed, level(24.0));
        assert!(play
            .fireballs
            .push(fireball(x, water + 0.5, -25.0, 0.0))
            .is_ok());
        rig.update(&mut play);
        assert!(play.fireballs.is_empty(), "it is gone");
        assert!(
            play.steam.len() >= 10,
            "in a cloud of steam: {}",
            play.steam.len()
        );
    }

    #[test]
    fn a_hit_bursts_into_forty_embers_at_the_boat() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        assert!(play
            .fireballs
            .push(fireball(boat.centre(), boat.y - 1.0, -25.0, 0.0))
            .is_ok());
        rig.update(&mut play);
        assert_eq!(play.embers.len(), 40, "the explosion");
    }

    #[test]
    fn a_fireball_passing_under_a_hopping_boat_does_not_hit_it() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120);
        rig.press(KeyCode::Char(' '));
        rig.update(&mut play);
        rig.release(KeyCode::Char(' '));
        rig.run(&mut play, 35); // near the top of the hop
        assert!(
            play.boat.air > 3.0,
            "well clear of the water: {}",
            play.boat.air
        );
        play.fireballs.clear(); // no random ones from the sea's own timer
        let surface = surface_under(&play, 24.0);
        let low = fireball(play.boat.centre(), surface - 1.0, -25.0, 0.0);
        assert!(play.fireballs.push(low).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES, "the boat is over it");
        assert_eq!(play.fireballs.len(), 1, "and the fireball flies on");
    }

    #[test]
    fn a_hop_pressed_in_mid_air_does_nothing() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120);
        rig.press(KeyCode::Char(' '));
        rig.update(&mut play);
        rig.release(KeyCode::Char(' '));
        rig.run(&mut play, 20); // well up in the air now
        assert!(play.boat.air > 0.0, "still airborne");
        let before = play.boat.air_v;
        rig.press(KeyCode::Char(' ')); // a new press, not a key repeat
        rig.update(&mut play);
        assert!(
            play.boat.air_v < before,
            "gravity only, no second kick: {before} then {}",
            play.boat.air_v
        );
    }

    #[test]
    fn holding_right_sails_the_boat_right_but_not_past_0_6_of_the_width() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1); // places the boat, a third of the way across
        let start = play.boat.x;
        rig.press(KeyCode::Right);
        rig.run(&mut play, 60);
        assert!(play.boat.x > start + 10.0, "{start} then {}", play.boat.x);
        rig.run(&mut play, 600);
        assert!(
            (play.boat.x - 48.0).abs() < 0.01,
            "stopped at 0.6 of 80 columns: {}",
            play.boat.x
        );
    }

    #[test]
    fn the_hull_follows_the_waves_within_a_row_once_settled() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        let mut worst = 0.0_f32;
        for tick in 0..600 {
            rig.update(&mut play);
            if tick >= 60 {
                assert!(play.boat.y.is_finite(), "not on the water at tick {tick}");
                let gap = (play.boat.y - surface_under(&play, 24.0)).abs();
                worst = worst.max(gap);
            }
        }
        assert!(worst <= 1.0, "the hull was {worst} rows off the water");
    }

    #[test]
    fn night_is_one_at_midnight_zero_at_midday_and_always_in_range() {
        assert_eq!(night(1.0), 0.0, "midday has no night");
        assert_eq!(night(0.0), 1.0, "midnight is full night");
        for i in 0..=100 {
            let amount = night(i as f32 / 100.0);
            assert!((0.0..=1.0).contains(&amount), "{i}: {amount}");
        }
    }

    #[test]
    fn the_sea_grows_from_calm_to_stormy_over_90_seconds_then_stays() {
        assert!((roughness(0.0) - 1.0).abs() < 1e-5, "calm at the start");
        assert!((roughness(45.0) - 1.6).abs() < 1e-5, "halfway");
        assert!((roughness(90.0) - 2.2).abs() < 1e-5, "stormy at 90 s");
        assert!(
            (roughness(400.0) - 2.2).abs() < 1e-5,
            "and no stormier after"
        );
        assert!((roughness(-5.0) - 1.0).abs() < 1e-5, "and never calmer");
    }

    #[test]
    fn the_sea_is_rougher_later_on_in_the_same_proportions() {
        let (x, t) = (5.0, 1.0);
        let calm = sea(0.0).height(x, t);
        let stormy = sea(90.0).height(x, t);
        assert!(calm.abs() > 0.05, "calm water still moves: {calm}");
        assert!(
            (stormy - 2.2 * calm).abs() < 1e-4,
            "at 90 s every swell is 2.2 times as high: {calm} then {stormy}"
        );
    }

    #[test]
    fn fireballs_come_faster_from_1_6_seconds_apart_to_0_45_and_never_slower() {
        assert!(
            (spawn_interval(0.0) - 1.6).abs() < 1e-5,
            "the first is slow"
        );
        assert!(
            (spawn_interval(90.0) - 0.45).abs() < 1e-5,
            "by 90 s they are fast"
        );
        assert!((spawn_interval(500.0) - 0.45).abs() < 1e-5, "and stay fast");
        let mut last = f32::MAX;
        for i in 0..=400 {
            let interval = spawn_interval(i as f32 * 0.25);
            assert!(interval <= last, "rose at {i}: {last} then {interval}");
            last = interval;
        }
    }

    #[test]
    fn a_day_is_120_seconds_from_dawn_through_noon_dusk_and_midnight() {
        assert!((daylight(0.0) - 0.5).abs() < 1e-5, "dawn");
        assert!((daylight(30.0) - 1.0).abs() < 1e-5, "midday");
        assert!((daylight(60.0) - 0.5).abs() < 1e-5, "dusk");
        assert!(daylight(90.0) < 1e-5, "midnight");
        assert!((daylight(120.0) - 0.5).abs() < 1e-5, "dawn again");
    }
}

//! SAIL: a sailboat on the open sea, under a sky that turns from dawn to day,
//! dusk and night. The boat never jumps: steer it left and right and it sails
//! on whatever the weather does. Fireballs fall out of the sky toward the
//! boat, and one that reaches the water fizzles out in a cloud of steam. The
//! farther you sail, the rougher the sea and the faster they come. Three lives,
//! a score for the leagues you sail, and a bonus when a fireball passes close.
//!
//! A and D (or the arrows) steer, p or Esc pauses, q quits.

use std::f32::consts::{PI, TAU};
use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(start(tuning()))
}

/// The numbers the website lets you change while the game runs.
pub fn tuning() -> Tuning {
    Tuning::new()
        .knob("fireball_rate", "fireball rate (x)", 1.0, 0.2, 3.0)
        .knob("wave_height", "wave height (x)", 1.0, 0.0, 2.0)
        .knob("boat_speed", "boat speed (x)", 1.0, 0.5, 2.0)
}

/// The title screen, reading `tuning` while it plays.
pub fn start(tuning: Tuning) -> impl Game {
    Title::new(0).tuned(tuning)
}

// ---------------------------------------------------------------------------
// Time, distance and how far the voyage has got
// ---------------------------------------------------------------------------

/// Seconds of play in one day, from dawn to dawn.
const DAY_SECONDS: f32 = 120.0;

/// Leagues at which the voyage reaches its roughest: progress 1. About 150
/// seconds of play, since the boat sails a bit faster as it goes.
const DISTANCE_MAX: f32 = 165.0;

/// Leagues a second at the start, and at progress 1.
const SPEED_CALM: f32 = 1.0;
const SPEED_STORM: f32 = 1.2;

/// Clamps `x` to 0..1, and reads a value that is not finite as 0.
fn unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The straight line from `from` to `to`, at `t` (0 to 1).
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// How far the voyage has got, from 0 at the start to 1 at [`DISTANCE_MAX`]
/// leagues. The sea, the fireballs and the speed all follow it.
fn progress_of(distance: f32) -> f32 {
    unit(distance / DISTANCE_MAX)
}

/// Leagues a second the boat sails at `progress`.
fn sail_speed(progress: f32) -> f32 {
    lerp(SPEED_CALM, SPEED_STORM, unit(progress))
}

/// How far through the day `seconds` of play is, from 0 (dawn) to 1 (the next dawn).
fn day_phase(seconds: f32) -> f32 {
    if seconds.is_finite() {
        (seconds / DAY_SECONDS).rem_euclid(1.0)
    } else {
        0.0
    }
}

/// How bright the sky is, from 0 (midnight) to 1 (midday), at `seconds` of play.
/// It starts at dawn (0.5), so the sky is brightest at 30 seconds.
fn daylight(seconds: f32) -> f32 {
    (day_phase(seconds) * TAU).sin() * 0.5 + 0.5
}

/// How dark the sky is, from 0 (no night) to 1 (midnight), given the daylight
/// from [`daylight`]. It is zero from dawn to dusk and grows toward midnight.
/// Stars fade in once it passes [`STAR_NIGHT`].
fn night(daylight: f32) -> f32 {
    ((0.5 - unit(daylight)) * 2.0).clamp(0.0, 1.0)
}

/// The night amount above which stars start to show.
const STAR_NIGHT: f32 = 0.4;

/// The name of the time of day at `seconds` of play, for the HUD.
fn time_of_day(seconds: f32) -> &'static str {
    let light = daylight(seconds);
    let phase = day_phase(seconds);
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

/// Where the still water sits, as a share of the screen height from the top.
const WATER_LEVEL: f32 = 0.72;

/// The row of the still water on a screen `rows` high.
fn level(rows: f32) -> f32 {
    WATER_LEVEL * rows.max(1.0)
}

// ---------------------------------------------------------------------------
// The sea: four swells, from calm to rough
// ---------------------------------------------------------------------------

/// The sea's lowest trough may come no closer than this many rows to the bottom
/// of the screen, so the keel of the boat (which sits in the trough) stays on it.
const BOTTOM_ROOM: f32 = 1.5;

/// The most the swells can lift or dip the sea at `progress`, in rows: the sum of
/// their heights, with the set at its biggest.
fn deepest_swell(progress: f32) -> f32 {
    let total: f32 = swells(progress, 0.0).iter().map(|s| s.0).sum();
    total * (1.0 + SET_DEPTH)
}

/// How much the swells are scaled on a screen `rows` high at `progress`. It is 1
/// when the deepest trough keeps [`BOTTOM_ROOM`] to spare, and otherwise shrinks
/// the swells so that it just fits. A tall screen keeps the full swell.
fn swell_fit(progress: f32, rows: f32) -> f32 {
    let room = (rows - BOTTOM_ROOM - level(rows)).max(0.0);
    let deep = deepest_swell(progress);
    if deep <= room {
        1.0
    } else {
        room / deep
    }
}

/// The four swells at progress 0 (calm) and at progress 1 (rough). Wavelengths
/// are in columns, heights in rows, and speeds in wavelengths a second (negative
/// runs left, so the boat seems to sail right). The wavelengths are not simple
/// multiples of one another, so the sea never settles into a pattern. The
/// fourth swell fades in from nothing.
const CALM_WAVELENGTH: [f32; 4] = [38.0, 23.0, 14.0, 9.0];
const STORM_WAVELENGTH: [f32; 4] = [15.0, 8.5, 4.4, 2.9];
const CALM_HEIGHT: [f32; 4] = [0.5, 0.25, 0.1, 0.0];
const STORM_HEIGHT: [f32; 4] = [2.4, 1.2, 0.7, 0.35];
const CALM_SPEED: [f32; 4] = [-0.05, -0.08, -0.12, -0.2];
const STORM_SPEED: [f32; 4] = [-0.15, -0.25, -0.35, -0.5];

/// The sets: the whole sea swells and dies down, by this share, this fast.
const SET_DEPTH: f32 = 0.35;
const SET_SPEED: f32 = 0.27;

/// The `(height, wavelength, speed)` of each swell at `progress`. Each wavelength
/// is nudged by up to 4% by `phase`, which is set per run, so no two seas match.
/// The nudge does not change with progress, so the swells change smoothly.
fn swells(progress: f32, phase: f32) -> [(f32, f32, f32); 4] {
    let p = unit(progress);
    let mut out = [(0.0, 1.0, 0.0); 4];
    for (i, slot) in out.iter_mut().enumerate() {
        let nudge = 1.0 + 0.04 * (phase + i as f32 * 2.3).sin();
        *slot = (
            lerp(CALM_HEIGHT[i], STORM_HEIGHT[i], p),
            lerp(CALM_WAVELENGTH[i], STORM_WAVELENGTH[i], p) * nudge,
            lerp(CALM_SPEED[i], STORM_SPEED[i], p),
        );
    }
    out
}

/// The sea at `progress`, at play time `t`, for a run with the given `phase`.
/// It is rebuilt every tick. The swells change slowly, so the height at one
/// column changes only a little from one tick to the next. The game itself now
/// calls [`sea_scaled`]; this is the unscaled sea the tests compare against.
#[cfg(test)]
fn sea(progress: f32, t: f32, phase: f32, rows: f32) -> Waves {
    sea_scaled(progress, t, phase, rows, 1.0)
}

/// [`sea`] with every swell's height multiplied by `scale` (the wave height knob).
fn sea_scaled(progress: f32, t: f32, phase: f32, rows: f32, scale: f32) -> Waves {
    let set = 1.0 + SET_DEPTH * (SET_SPEED * t + phase).sin();
    let fit = swell_fit(progress, rows) * scale;
    let mut waves = Waves::new();
    for (height, wavelength, speed) in swells(progress, phase) {
        waves = waves.layer(height * set * fit, wavelength, speed);
    }
    waves
}

/// The shade of the sea at `depth` rows below the surface, for the column `x`.
/// It is brightest (foam) at the surface and darkens with depth. The small
/// ripple in the bands follows the surface a few columns away, so the depths
/// echo the waves. Nothing here is random, so neighbouring cells never jump.
fn sea_value(depth: f32, x: f32, t: f32, waves: &Waves) -> f32 {
    let band = 0.08 * (1.7 * depth + 0.8 * waves.height(x - 3.0, t)).sin();
    (1.0 - 0.4 * depth + band).clamp(-1.0, 1.0)
}

/// Whether the picture at pixel `(px, py)` is above the water in its column.
fn above_water(waves: &Waves, t: f32, water: f32, px: i32, py: i32) -> bool {
    let centre_row = (py as f32 + 0.5) / 2.0;
    centre_row < waves.surface_y(px as f32, t, water)
}

// ---------------------------------------------------------------------------
// Difficulty
// ---------------------------------------------------------------------------

/// Seconds between fireballs, from the first to the stormiest.
const SPAWN_CALM: f32 = 1.8;
const SPAWN_STORM: f32 = 0.4;

/// Seconds between fireballs at `progress`. It falls as the voyage goes on.
fn spawn_interval(progress: f32) -> f32 {
    lerp(SPAWN_CALM, SPAWN_STORM, unit(progress))
}

/// How much faster fireballs fall at `progress`, from 1 to 1.6.
fn fall_scale(progress: f32) -> f32 {
    lerp(1.0, 1.6, unit(progress))
}

/// The chance that a spawn comes as a volley of two or three, at `progress`.
/// There are none at the start, and this chance is the most at the end.
const VOLLEY_CHANCE: f32 = 0.35;
fn volley_chance(progress: f32) -> f32 {
    VOLLEY_CHANCE * unit(progress)
}

// ---------------------------------------------------------------------------
// The boat
// ---------------------------------------------------------------------------

/// The boat's picture is this many pixels wide and tall. A pixel is a column
/// across and a half row down, so it is square on screen.
const BOAT_WIDTH: f32 = 17.0;
const BOAT_HEIGHT: f32 = 12.0;

/// Columns a second the boat sails sideways when a key is held, and how quickly
/// its speed eases toward that.
const SAIL_SPEED: f32 = 22.5;
const SAIL_EASE: f32 = 5.0;

/// How fast the hull's keel chases the water, as a spring: stiffness and damping.
const SPRING: f32 = 36.0;
const DAMPING: f32 = 7.0;

/// The boat: `x` is its left column, and `keel` is the pixel row of the bottom
/// of its picture, which sits just under the water. It only moves sideways.
#[derive(Clone, Copy, Debug)]
struct Boat {
    x: f32,
    vx: f32,
    keel: f32,
    keel_v: f32,
}

impl Boat {
    /// A boat not yet on the water. Its `keel` is NaN until [`Boat::place`].
    fn new() -> Boat {
        Boat {
            x: f32::NAN,
            vx: 0.0,
            keel: f32::NAN,
            keel_v: 0.0,
        }
    }

    /// The column of the middle of the hull.
    fn centre(&self) -> f32 {
        self.x + BOAT_WIDTH / 2.0
    }

    /// The pixel row of the top of the picture.
    fn top(&self) -> f32 {
        self.keel - (BOAT_HEIGHT - 1.0)
    }

    /// The picture as a rectangle, for collisions. Both are in pixels.
    fn hit(&self) -> Hit {
        Hit::rect(self.x, self.top(), BOAT_WIDTH, BOAT_HEIGHT)
    }

    /// On the first call, puts the boat a third of the way across the screen,
    /// with its keel where `keel_under` says the water is. Then does nothing.
    fn place(&mut self, width: f32, keel_under: impl Fn(f32) -> f32) {
        if self.keel.is_nan() {
            self.x = width * 0.3;
            self.keel = keel_under(self.centre());
            self.keel_v = 0.0;
        }
    }

    /// Eases the speed toward `wanted` columns a second, then moves the boat
    /// and keeps its left column between `near` and `far`. Reaching an edge also
    /// stops the boat, so it does not push against it. `min` then `max` is used
    /// rather than `clamp`, which panics when its bounds cross.
    fn steer(&mut self, wanted: f32, dt: f32, near: f32, far: f32) {
        let ease = (dt * SAIL_EASE).min(1.0);
        self.vx += (wanted - self.vx) * ease;
        let free = self.x + self.vx * dt;
        let moved = free.min(far).max(near);
        if moved != free {
            self.vx = 0.0;
        }
        self.x = moved;
    }

    /// Pulls the keel toward `target` with a spring, so the boat bobs on the
    /// swell rather than snapping to it.
    fn settle(&mut self, target: f32, dt: f32) {
        let accel = SPRING * (target - self.keel) - DAMPING * self.keel_v;
        self.keel_v += accel * dt;
        self.keel += self.keel_v * dt;
    }
}

/// Whether the boat can be hit: it cannot be while its invulnerable time lasts.
/// It blinks, off on every other [`BLINK_SECONDS`] slot, so the player sees it.
fn boat_visible(elapsed: f32, invulnerable_until: f32) -> bool {
    if elapsed >= invulnerable_until {
        return true;
    }
    (elapsed / BLINK_SECONDS).floor() as i64 % 2 == 0
}

/// How long one on or off slot of an invulnerable boat's blinking lasts.
const BLINK_SECONDS: f32 = 0.08;

/// The keel row for the water under column `x`: just below the surface.
fn keel_under(waves: &Waves, x: f32, t: f32, water: f32) -> f32 {
    2.0 * waves.surface_y(x, t, water) + 1.0
}

// ---------------------------------------------------------------------------
// Fireballs
// ---------------------------------------------------------------------------

/// A fireball is a disc of this many pixels across the radius, with a glow.
const FIREBALL_RADIUS: f32 = 2.5;
const FIREBALL_GLOW: f32 = 7.0;

/// The most fireballs at once.
const FIREBALL_LIMIT: usize = 16;

/// A fireball starts this many pixels above the top (so it falls into view), in
/// pixels a second down at the start, and how fast that speeds up.
const FALL_FROM: f32 = -2.0;
const FALL_START: f32 = 4.8;
const FALL_GRAVITY: f32 = 12.8;

/// Wind pushes fireballs left, by this many columns a second.
const WIND_MIN: f32 = 3.4;
const WIND_MAX: f32 = 10.2;

/// One spawn in this many aims near the boat, within this many columns.
const AIM_CHANCE: f32 = 0.4;
const AIM_SPREAD: f32 = 5.0;

/// A fireball that has drifted this far past the left edge has left the screen.
const OFF_LEFT: f32 = -4.0;

/// Once a fireball reaches the water it sinks for this long, sliding down
/// slowly and fading from fire to grey. Drag stops it quickly at first.
const SINK_SECONDS: f32 = 1.2;
const SINK_DRAG: f32 = 6.0;
const SINK_DRIFT: f32 = 3.0;

/// The colours a sinking fireball steps through, from fire to grey.
const SINK_RAMP: [Colour; 5] = [
    Colour::BrightYellow,
    Colour::BrightRed,
    Colour::White,
    Colour::BrightBlack,
    Colour::Blue,
];

/// Which colour of [`SINK_RAMP`] a fireball is, given how far through its
/// sinking it is (0 to 1). It only goes up as the sinking goes on.
fn sink_step(sunk: f32) -> usize {
    ((unit(sunk) * SINK_RAMP.len() as f32) as usize).min(SINK_RAMP.len() - 1)
}

/// The colour of a sinking fireball that is `sunk` through its sinking.
fn sink_colour(sunk: f32) -> Colour {
    SINK_RAMP[sink_step(sunk)]
}

/// Where a fireball is in its life.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    /// In the air. It can hit the boat.
    Flying,
    /// In the water, for this many seconds. It cannot hit the boat.
    Sinking(f32),
    /// Gone: removed at the end of the tick.
    Spent,
}

/// A fireball. `x` is a column and `y` a pixel row. `vx` is columns a second
/// and `vy` pixels a second, and `gravity` speeds up `vy`. `passed` is set once
/// it has crossed the boat's column, so its near miss counts once.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fireball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    gravity: f32,
    passed: bool,
    stage: Stage,
}

impl Fireball {
    fn new(x: f32, y: f32, vx: f32, vy: f32, gravity: f32) -> Fireball {
        Fireball {
            x,
            y,
            vx,
            vy,
            gravity,
            passed: false,
            stage: Stage::Flying,
        }
    }

    /// The fireball as a circle for collisions, in pixels.
    fn hit_shape(&self) -> Hit {
        Hit::circle(self.x, self.y, FIREBALL_RADIUS)
    }

    /// Carries a flying fireball on by `dt`: gravity speeds its fall and the
    /// wind carries it left.
    fn fall(&mut self, dt: f32) {
        self.vy += self.gravity * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
    }

    /// Carries a sinking fireball on by `dt`. Drag slows it, and it drifts down
    /// a little. When its time is up it is spent.
    fn sink(&mut self, dt: f32) {
        let keep = (-SINK_DRAG * dt).exp();
        self.vx *= keep;
        self.vy *= keep;
        self.y += (self.vy + SINK_DRIFT) * dt;
        if let Stage::Sinking(age) = self.stage {
            let age = age + dt;
            self.stage = if age >= SINK_SECONDS {
                Stage::Spent
            } else {
                Stage::Sinking(age)
            };
        }
    }

    /// How far through its sinking it is, from 0 (just reached the water) to 1
    /// (gone). A flying fireball is at 0.
    fn sunk(&self) -> f32 {
        match self.stage {
            Stage::Sinking(age) => unit(age / SINK_SECONDS),
            _ => 0.0,
        }
    }

    /// The radius in pixels: full while flying, shrinking to nothing as it sinks.
    fn radius(&self) -> f32 {
        FIREBALL_RADIUS * (1.0 - self.sunk())
    }
}

/// How long it takes to fall `distance` pixels from `v0` pixels a second, with
/// `gravity` speeding it up. Used to aim a fireball at the boat.
fn fall_time(distance: f32, v0: f32, gravity: f32) -> f32 {
    if gravity <= 0.0 {
        return distance / v0.max(f32::MIN_POSITIVE);
    }
    (-v0 + (v0 * v0 + 2.0 * gravity * distance.max(0.0)).sqrt()) / gravity
}

/// The gap in pixels between a fireball at `y` and the boat's top and bottom.
/// Zero when it is level with the boat.
fn gap_to_boat(y: f32, top: f32, bottom: f32) -> f32 {
    if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0.0
    }
}

/// A fireball this close to the boat as it passes, but missing, scores a bonus.
const NEAR_PX: f32 = 6.0;
const NEAR_BONUS: u32 = 5;

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

// ---------------------------------------------------------------------------
// Lives and effects
// ---------------------------------------------------------------------------

/// The boat starts with this many lives, and is safe for this long after a hit.
const LIVES: u32 = 3;
const INVULNERABLE_SECONDS: f32 = 1.5;

/// How long the screen flashes after a hit.
const FLASH_SECONDS: f32 = 0.15;

/// Columns a second the island scrolls, as a share of the screen: the island
/// moves one screen width for every this many leagues sailed.
const ISLAND_LEAGUES: f32 = 10.0;

/// How long a wake droplet lives and how often one is shed.
const WAKE_SECONDS: f32 = 0.15;
const SPRAY_FALL: f32 = 12.0;

/// The most spray droplets, embers and steam particles at once.
const SPRAY_LIMIT: usize = 200;
const EMBER_LIMIT: usize = 400;
const STEAM_LIMIT: usize = 160;

/// Embers a flying fireball leaves each tick, how long each lives in seconds,
/// and how fast they rise (negative gravity: rows a second each second).
const EMBERS_PER_TICK: usize = 2;
const EMBER_LIFE: f32 = 0.5;
const EMBER_RISE: f32 = -6.0;

/// Steam in the cloud when a fireball reaches the water, and grey droplets.
const STEAM_BURST: usize = 14;
const DROPLETS: usize = 5;

// ---------------------------------------------------------------------------
// The sky
// ---------------------------------------------------------------------------

/// The sky's colours, from the darkest to the brightest: night, a faint glow at
/// the horizon, the reds and yellows of sunrise and sunset, then white by day.
/// There is no blue or cyan here: those belong to the water.
const SKY_RAMP: [Colour; 7] = [
    Colour::Black,
    Colour::BrightBlack,
    Colour::Red,
    Colour::BrightRed,
    Colour::BrightYellow,
    Colour::White,
    Colour::BrightWhite,
];

/// The palette the sky is drawn with.
fn sky_palette() -> Palette {
    Palette::new(&SKY_RAMP)
}

/// How much the horizon glows at the bottom of the sky, on the 0 to 1 ramp.
const HORIZON_GLOW: f32 = 0.07;

/// Eases a value from 0 to 1 so that it stays low for longer at the bottom and
/// top: night stays black well past dusk, and day stays white.
fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

/// The value of the sky at one cell, from -1 (darkest) to 1 (brightest), for
/// the given daylight, flash and play time. Clouds drift in slow bands: at noon
/// they are a little grey against the white, at dawn and dusk they catch the
/// light, and at night they are hard to see.
fn sky_value(cell: &Cell, daylight: f32, flash: f32, t: f32) -> f32 {
    let horizon = if cell.height > 0.0 {
        unit(cell.y / cell.height)
    } else {
        1.0
    };
    let cloud = cloud_amount(cell.x, cell.y, t);
    let twilight = 1.0 - (2.0 * unit(daylight) - 1.0).abs();
    let light = smooth(unit(daylight)) + HORIZON_GLOW * horizon - 0.14 * daylight * cloud
        + 0.12 * twilight * cloud
        + 0.2 * flash;
    unit(light) * 2.0 - 1.0
}

/// How much cloud is at pixel `(x, y)` at play time `t`, from 0 (clear) to 1.
fn cloud_amount(x: f32, y: f32, t: f32) -> f32 {
    let wave = (x * 0.07 + t * 0.12).sin() + (x * 0.031 - y * 0.35 + t * 0.05).sin();
    ((wave - 1.1) * 1.5).clamp(0.0, 1.0)
}

/// Stars over the sky above pixel row `sky_bottom`, twinkling with play time
/// `t`. They are single pixels, only at night, and fullest at midnight.
fn paint_stars(screen: &mut Screen, night: f32, t: f32, sky_bottom: f32) {
    if night <= STAR_NIGHT {
        return;
    }
    let fullness = unit((night - STAR_NIGHT) / (1.0 - STAR_NIGHT));
    // out of every thousand pixels, this many are stars at full night
    let chance = (fullness * 30.0) as u32;
    let twinkle = (t * 2.0).floor() as i32;
    let width = screen.width();
    // pixel rows 0 and 1 are the HUD's, so the stars start at row 2
    for py in 2..sky_bottom.floor() as i32 {
        for px in 0..width {
            if mix(px, py, 0) % 1000 >= chance || mix(px, py, twinkle) % 4 == 0 {
                continue;
            }
            let colour = match mix(px, py, 1) % 3 {
                0 => Colour::BrightWhite,
                1 => Colour::White,
                _ => Colour::BrightYellow,
            };
            screen.pixel(px, py, colour);
        }
    }
}

/// A cheap, deterministic hash of a pixel and a salt, using only wrapping
/// arithmetic, so the same inputs always give the same bits and nothing panics.
fn mix(x: i32, y: i32, salt: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ (salt as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// Where a body is on its arc across the sky at `seconds`: how far along the
/// arc from the left (0 to 1) and how high (0 at the horizon, 1 at the top).
/// `start` is the day phase at which it rises. None when it is below the horizon.
fn arc(seconds: f32, start: f32) -> Option<(f32, f32)> {
    if !seconds.is_finite() {
        return None;
    }
    let along = (day_phase(seconds) - start).rem_euclid(1.0) * 2.0;
    if along >= 1.0 {
        return None;
    }
    Some((along, (PI * along).sin()))
}

/// The sun rises at dawn and sets at dusk. None at night.
fn sun_arc(seconds: f32) -> Option<(f32, f32)> {
    arc(seconds, 0.0)
}

/// The moon is the opposite: it rises at dusk and sets at dawn. None by day.
fn moon_arc(seconds: f32) -> Option<(f32, f32)> {
    arc(seconds, 0.5)
}

/// The pixel row a body at `height` (0 to 1) sits on: the horizon at 0, the top
/// of the sky at 1.
fn body_row(height: f32, water: f32) -> f32 {
    lerp(2.0 * water, SKY_TOP, height)
}

/// The row the sun and moon reach at their highest, clear of the HUD.
const SKY_TOP: f32 = 9.0;
const SUN_RADIUS: f32 = 4.0;
const SUN_GLOW: f32 = 14.0;
const MOON_RADIUS: f32 = 3.0;
const MOON_GLOW: f32 = 9.0;

/// Draws a disc of pixels, but only the pixels that are above the water, so
/// the waves cover the lower edge of a body that is setting.
#[allow(clippy::too_many_arguments)]
fn disc_above_water(
    screen: &mut Screen,
    centre: (f32, f32),
    radius: f32,
    colour: Colour,
    waves: &Waves,
    t: f32,
    water: f32,
) {
    let (cx, cy) = centre;
    let reach = radius.ceil() as i32 + 1;
    let (x0, y0) = (cx.round() as i32, cy.round() as i32);
    for py in (y0 - reach)..=(y0 + reach) {
        for px in (x0 - reach)..=(x0 + reach) {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            if dx * dx + dy * dy < radius * radius && above_water(waves, t, water, px, py) {
                screen.pixel(px, py, colour);
            }
        }
    }
}

/// Draws the sun and the moon, each with a soft glow, at their places on the
/// arc for `seconds` of play. Their pixels are clipped to the sky.
#[allow(clippy::too_many_arguments)]
fn paint_celestial(
    screen: &mut Screen,
    look: &Look,
    seconds: f32,
    waves: &Waves,
    t: f32,
    water: f32,
) {
    let width = screen.width() as f32;
    if let Some((along, height)) = sun_arc(seconds) {
        let centre = (lerp(0.12, 0.88, along) * width, body_row(height, water));
        screen.glow(centre.0, centre.1, SUN_GLOW, &look.sun_glow);
        disc_above_water(
            screen,
            centre,
            SUN_RADIUS,
            Colour::BrightYellow,
            waves,
            t,
            water,
        );
    }
    if let Some((along, height)) = moon_arc(seconds) {
        let centre = (lerp(0.88, 0.12, along) * width, body_row(height, water));
        screen.glow(centre.0, centre.1, MOON_GLOW, &look.grey_glow);
        disc_above_water(
            screen,
            centre,
            MOON_RADIUS,
            Colour::BrightWhite,
            waves,
            t,
            water,
        );
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

/// The colour of the sea palette at `value` (-1 to 1), for a single pixel.
fn ramp_colour(palette: &Palette, value: f32) -> Colour {
    let colours = palette.colours();
    let steps = colours.len() - 1;
    let position = ((unit((value + 1.0) / 2.0)) * steps as f32).round() as usize;
    colours[position.min(steps)]
}

/// Paints the sea from a little above the still water to the bottom of the
/// screen. Each cell is shaded with a block, as the sketch art does. A cell
/// whose top half is in the sky but whose bottom half is in the water gets a
/// lower-half pixel, so the surface moves at half-row steps and not whole rows.
fn paint_sea(screen: &mut Screen, palette: &Palette, waves: &Waves, t: f32, water: f32) {
    let (width, height) = screen.size();
    for x in 0..width as i32 {
        let column = x as f32;
        let surface = waves.surface_y(column, t, water);
        let first = (surface.floor() as i32 - 1).max(0);
        for y in first..height as i32 {
            let row = y as f32;
            if row + 0.25 > surface {
                let value = sea_value(row + 0.5 - surface, column, t, waves);
                let (block, fg, bg) = palette.shade_signed(value);
                screen.put_on(x, y, block, fg, bg);
            } else if row + 0.75 > surface {
                let value = sea_value(row + 0.75 - surface, column, t, waves);
                screen.pixel(x, 2 * y + 1, ramp_colour(palette, value));
            }
        }
    }
}

/// Paints the pixels of `art` with its top left at pixel `(x, y)`, leaving out
/// every pixel below the water, so the island sinks behind the waves.
#[allow(clippy::too_many_arguments)]
fn paint_art_above_water(
    screen: &mut Screen,
    art: &PixelArt,
    x: i32,
    y: i32,
    waves: &Waves,
    t: f32,
    water: f32,
) {
    for row in 0..art.height() {
        for column in 0..art.width() {
            let Some(colour) = art.pixel(column, row) else {
                continue;
            };
            let (px, py) = (x + column as i32, y + row as i32);
            if above_water(waves, t, water, px, py) {
                screen.pixel(px, py, colour);
            }
        }
    }
}

/// The column of the island's left edge after the boat has sailed `distance`
/// leagues. It starts on the left and scrolls away.
fn island_column(distance: f32, width: f32) -> f32 {
    width * 0.08 - distance / ISLAND_LEAGUES * width
}

/// Paints the island on the horizon at the left, for the voyage's `distance`.
#[allow(clippy::too_many_arguments)]
fn paint_island(
    screen: &mut Screen,
    look: &Look,
    distance: f32,
    waves: &Waves,
    t: f32,
    water: f32,
) {
    let x = island_column(distance, screen.width() as f32).round() as i32;
    let top = (2.0 * water).round() as i32 - ISLAND_RISE;
    paint_art_above_water(screen, &look.island, x, top, waves, t, water);
}

/// How many pixel rows the island's picture is, from its top to the water: its
/// sand sits on the water, and the waves cover what is below it.
const ISLAND_RISE: i32 = 11;

/// Draws the boat's picture with its left column at `x` and its keel at pixel
/// row `keel`, leaning as the water under it does.
fn draw_boat(screen: &mut Screen, look: &Look, x: f32, keel: f32, tilt: Tilt) {
    let top = keel - (BOAT_HEIGHT - 1.0);
    let art = match tilt {
        Tilt::Left => &look.boats[0],
        Tilt::Level => &look.boats[1],
        Tilt::Right => &look.boats[2],
    };
    screen.pixel_art(x.round() as i32, top.round() as i32, art);
}

/// Draws a fireball: a fire-coloured disc with a glow while it flies, and as it
/// sinks, a grey one that shrinks to nothing.
fn draw_fireball(screen: &mut Screen, look: &Look, f: &Fireball) {
    match f.stage {
        Stage::Flying => {
            screen.glow(f.x, f.y, FIREBALL_GLOW, &look.fire_glow);
            screen.disc(f.x, f.y, FIREBALL_RADIUS, Colour::BrightRed);
            screen.disc(f.x, f.y, FIREBALL_RADIUS * 0.5, Colour::BrightYellow);
        }
        Stage::Sinking(_) => {
            let sunk = f.sunk();
            screen.glow(f.x, f.y, FIREBALL_GLOW * (1.0 - sunk), &look.grey_glow);
            screen.disc(f.x, f.y, f.radius(), sink_colour(sunk));
        }
        Stage::Spent => {}
    }
}

/// What the HUD shows.
struct Hud {
    /// Leagues sailed, in tenths of a league.
    tenths: u32,
    score: u32,
    best: u32,
    lives: u32,
    time: &'static str,
}

/// The strip along the top: the leagues and the score on the left, the time of
/// day in the middle, the best score and one heart per life on the right (red
/// while it lasts, grey once lost).
fn draw_hud(screen: &mut Screen, hud: &Hud) {
    let width = screen.width();
    screen.fill(0, 0, width, 1, ' ', Colour::White, Colour::Black);
    draw_leagues(screen, 1, 0, hud.tenths, Colour::BrightWhite);
    screen.text(17, 0, "score ", Colour::BrightYellow);
    number(screen, 23, 0, hud.score, Colour::BrightYellow);
    screen.text_centred(0, hud.time, Colour::BrightCyan);
    let best_at = width - 2 * LIVES as i32 - 12;
    screen.text(best_at, 0, "best ", Colour::White);
    number(screen, best_at + 5, 0, hud.best, Colour::White);
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

/// Draws the distance as leagues with one decimal, such as `12.4 leagues`,
/// starting at `(x, y)`, without allocating a string. Returns the column after.
fn draw_leagues(screen: &mut Screen, x: i32, y: i32, tenths: u32, colour: Colour) -> i32 {
    let end = number(screen, x, y, tenths / 10, colour);
    screen.put(end, y, '.', colour);
    screen.put(end + 1, y, (b'0' + (tenths % 10) as u8) as char, colour);
    screen.text(end + 2, y, " leagues", colour);
    end + 2 + " leagues".len() as i32
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

// ---------------------------------------------------------------------------
// Pictures
// ---------------------------------------------------------------------------

/// The boat's colours: a red hull with a bright red trim, a yellow deck and
/// mast, a white sail with a dark foot (so it shows against the bright sky), and
/// a red pennant.
const BOAT_LEGEND: [(char, Colour); 7] = [
    ('H', Colour::Red),
    ('T', Colour::BrightRed),
    ('D', Colour::Yellow),
    ('M', Colour::Yellow),
    ('F', Colour::BrightRed),
    ('S', Colour::White),
    ('W', Colour::BrightBlack),
];

/// The boat, level on the water. The hull is the bottom rows.
const BOAT_LEVEL: &str = r"
        MFF
        M
        MS
        MSS
        MSSS
        MSSSS
        MSSSSS
        MWWWWW
     DDDDDDDDDDD
    HHHHHHHHHHHHH
     HHHHHHHHHHHH
      THHHHHHHHHT
";

/// The boat leaning left: its hull is shorter at the back and the bow dips.
const BOAT_LEFT: &str = r"
        MFF
        M
        MS
        MSS
        MSSS
        MSSSS
        MSSSSS
        MWWWWW
      DDDDDDDDD
   HHHHHHHHHHHHH
    HHHHHHHHHHHH
     THHHHHHHHHT
";

/// The island's colours: sand, a green hill, the palm's fronds and trunk.
const ISLAND_LEGEND: [(char, Colour); 6] = [
    ('Y', Colour::Yellow),
    ('y', Colour::BrightYellow),
    ('G', Colour::Green),
    ('g', Colour::BrightGreen),
    ('T', Colour::Red),
    ('L', Colour::BrightGreen),
];

/// The island: a sand mound with a green hill and a palm tree, on the horizon.
const ISLAND: &str = r"
                 LLLLL
                LLLLLLL
                  LTL
                   T
         GG        T
       GGGGGG      T
     GGGGGGGGGG    T
   GGGGGGGGGGGGGG  T
GGGGGGGGGGGGGGGGGGGG
yyyyyyyyyyyyyyyyyyyyyyyyyyyy
YYYYYYYYYYYYYYYYYYYYYYYYYYYYYY
";

/// The colours and pictures the game draws with. Made once per scene, so that
/// drawing allocates nothing.
struct Look {
    sky: Palette,
    sea: Palette,
    fire_glow: Palette,
    grey_glow: Palette,
    sun_glow: Palette,
    /// Indexed as [`draw_boat`] reads it: leaning left, level, leaning right.
    boats: [PixelArt; 3],
    island: PixelArt,
}

impl Look {
    fn new() -> Look {
        let left = PixelArt::new(BOAT_LEFT, &BOAT_LEGEND);
        Look {
            sky: sky_palette(),
            sea: Palette::ocean(),
            fire_glow: Palette::new(&[
                Colour::Black,
                Colour::Red,
                Colour::BrightRed,
                Colour::Yellow,
                Colour::BrightYellow,
            ]),
            grey_glow: Palette::new(&[Colour::Black, Colour::BrightBlack, Colour::White]),
            sun_glow: Palette::new(&[
                Colour::Red,
                Colour::BrightRed,
                Colour::Yellow,
                Colour::BrightYellow,
            ]),
            boats: [
                left.clone(),
                PixelArt::new(BOAT_LEVEL, &BOAT_LEGEND),
                left.flipped(),
            ],
            island: PixelArt::new(ISLAND, &ISLAND_LEGEND),
        }
    }
}

// ---------------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------------

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

/// Columns from one block letter to the next: five wide and one space.
const LETTER_STEP: i32 = 6;

/// The first screen: the name in block letters, the sea and the sky, and a
/// boat sailing calmly away from an island.
struct Title {
    best: u32,
    look: Look,
    letters: [Sprite; 4],
    tuning: Tuning,
    /// The wave height knob as the last update read it.
    wave_scale: f32,
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
            tuning: Tuning::new(),
            wave_scale: 1.0,
        }
    }

    /// Reads the knobs from `tuning` while the title plays, and hands them on.
    fn tuned(mut self, tuning: Tuning) -> Title {
        self.tuning = tuning;
        self
    }
}

impl Game for Title {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        self.wave_scale = wave_scale_of(&self.tuning);
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed, self.best).tuned(self.tuning.clone()))
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
        let water = level(rows);
        let waves = sea_scaled(0.0, t, 0.0, rows, self.wave_scale);
        paint_sky(screen, &self.look.sky, daylight(t * 0.25), 0.0, t, rows);
        paint_sea(screen, &self.look.sea, &waves, t, water);
        // the island scrolls away as the boat sails, one league a second
        paint_island(screen, &self.look, t, &waves, t, water);
        let x = screen.width() as f32 / 3.0;
        let centre = x + BOAT_WIDTH / 2.0;
        let keel = keel_under(&waves, centre, t, water);
        draw_boat(screen, &self.look, x, keel, waves.tilt(centre, t));
        let word = LETTER_STEP * self.letters.len() as i32 - 1;
        let left = (screen.width() - word) / 2;
        // a dark band behind the letters, so they read over the busy sky
        screen.fill(left - 2, 1, word + 4, 7, ' ', Colour::White, Colour::Black);
        for (i, letter) in self.letters.iter().enumerate() {
            screen.sprite(left + LETTER_STEP * i as i32, 2, letter);
        }
        strip(screen, 0, "best ", self.best);
        let keys = "A/D or arrows steer   p pause   q quit";
        let panel = Panel::new(screen, keys.chars().count() as i32, 2, screen.height() - 5);
        panel.draw(screen);
        panel.text_line(screen, 0, "press Enter to sail", Colour::BrightWhite);
        panel.text_line(screen, 1, keys, Colour::White);
    }
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
        let text = "PAUSED  (p to resume)";
        let width = text.chars().count() as i32;
        let panel = Panel::new(screen, width, 1, (screen.height() - 3) / 2);
        panel.draw(screen);
        panel.text_line(screen, 0, text, Colour::BrightWhite);
    }
}

/// The game-over screen, shown when the last life is lost. The sky stays at the
/// time of day the round ended, and the sea keeps moving.
struct Over {
    score: u32,
    best: u32,
    at: f32,
    progress: f32,
    phase: f32,
    look: Look,
    tuning: Tuning,
    /// The wave height the round ended with, so the sea does not jump.
    wave_scale: f32,
}

impl Over {
    fn new(score: u32, best: u32, at: f32, progress: f32, phase: f32) -> Over {
        Over {
            score,
            best,
            at,
            progress,
            phase,
            look: Look::new(),
            tuning: Tuning::new(),
            wave_scale: 1.0,
        }
    }

    /// Reads the knobs from `tuning`, and passes them on to the next round.
    fn tuned(mut self, tuning: Tuning) -> Over {
        self.tuning = tuning;
        self
    }

    /// The wave height the sea is drawn with, as the round left it.
    fn waves_at(mut self, scale: f32) -> Over {
        self.wave_scale = scale;
        self
    }
}

impl Game for Over {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        self.wave_scale = wave_scale_of(&self.tuning);
        if ctx.keys.pressed(KeyCode::Enter) {
            let seed = ctx.seed.wrapping_add(ctx.tick);
            Next::switch(Play::new(seed, self.best).tuned(self.tuning.clone()))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        let t = screen.time();
        let rows = screen.height() as f32;
        let water = level(rows);
        let light = daylight(self.at);
        let waves = sea_scaled(self.progress, t, self.phase, rows, self.wave_scale);
        paint_sky(screen, &self.look.sky, light, 0.0, t, rows);
        paint_stars(screen, night(light), t, (water - 4.0) * 2.0);
        paint_celestial(screen, &self.look, self.at, &waves, t, water);
        paint_sea(screen, &self.look.sea, &waves, t, water);
        // the words sit in a panel up in the sky; it is drawn last, so nothing covers it
        let longest = (OVER_TITLE.chars().count() as i32)
            .max(OVER_HINT.chars().count() as i32)
            .max(6 + digit_count(self.score).max(digit_count(self.best)));
        let top = (screen.height() as f32 * 0.3) as i32;
        let panel = Panel::new(screen, longest, 6, top);
        panel.draw(screen);
        panel.text_line(screen, 0, OVER_TITLE, Colour::BrightRed);
        panel.number_line(screen, 2, "score ", self.score, Colour::BrightYellow);
        panel.number_line(screen, 3, "best ", self.best, Colour::White);
        panel.text_line(screen, 5, OVER_HINT, Colour::White);
    }
}

/// The game-over screen's first line and its last.
const OVER_TITLE: &str = "the sea wins this time";
const OVER_HINT: &str = "Enter: sail again   q: quit";

/// The colours of a panel's border and of the inside it is filled with.
const PANEL_EDGE: Colour = Colour::BrightYellow;
const PANEL_INSIDE: Colour = Colour::Black;

/// A framed panel of text, centred across the screen: a border, and lines of
/// text inside it. Draw it last, after everything it is over.
#[derive(Clone, Copy, Debug)]
struct Panel {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Panel {
    /// A panel for lines up to `width` columns wide, `lines` lines tall, with its
    /// top edge `top` rows down. It has two columns of padding inside its border.
    fn new(screen: &Screen, width: i32, lines: i32, top: i32) -> Panel {
        let w = width + 6;
        Panel {
            x: (screen.width() - w) / 2,
            y: top,
            w,
            h: lines + 2,
        }
    }

    /// Draws the border and fills the inside.
    fn draw(&self, screen: &mut Screen) {
        screen.frame(self.x, self.y, self.w, self.h, PANEL_EDGE, PANEL_INSIDE);
    }

    /// Line `line` (from 0) of `text`, centred inside the panel.
    fn text_line(&self, screen: &mut Screen, line: i32, text: &str, colour: Colour) {
        let left = self.x + (self.w - text.chars().count() as i32) / 2;
        screen.text(left, self.y + 1 + line, text, colour);
    }

    /// Line `line` of `label` and then `n`, centred inside the panel.
    fn number_line(&self, screen: &mut Screen, line: i32, label: &str, n: u32, colour: Colour) {
        let label_width = label.chars().count() as i32;
        let left = self.x + (self.w - label_width - digit_count(n)) / 2;
        let row = self.y + 1 + line;
        screen.text(left, row, label, colour);
        number(screen, left + label_width, row, n, colour);
    }
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

/// The keys the game answers to.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Left,
    Right,
    Pause,
}

fn controls() -> Controls<Action> {
    Controls::new()
        .bind(Action::Left, &[KeyCode::Left, KeyCode::Char('a')])
        .bind(Action::Right, &[KeyCode::Right, KeyCode::Char('d')])
        .bind(Action::Pause, &[KeyCode::Char('p'), KeyCode::Esc])
}

/// The game itself.
struct Play {
    best: u32,
    look: Look,
    elapsed: f32,
    /// Leagues sailed so far.
    distance: f32,
    /// Bonus points for fireballs that passed close.
    bonus: u32,
    /// Per run, so that no two seas match.
    phase: f32,
    boat: Boat,
    rng: Rng,
    controls: Controls<Action>,
    lives: u32,
    /// Play time until which the boat cannot be hit. Negative infinity means never hit.
    invulnerable_until: f32,
    /// Play time until which the screen flashes after a hit.
    flash_until: f32,
    fireballs: Pool<Fireball>,
    spawn: Timer,
    embers: Particles,
    steam: Particles,
    spray: Particles,
    wake: Timer,
    floaters: Pool<Floater>,
    /// The knobs the page may change while the round runs.
    tuning: Tuning,
    /// The wave height knob as the last update read it. The draw uses it too,
    /// so the boat sits on the waves that are drawn.
    wave_scale: f32,
}

/// The wave height knob's value, or 1 when no knob is set.
fn wave_scale_of(tuning: &Tuning) -> f32 {
    tuning.get("wave_height", 1.0)
}

impl Play {
    fn new(seed: u64, best: u32) -> Play {
        let mut rng = Rng::new(seed);
        let phase = rng.f32() * TAU;
        Play {
            best,
            look: Look::new(),
            tuning: Tuning::new(),
            wave_scale: 1.0,
            elapsed: 0.0,
            distance: 0.0,
            bonus: 0,
            phase,
            boat: Boat::new(),
            rng,
            controls: controls(),
            lives: LIVES,
            // finished from the start, so the boat begins vulnerable
            invulnerable_until: f32::NEG_INFINITY,
            flash_until: f32::NEG_INFINITY,
            fireballs: Pool::with_capacity(FIREBALL_LIMIT),
            spawn: Timer::every(SPAWN_CALM),
            embers: Particles::new(EMBER_LIMIT).gravity(EMBER_RISE).drag(0.5),
            steam: Particles::new(STEAM_LIMIT)
                .gravity(-6.0)
                .drag(0.5)
                .palette(Palette::new(&[
                    Colour::BrightBlack,
                    Colour::White,
                    Colour::BrightWhite,
                ])),
            spray: Particles::new(SPRAY_LIMIT)
                .gravity(SPRAY_FALL)
                .drag(0.2)
                .palette(Palette::new(&[
                    Colour::Cyan,
                    Colour::BrightCyan,
                    Colour::White,
                    Colour::BrightWhite,
                ])),
            wake: Timer::every(WAKE_SECONDS),
            floaters: Pool::with_capacity(FLOATER_LIMIT),
        }
    }

    /// Reads the knobs from `tuning` while the round runs.
    fn tuned(mut self, tuning: Tuning) -> Play {
        self.tuning = tuning;
        self
    }

    /// The score: a point for every tenth of a league sailed, and the bonuses.
    fn score(&self) -> u32 {
        ((self.distance * 10.0).floor() as u32).saturating_add(self.bonus)
    }

    /// Whether the screen is flashing after a hit.
    fn flashing(&self) -> bool {
        self.elapsed < self.flash_until
    }

    /// Sends one fireball, or a volley of two or three, down from above the top.
    fn launch(&mut self, width: f32, horizon: f32, progress: f32) {
        let volley = if self.rng.chance(volley_chance(progress)) {
            2 + self.rng.range_i32(0, 2)
        } else {
            1
        };
        for _ in 0..volley {
            let f = self.new_fireball(width, horizon, progress);
            let _ = self.fireballs.push(f);
        }
    }

    /// A fireball from above the top. Most are dropped at random across the
    /// screen; some are aimed where the boat will be, so they land on it.
    fn new_fireball(&mut self, width: f32, horizon: f32, progress: f32) -> Fireball {
        let scale = fall_scale(progress);
        let vx = -self.rng.range_f32(WIND_MIN, WIND_MAX);
        let vy = FALL_START * scale;
        let gravity = FALL_GRAVITY * scale;
        let x = if self.rng.chance(AIM_CHANCE) {
            let aim = self.boat.centre() + self.rng.range_f32(-AIM_SPREAD, AIM_SPREAD);
            // the wind carries it left, so start it as far right as it will drift
            let fall = fall_time(horizon - FALL_FROM, vy, gravity);
            aim - vx * fall
        } else {
            lerp(0.3 * width, 1.15 * width, self.rng.f32())
        };
        Fireball::new(x, FALL_FROM, vx, vy, gravity)
    }

    /// Moves every fireball, strikes the boat with the first flying one that
    /// touches it, and sinks any that reach the water. A fireball that passes
    /// close scores once.
    fn fly_fireballs(&mut self, dt: f32, waves: &Waves, water: f32) {
        let t = self.elapsed;
        let vulnerable = t >= self.invulnerable_until;
        let hull = self.boat.hit();
        let centre = self.boat.centre();
        let (top, bottom) = (self.boat.top(), self.boat.top() + BOAT_HEIGHT);
        let mut struck = false;
        for f in self.fireballs.iter_mut() {
            match f.stage {
                Stage::Flying => {
                    f.fall(dt);
                    if vulnerable && !struck && hull.overlaps(&f.hit_shape()) {
                        struck = true;
                        f.stage = Stage::Spent;
                        continue;
                    }
                    if f.y >= 2.0 * waves.surface_y(f.x, t, water) {
                        f.stage = Stage::Sinking(0.0);
                        let (x, row) = (f.x, f.y / 2.0);
                        self.steam
                            .burst(&mut self.rng, x, row, STEAM_BURST, 6.0, 1.2);
                        self.spray.burst(&mut self.rng, x, row, DROPLETS, 5.0, 0.6);
                    } else if f.x < OFF_LEFT {
                        f.stage = Stage::Spent;
                    }
                    if !f.passed && f.x <= centre {
                        f.passed = true;
                        let gap = gap_to_boat(f.y, top, bottom);
                        if f.stage == Stage::Flying && gap <= NEAR_PX {
                            self.bonus += NEAR_BONUS;
                            let _ = self.floaters.push(Floater {
                                x: centre,
                                y: top / 2.0,
                                age: 0.0,
                            });
                        }
                    }
                }
                Stage::Sinking(_) => f.sink(dt),
                Stage::Spent => {}
            }
        }
        if struck {
            self.lives = self.lives.saturating_sub(1);
            let row = self.boat.keel / 2.0 - 1.5;
            self.embers.burst(&mut self.rng, centre, row, 40, 14.0, 1.0);
            self.invulnerable_until = t + INVULNERABLE_SECONDS;
            self.flash_until = t + FLASH_SECONDS;
        }
        self.fireballs.retain(|f| f.stage != Stage::Spent);
    }
}

impl Game for Play {
    fn draw(&self, screen: &mut Screen) {
        // every value here comes from play time, so a paused game does not move
        let t = self.elapsed;
        let water = level(screen.height() as f32);
        let progress = progress_of(self.distance);
        let waves = sea_scaled(
            progress,
            t,
            self.phase,
            screen.height() as f32,
            self.wave_scale,
        );
        let light = daylight(t);
        let flash = if self.flashing() { 1.0 } else { 0.0 };
        paint_sky(
            screen,
            &self.look.sky,
            light,
            flash,
            t,
            screen.height() as f32,
        );
        paint_stars(screen, night(light), t, (water - 4.0) * 2.0);
        paint_celestial(screen, &self.look, t, &waves, t, water);
        paint_sea(screen, &self.look.sea, &waves, t, water);
        paint_island(screen, &self.look, self.distance, &waves, t, water);
        self.steam.draw(screen);
        self.embers.draw(screen);
        self.spray.draw(screen);
        for f in self.fireballs.iter() {
            draw_fireball(screen, &self.look, f);
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
        if boat_visible(t, self.invulnerable_until) {
            let centre = self.boat.centre();
            let tilt = waves.tilt(centre, t);
            draw_boat(screen, &self.look, self.boat.x, self.boat.keel, tilt);
        }
        let hud = Hud {
            tenths: (self.distance * 10.0).floor() as u32,
            score: self.score(),
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
        let dt = ctx.dt;
        let (width, rows) = (ctx.size.0 as f32, ctx.size.1 as f32);
        let water = level(rows);
        self.wave_scale = wave_scale_of(&self.tuning);
        let boat_speed = self.tuning.get("boat_speed", 1.0);
        let fireball_rate = self.tuning.get("fireball_rate", 1.0);
        self.elapsed += dt;
        self.distance += sail_speed(progress_of(self.distance)) * dt;
        let progress = progress_of(self.distance);
        let t = self.elapsed;
        let waves = sea_scaled(progress, t, self.phase, rows, self.wave_scale);

        self.boat.place(width, |x| keel_under(&waves, x, t, water));
        let across = self.controls.axis(ctx.keys, Action::Left, Action::Right) as f32;
        self.boat.steer(
            across * SAIL_SPEED * boat_speed,
            dt,
            width * 0.2,
            width * 0.6,
        );
        let target = keel_under(&waves, self.boat.centre(), t, water);
        self.boat.settle(target, dt);

        // the boat always sails forward, so it always leaves a wake behind its stern
        if self.wake.tick(dt) {
            let (x, row) = (self.boat.x, self.boat.keel / 2.0);
            self.spray.emit(x, row, -2.0, -1.5, 0.5);
        }
        self.spray.step(dt);
        self.steam.step(dt);
        self.embers.step(dt);

        self.spawn
            .set_seconds(spawn_interval(progress) / fireball_rate);
        let horizon = 2.0 * water;
        for _ in 0..self.spawn.ticks(dt) {
            self.launch(width, horizon, progress);
        }
        self.fly_fireballs(dt, &waves, water);
        // every fireball that is still flying leaves embers behind it
        for f in self.fireballs.iter() {
            if f.stage != Stage::Flying {
                continue;
            }
            for _ in 0..EMBERS_PER_TICK {
                let vx = self.rng.range_f32(0.5, 2.0);
                let vy = self.rng.range_f32(-1.5, 0.0);
                self.embers.emit(f.x + 1.0, f.y / 2.0, vx, vy, EMBER_LIFE);
            }
        }
        for floater in self.floaters.iter_mut() {
            floater.y -= FLOATER_RISE * dt;
            floater.age += dt;
        }
        self.floaters.retain(|floater| floater.age < FLOATER_LIFE);
        if self.lives == 0 {
            let best = self.best.max(self.score());
            let over = Over::new(self.score(), best, t, progress, self.phase);
            return Next::switch(over.tuned(self.tuning.clone()).waves_at(self.wave_scale));
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
                players: 1,
                me: PlayerId(0),
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

    /// The sea as the game has it right now.
    fn waves_now(play: &Play, rows: f32) -> Waves {
        sea(progress_of(play.distance), play.elapsed, play.phase, rows)
    }

    /// The keel row the water asks for under the boat's middle, now.
    fn keel_wanted(play: &Play, rows: f32) -> f32 {
        keel_under(
            &waves_now(play, rows),
            play.boat.centre(),
            play.elapsed,
            level(rows),
        )
    }

    /// A play with no random fireballs, so a test can place its own.
    fn quiet(seed: u64) -> Play {
        let mut play = Play::new(seed, 0);
        play.spawn = Timer::every(1000.0);
        play
    }

    /// The sky cell at column `x`, `y_rows` rows down a sky `height_rows` tall.
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

    /// Draws `play` on a fresh screen and finishes the frame, as the loop does.
    fn frame(play: &Play, width: u32, height: u32) -> Screen {
        let mut screen = Screen::new(width, height);
        screen.clear(Colour::Black);
        play.draw(&mut screen);
        screen.finish_frame();
        screen
    }

    #[test]
    fn the_hop_keys_do_nothing_because_a_sailboat_does_not_jump() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120);
        for code in [KeyCode::Char(' '), KeyCode::Up, KeyCode::Char('w')] {
            rig.press(code);
            rig.update(&mut play);
            rig.release(code);
            let gap = (play.boat.keel - keel_wanted(&play, 24.0)).abs();
            assert!(
                gap < 2.0,
                "{code:?} must not lift the boat: {gap} pixels off"
            );
            rig.run(&mut play, 30);
        }
    }

    #[test]
    fn the_boat_stays_between_a_fifth_and_three_fifths_of_the_width() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        assert!(
            (play.boat.x - 24.0).abs() < 0.01,
            "a third of the way across: {}",
            play.boat.x
        );
        rig.press(KeyCode::Right);
        rig.run(&mut play, 600);
        assert!(
            (play.boat.x - 48.0).abs() < 0.01,
            "stopped at 0.6 of 80: {}",
            play.boat.x
        );
        rig.release(KeyCode::Right);
        rig.press(KeyCode::Left);
        rig.run(&mut play, 600);
        assert!(
            (play.boat.x - 16.0).abs() < 0.01,
            "stopped at 0.2 of 80: {}",
            play.boat.x
        );
    }

    #[test]
    fn the_boat_follows_the_water_once_settled() {
        // at the roughest sea the swell is several pixels high, so a boat that
        // ignored the water would be far off it; the boat must keep up
        let mut play = Play::new(1, 0);
        play.distance = DISTANCE_MAX;
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 300);
        let mut worst = 0.0_f32;
        for _ in 0..600 {
            rig.update(&mut play);
            worst = worst.max((play.boat.keel - keel_wanted(&play, 24.0)).abs());
        }
        // the spring trails the fastest swells by up to two pixels; a boat that
        // ignored the water is more than eight pixels off it at the storm
        assert!(
            worst <= 2.5,
            "the keel was {worst} pixels off the water at its worst"
        );
    }

    #[test]
    fn distance_grows_by_about_one_league_a_second_at_the_start() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 60);
        assert!(
            (0.99..1.03).contains(&play.distance),
            "one league in the first second: {}",
            play.distance
        );
    }

    #[test]
    fn progress_is_zero_at_the_start_grows_with_distance_and_is_clamped() {
        assert_eq!(progress_of(0.0), 0.0);
        assert!((progress_of(DISTANCE_MAX / 2.0) - 0.5).abs() < 1e-6);
        assert_eq!(progress_of(DISTANCE_MAX * 9.0), 1.0, "clamped at the top");
        assert_eq!(progress_of(-4.0), 0.0, "never below zero");
        assert_eq!(progress_of(f32::NAN), 0.0, "NaN counts as zero");
    }

    #[test]
    fn the_swells_shorten_and_grow_steadily_from_calm_to_rough() {
        let mut last = swells(0.0, 0.7);
        for i in 1..=100 {
            let now = swells(i as f32 / 100.0, 0.7);
            for layer in 0..4 {
                assert!(
                    now[layer].1 <= last[layer].1 + 1e-4,
                    "wavelength {layer} at {i}"
                );
                assert!(
                    now[layer].0 >= last[layer].0 - 1e-6,
                    "height {layer} at {i}"
                );
            }
            last = now;
        }
    }

    #[test]
    fn the_calm_sea_has_long_low_swells_and_the_rough_one_short_high_ones() {
        let calm = swells(0.0, 0.0);
        assert!(calm[0].1 >= 35.0, "a long first swell: {}", calm[0].1);
        assert!(calm[0].0 <= 0.6, "a low first swell: {}", calm[0].0);
        let rough = swells(1.0, 0.0);
        assert!(
            rough[0].0 >= 2.0,
            "a high first swell at the end: {}",
            rough[0].0
        );
        assert!(calm[3].0 == 0.0, "the fourth swell starts from nothing");
    }

    #[test]
    fn rebuilding_the_sea_each_tick_never_makes_a_column_jump_more_than_0_15_rows() {
        let phase = 1.3;
        let dt = 1.0 / 60.0;
        let mut distance = 0.0_f32;
        let mut elapsed = 0.0_f32;
        let mut previous = sea(0.0, 0.0, phase, 24.0);
        let mut worst = 0.0_f32;
        for _ in 0..12000 {
            let progress = progress_of(distance);
            elapsed += dt;
            let waves = sea(progress, elapsed, phase, 24.0);
            // the same moment on both sides, so only the rebuild can change the height
            for x in 0..80 {
                let jump =
                    (waves.height(x as f32, elapsed) - previous.height(x as f32, elapsed)).abs();
                worst = worst.max(jump);
            }
            previous = waves;
            distance += sail_speed(progress) * dt;
        }
        assert!(worst <= 0.15, "the sea jumped by {worst} rows in one tick");
    }

    #[test]
    fn fireballs_come_from_above_the_top_fall_and_drift_left() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        play.launch(80.0, 30.0, 0.0);
        let f = *play.fireballs.get(0).expect("one fireball");
        assert!(f.y < 0.0, "it starts above the top: {}", f.y);
        assert!((-10.2..=-3.4).contains(&f.vx), "wind to the left: {}", f.vx);
        rig.run(&mut play, 30);
        let later = *play.fireballs.get(0).expect("still in the air");
        assert!(later.y > f.y, "it falls: {} then {}", f.y, later.y);
        assert!(later.x < f.x, "it drifts left");
    }

    #[test]
    fn fireballs_fall_faster_and_come_quicker_as_the_voyage_goes_on() {
        assert!((spawn_interval(0.0) - 1.8).abs() < 1e-5, "first gap 1.8 s");
        assert!((spawn_interval(1.0) - 0.4).abs() < 1e-5, "last gap 0.4 s");
        let mut last = f32::MAX;
        for i in 0..=100 {
            let gap = spawn_interval(i as f32 / 100.0);
            assert!(gap <= last + 1e-6, "the gap rose at {i}");
            last = gap;
        }
        assert!((fall_scale(0.0) - 1.0).abs() < 1e-5);
        assert!(
            (fall_scale(1.0) - 1.6).abs() < 1e-5,
            "the top speed is 1.6 times the start"
        );
        let mut last_scale = 0.0_f32;
        for i in 0..=100 {
            let scale = fall_scale(i as f32 / 100.0);
            assert!(
                scale >= last_scale - 1e-6,
                "the fall speed fell back at {i}"
            );
            last_scale = scale;
        }
    }

    #[test]
    fn volleys_are_never_at_the_start_and_grow_to_0_35_at_the_end() {
        assert_eq!(volley_chance(0.0), 0.0, "no volleys at the start");
        assert!((volley_chance(1.0) - 0.35).abs() < 1e-5);
    }

    #[test]
    fn a_fireball_that_reaches_the_water_sinks_instead_of_vanishing() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let x = 60.0;
        let water = 2.0 * waves_now(&play, 24.0).surface_y(x, play.elapsed, level(24.0));
        assert!(play
            .fireballs
            .push(Fireball::new(x, water + 0.5, -6.0, 0.0, 0.0))
            .is_ok());
        rig.update(&mut play);
        let f = *play.fireballs.get(0).expect("it is still there");
        assert!(matches!(f.stage, Stage::Sinking(_)), "{:?}", f.stage);
    }

    #[test]
    fn a_sinking_fireball_is_removed_after_1_2_seconds() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        assert!(play
            .fireballs
            .push(Fireball::new(60.0, 40.0, -6.0, 0.0, 0.0))
            .is_ok());
        play.fireballs
            .iter_mut()
            .for_each(|f| f.stage = Stage::Sinking(0.0));
        rig.run(&mut play, 60); // one second in
        assert_eq!(play.fireballs.len(), 1, "still sinking at 1 s");
        rig.run(&mut play, 20); // 1.33 s in
        assert!(play.fireballs.is_empty(), "gone after 1.2 s");
    }

    #[test]
    fn a_sinking_fireball_cannot_hit_the_boat() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let mut f = Fireball::new(boat.centre(), boat.top() + 3.0, 0.0, 0.0, 0.0);
        f.stage = Stage::Sinking(0.0);
        assert!(play.fireballs.push(f).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES, "the sinking one does nothing");
    }

    #[test]
    fn a_sinking_fireball_steps_toward_grey_and_shrinks() {
        let mut last = 0;
        for i in 0..=100 {
            let step = sink_step(i as f32 / 100.0);
            assert!(step >= last, "the colour went back at {i}");
            last = step;
        }
        let young = Fireball {
            stage: Stage::Sinking(0.0),
            ..Fireball::new(0.0, 0.0, 0.0, 0.0, 0.0)
        };
        let old = Fireball {
            stage: Stage::Sinking(SINK_SECONDS * 0.9),
            ..young
        };
        assert!(old.radius() < young.radius(), "it shrinks");
        assert!(old.radius() > 0.0, "and is not gone yet");
        assert!(matches!(sink_colour(0.0), Colour::BrightYellow));
        assert!(matches!(sink_colour(1.0), Colour::Blue));
    }

    #[test]
    fn a_fireball_reaching_the_water_throws_up_a_cloud_of_steam() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let water = 2.0 * waves_now(&play, 24.0).surface_y(60.0, play.elapsed, level(24.0));
        assert!(play
            .fireballs
            .push(Fireball::new(60.0, water + 0.5, -6.0, 0.0, 0.0))
            .is_ok());
        rig.update(&mut play);
        assert!(
            play.steam.len() >= STEAM_BURST,
            "steam: {}",
            play.steam.len()
        );
    }

    #[test]
    fn a_falling_fireball_on_the_boat_hits_it_and_costs_a_life() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let hit = Fireball::new(boat.centre(), boat.top() + 3.0, 0.0, 0.0, 0.0);
        assert!(play.fireballs.push(hit).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 1, "one life lost");
        assert!(play.fireballs.is_empty(), "the one that hit is gone");
        assert_eq!(play.embers.len(), 40, "the explosion");
    }

    #[test]
    fn after_a_hit_a_second_hit_within_1_5_seconds_does_nothing_but_a_later_one_counts() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let hit = |y: f32| Fireball::new(boat.centre(), y, 0.0, 0.0, 0.0);
        assert!(play.fireballs.push(hit(boat.top() + 3.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 1);
        assert!(play.fireballs.push(hit(boat.top() + 3.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(
            play.lives,
            LIVES - 1,
            "invulnerable: the second hit does nothing"
        );
        rig.run(&mut play, 92); // a little over 1.5 seconds after the first hit
        play.fireballs.clear();
        assert!(play.fireballs.push(hit(boat.top() + 3.0)).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES - 2, "vulnerable again");
    }

    #[test]
    fn the_third_hit_ends_the_round_and_the_first_two_do_not() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let mut outcomes = Vec::new();
        for _ in 0..3 {
            let hit = Fireball::new(boat.centre(), boat.top() + 3.0, 0.0, 0.0, 0.0);
            assert!(play.fireballs.push(hit).is_ok());
            outcomes.push(rig.update(&mut play));
            rig.run(&mut play, 92);
            play.fireballs.clear();
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
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        let close = Fireball::new(boat.centre() + 0.3, boat.top() - 4.0, -25.0, 0.0, 0.0);
        assert!(play.fireballs.push(close).is_ok());
        rig.update(&mut play);
        assert_eq!(play.lives, LIVES, "a miss is not a hit");
        assert_eq!(play.bonus, NEAR_BONUS, "the bonus for passing close");
        assert_eq!(play.floaters.len(), 1, "and a +5 floats up");
        rig.run(&mut play, 10);
        assert_eq!(play.bonus, NEAR_BONUS, "no second bonus");
    }

    #[test]
    fn the_score_is_a_point_for_each_tenth_of_a_league_sailed() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 120);
        assert!(
            (19..=20).contains(&play.score()),
            "two leagues: {}",
            play.score()
        );
    }

    #[test]
    fn a_hit_flashes_the_screen_for_0_15_seconds() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        let boat = play.boat;
        assert!(play
            .fireballs
            .push(Fireball::new(
                boat.centre(),
                boat.top() + 3.0,
                0.0,
                0.0,
                0.0
            ))
            .is_ok());
        rig.update(&mut play);
        assert!(play.flashing(), "the hit flashes");
        rig.run(&mut play, 5);
        assert!(play.flashing(), "still flashing");
        rig.run(&mut play, 6);
        assert!(!play.flashing(), "the flash is over");
    }

    #[test]
    fn a_boat_that_is_invulnerable_blinks_on_and_off() {
        assert!(
            boat_visible(0.01, INVULNERABLE_SECONDS),
            "shown in the first slot"
        );
        assert!(
            !boat_visible(0.10, INVULNERABLE_SECONDS),
            "hidden in the next slot"
        );
        assert!(boat_visible(0.17, INVULNERABLE_SECONDS), "and shown again");
        assert!(
            boat_visible(5.0, INVULNERABLE_SECONDS),
            "and always shown once it is over"
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
            play.score() > 0,
            "the script sailed for a while: {hits} hits"
        );
    }

    #[test]
    fn sailing_leaves_a_wake_of_spray_behind_the_stern() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 60);
        assert!(play.spray.len() >= 2, "a wake: {}", play.spray.len());
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
    fn p_pauses_by_pushing_the_pause_screen_before_time_moves() {
        let mut play = Play::new(1, 0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 30);
        let before = (play.elapsed, play.distance);
        rig.press(KeyCode::Char('p'));
        let next = rig.update(&mut play);
        assert!(matches!(next, Next::Push(_)), "p pushes the pause screen");
        assert_eq!(
            (play.elapsed, play.distance),
            before,
            "the pausing update moves nothing"
        );
    }

    #[test]
    fn the_pause_screen_freezes_the_time_of_day_the_distance_and_the_sea() {
        let mut game = Headless::new(Play::new(1, 0)).size(80, 24);
        game.step(300);
        game.tap(KeyCode::Char('p'));
        let frozen = game.text();
        game.step(300);
        assert_eq!(game.text(), frozen, "five seconds of pause changed nothing");
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
            game.step(1);
        }
        game.text()
    }

    #[test]
    fn the_sea_is_rough_only_later_on_the_voyage() {
        let calm = swells(0.0, 0.0);
        let rough = swells(1.0, 0.0);
        let calm_surface: f32 = calm.iter().map(|s| s.0).sum();
        let rough_surface: f32 = rough.iter().map(|s| s.0).sum();
        assert!(
            rough_surface > 3.0 * calm_surface,
            "{calm_surface} then {rough_surface}"
        );
    }

    #[test]
    fn a_day_is_120_seconds_from_dawn_through_noon_dusk_and_midnight() {
        assert!((daylight(0.0) - 0.5).abs() < 1e-5, "dawn");
        assert!((daylight(30.0) - 1.0).abs() < 1e-5, "midday");
        assert!((daylight(60.0) - 0.5).abs() < 1e-5, "dusk");
        assert!(daylight(90.0) < 1e-5, "midnight");
        assert!((daylight(120.0) - 0.5).abs() < 1e-5, "dawn again");
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
    fn the_time_of_day_reads_dawn_day_dusk_and_night() {
        assert_eq!(time_of_day(0.0), "dawn");
        assert_eq!(time_of_day(30.0), "day");
        assert_eq!(time_of_day(60.0), "dusk");
        assert_eq!(time_of_day(90.0), "night");
    }

    #[test]
    fn the_sky_ramp_has_no_blue_or_cyan_because_those_belong_to_the_water() {
        for colour in SKY_RAMP {
            assert!(
                !matches!(
                    colour,
                    Colour::Blue | Colour::BrightBlue | Colour::Cyan | Colour::BrightCyan
                ),
                "{colour:?} is in the sky"
            );
        }
    }

    #[test]
    fn the_sky_is_dark_at_midnight_and_bright_at_midday() {
        let cell = sky_cell(10.0, 2.0, 10.0);
        let midnight = sky_value(&cell, 0.0, 0.0, 0.0);
        let midday = sky_value(&cell, 1.0, 0.0, 0.0);
        assert!(midnight < -0.7, "midnight is dark: {midnight}");
        assert!(midday > 0.7, "midday is bright: {midday}");
    }

    #[test]
    fn midday_sky_is_white_and_midnight_sky_is_black_or_very_dark() {
        let mut screen = Screen::new(40, 12);
        paint_sky(&mut screen, &sky_palette(), 1.0, 0.0, 30.0, 10.0);
        let noon = screen.display().pixel(5, 3).unwrap().background();
        assert!(
            matches!(noon, Colour::White | Colour::BrightWhite),
            "noon: {noon:?}"
        );
        let mut night_screen = Screen::new(40, 12);
        paint_sky(&mut night_screen, &sky_palette(), 0.0, 0.0, 90.0, 10.0);
        let midnight = night_screen.display().pixel(5, 3).unwrap().background();
        assert!(
            matches!(midnight, Colour::Black | Colour::BrightBlack),
            "midnight: {midnight:?}"
        );
    }

    #[test]
    fn the_sky_formula_is_finite_at_every_time_of_day_and_progress() {
        let cell = sky_cell(3.0, 1.0, 10.0);
        for i in 0..30 {
            let t = i as f32 * 4.1;
            let v = sky_value(&cell, daylight(t), 0.5, t);
            assert!(v.is_finite() && (-1.0..=1.0).contains(&v), "{t}: {v}");
            let waves = sea(i as f32 / 29.0, t, 0.4, 24.0);
            let sea_v = sea_value(1.0, 5.0, t, &waves);
            assert!(sea_v.is_finite() && (-1.0..=1.0).contains(&sea_v));
        }
    }

    /// A black screen of `width` by `height`, with nothing drawn yet.
    fn black_screen(width: u32, height: u32) -> Screen {
        let mut screen = Screen::new(width, height);
        screen.clear(Colour::Black);
        screen
    }

    /// How many cells in rows `from..to` show a colour other than black. Only
    /// things drawn on a black screen are counted, so the count is what was drawn.
    fn lit_cells(screen: &Screen, from: i32, to: i32) -> usize {
        let (width, _) = screen.size();
        let display = screen.display();
        let mut count = 0;
        for y in from..to {
            for x in 0..width as isize {
                if let Some(pixel) = display.pixel(x, y as isize) {
                    if pixel.colour() != Colour::Black || pixel.background() != Colour::Black {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    #[test]
    fn stars_show_only_at_night_and_only_in_the_sky() {
        let mut day = black_screen(80, 24);
        paint_stars(&mut day, 0.0, 0.0, 15.0);
        day.finish_frame();
        assert_eq!(lit_cells(&day, 0, 24), 0, "no stars at midday");
        let mut night_screen = black_screen(80, 24);
        paint_stars(&mut night_screen, 1.0, 0.0, 15.0);
        night_screen.finish_frame();
        let stars = lit_cells(&night_screen, 0, 24);
        assert!((5..200).contains(&stars), "a sparse sky of stars: {stars}");
        assert_eq!(
            lit_cells(&night_screen, 8, 24),
            0,
            "none below the sky, which ends at pixel row 15"
        );
    }

    #[test]
    fn the_sun_arcs_across_by_day_and_the_moon_by_night() {
        assert!(sun_arc(30.0).is_some(), "the sun at noon");
        assert!(sun_arc(90.0).is_none(), "no sun at midnight");
        assert!(moon_arc(90.0).is_some(), "the moon at midnight");
        assert!(moon_arc(30.0).is_none(), "no moon at noon");
        let (along, height) = sun_arc(30.0).unwrap();
        assert!(
            (along - 0.5).abs() < 1e-5 && (height - 1.0).abs() < 1e-5,
            "highest at noon"
        );
    }

    /// The cell at the middle of the sky, where the sun is at noon and the moon
    /// at midnight (both are high and in the middle of the arc).
    fn sky_middle(screen: &Screen) -> Colour {
        screen.display().pixel(40, 4).unwrap().background()
    }

    #[test]
    fn the_sun_is_a_yellow_disc_at_noon_and_the_moon_takes_its_place_at_midnight() {
        let mut play = Play::new(1, 0);
        play.elapsed = 30.0;
        let noon = frame(&play, 80, 24);
        assert_eq!(
            sky_middle(&noon),
            Colour::BrightYellow,
            "the sun is drawn at noon"
        );
        play.elapsed = 90.0;
        let midnight = frame(&play, 80, 24);
        assert_eq!(
            sky_middle(&midnight),
            Colour::BrightWhite,
            "the moon, not the sun, at midnight"
        );
    }

    #[test]
    fn the_moon_is_drawn_at_night_and_not_by_day() {
        assert!(moon_arc(90.0).is_some(), "the moon rises at night");
        assert!(moon_arc(30.0).is_none(), "and is down at noon");
        let mut play = Play::new(1, 0);
        play.elapsed = 30.0;
        let noon = frame(&play, 80, 24);
        assert_ne!(sky_middle(&noon), Colour::BrightWhite, "no moon at noon");
    }

    #[test]
    fn the_sun_is_clipped_by_the_water_so_it_never_shows_below_the_surface() {
        let mut screen = black_screen(80, 24);
        let waves = sea(0.0, 0.0, 0.0, 24.0);
        let water = level(24.0);
        // at dawn the sun sits on the horizon, so the waves hide most of it. The
        // glow is painted on cells, so the sea is painted after it, as in a frame
        let look = Look::new();
        paint_celestial(&mut screen, &look, 0.0, &waves, 0.0, water);
        paint_sea(&mut screen, &look.sea, &waves, 0.0, water);
        screen.finish_frame();
        for row in 0..24_i32 {
            for column in 0..80_i32 {
                let both_under = !above_water(&waves, 0.0, water, column, 2 * row)
                    && !above_water(&waves, 0.0, water, column, 2 * row + 1);
                if both_under {
                    let pixel = screen
                        .display()
                        .pixel(column as isize, row as isize)
                        .unwrap();
                    assert_ne!(
                        pixel.background(),
                        Colour::BrightYellow,
                        "the sun under the water at {column},{row}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_boats_picture_is_drawn_on_the_boat_cell() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 60);
        play.elapsed = 30.0;
        let screen = frame(&play, 80, 24);
        let x = play.boat.centre().round() as isize;
        let row = (play.boat.keel / 2.0).round() as isize;
        let boat_colours = [Colour::BrightWhite, Colour::Red, Colour::Yellow];
        let cell = screen.display().pixel(x, row).unwrap();
        assert!(
            boat_colours.contains(&cell.colour()) || boat_colours.contains(&cell.background()),
            "the boat is pixel art at its cell: {:?} {:?}",
            cell.colour(),
            cell.background()
        );
    }

    #[test]
    fn the_sea_near_the_surface_is_brighter_than_in_the_deep() {
        let waves = sea(0.0, 0.0, 0.0, 24.0);
        let surface = waves.surface_y(10.0, 0.0, level(24.0));
        let near = sea_value(0.5, 10.0, 0.0, &waves);
        let deep = sea_value(6.0, 10.0, 0.0, &waves);
        assert!(near > deep + 0.3, "surface {near}, deep {deep}");
        let _ = surface;
    }

    #[test]
    fn the_sea_has_no_noise_so_neighbouring_cells_never_jump() {
        let waves = sea(0.5, 12.0, 0.9, 24.0);
        for x in 0..80 {
            let column = x as f32;
            let mut last = sea_value(1.0, column, 12.0, &waves);
            for step in 1..20 {
                let depth = 1.0 + step as f32;
                let value = sea_value(depth, column, 12.0, &waves);
                // the depth slope is 0.4 a row and the ripple adds at most 0.16 more
                assert!(
                    (value - last).abs() <= 0.6,
                    "a jump of {} at column {x}",
                    value - last
                );
                last = value;
            }
        }
    }

    #[test]
    fn the_island_scrolls_away_as_the_boat_sails() {
        let width = 80.0;
        let start = island_column(0.0, width);
        let later = island_column(4.0, width);
        assert!(later < start, "the island moves left: {start} then {later}");
        assert!(
            island_column(ISLAND_LEAGUES, width) <= -width * 0.9,
            "one screen width after ten leagues"
        );
    }

    #[test]
    fn a_fireball_is_drawn_where_it_is() {
        let mut play = quiet(1);
        assert!(play
            .fireballs
            .push(Fireball::new(30.0, 10.0, -6.0, 0.0, 0.0))
            .is_ok());
        let screen = frame(&play, 80, 24);
        assert_eq!(
            screen.display().pixel(30, 5).unwrap().background(),
            Colour::BrightRed,
            "the disc's fire colour at its cell"
        );
    }

    #[test]
    fn a_sinking_fireball_is_only_grey_or_blue_once_it_has_faded() {
        let mut play = quiet(1);
        let mut f = Fireball::new(30.0, 20.0, 0.0, 0.0, 0.0);
        f.stage = Stage::Sinking(SINK_SECONDS * 0.7);
        assert!(play.fireballs.push(f).is_ok());
        let screen = frame(&play, 80, 24);
        let grey_or_blue = |c: Colour| {
            matches!(
                c,
                Colour::White
                    | Colour::BrightBlack
                    | Colour::Black
                    | Colour::Blue
                    | Colour::BrightWhite
            )
        };
        let cell = screen.display().pixel(30, 10).unwrap();
        assert!(
            grey_or_blue(cell.colour())
                || grey_or_blue(cell.background())
                || cell.background() == Colour::BrightBlack,
            "{:?} {:?}",
            cell.colour(),
            cell.background()
        );
    }

    #[test]
    fn the_hud_shows_the_leagues_the_score_the_best_score_and_the_lives_as_hearts() {
        let mut game = Headless::new(Play::new(1, 7)).size(80, 24);
        game.step(2);
        let text = game.text();
        let top = text.lines().next().unwrap_or("");
        assert!(top.contains("0.0 leagues"), "{top}");
        assert!(top.contains("score 0"), "{top}");
        assert!(top.contains("best 7"), "{top}");
        assert!(top.contains("♥ ♥ ♥"), "{top}");
        assert!(top.contains("dawn"), "the time of day, centred: {top}");
    }

    #[test]
    fn distance_is_shown_as_leagues_with_one_decimal() {
        let mut play = Play::new(1, 0);
        play.distance = 12.4;
        let mut screen = Screen::new(80, 24);
        play.draw(&mut screen);
        let row: String = (0..80)
            .map(|x| screen.display().pixel(x, 0).unwrap().character())
            .collect();
        assert!(row.contains("12.4 leagues"), "{row:?}");
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
    fn enter_on_the_title_starts_a_round_with_no_score() {
        let mut game = Headless::new(Title::new(4)).size(80, 24);
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "a round is on screen: {text}");
        assert!(text.contains("best 4"), "with the best score kept: {text}");
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
    fn the_title_letters_sit_on_a_dark_band_so_they_read_over_the_sky() {
        let mut screen = Screen::new(80, 24);
        Title::new(0).draw(&mut screen);
        let left = (80 - 23) / 2;
        let behind = screen.display().pixel(left - 1, 4).unwrap().background();
        assert_eq!(behind, Colour::Black, "just left of the S");
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
    fn enter_on_the_game_over_screen_sails_again_and_keeps_the_best() {
        let mut game = Headless::new(Over::new(3, 9, 0.0, 0.0, 0.0)).size(80, 24);
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "a fresh round: {text}");
        assert!(text.contains("best 9"), "the best is kept: {text}");
    }

    #[test]
    fn q_quits_from_the_game_over_screen() {
        let mut game = Headless::new(Over::new(3, 9, 0.0, 0.0, 0.0));
        game.tap(KeyCode::Char('q'));
        assert!(game.quit());
    }

    #[test]
    fn the_game_over_screen_shows_the_score_the_best_and_how_to_go_on() {
        let mut game = Headless::new(Over::new(3, 9, 0.0, 0.0, 0.0)).size(80, 24);
        game.step(2);
        let text = game.text();
        assert!(text.contains("score 3"), "{text}");
        assert!(text.contains("best 9"), "{text}");
        assert!(text.contains("Enter: sail again"), "{text}");
    }

    /// Seconds for a fireball to fall from its spawn to the water under column
    /// 32.5 (the boat's middle on an 80 by 24 screen) at `progress`. It is averaged
    /// over three runs of the sea and 200 moments, the same way the baseline was.
    fn seconds_to_the_water(progress: f32) -> f32 {
        let scale = fall_scale(progress);
        let dt = 1.0 / 60.0;
        let (mut total, mut n) = (0.0_f32, 0);
        for phase in [0.0_f32, 2.0, 4.0] {
            for k in 0..200 {
                let t0 = k as f32 * 0.05;
                let mut f = Fireball::new(
                    32.5,
                    FALL_FROM,
                    0.0,
                    FALL_START * scale,
                    FALL_GRAVITY * scale,
                );
                let mut elapsed = 0.0_f32;
                loop {
                    f.fall(dt);
                    elapsed += dt;
                    let t = t0 + elapsed;
                    let surface = sea(progress, t, phase, 24.0).surface_y(32.5, t, level(24.0));
                    if f.y >= 2.0 * surface || elapsed > 10.0 {
                        break;
                    }
                }
                total += elapsed;
                n += 1;
            }
        }
        total / n as f32
    }

    /// The fall to the water before this change, measured on the old sea (level
    /// 0.64, no scaling) with the method of [`seconds_to_the_water`]: 1.667 s at
    /// progress 0 and 1.185 s at progress 1.
    const FALL_SECONDS_CALM_BEFORE: f32 = 1.6672;
    const FALL_SECONDS_STORM_BEFORE: f32 = 1.1847;

    #[test]
    fn fireballs_take_at_least_a_fifth_longer_to_reach_the_water_when_calm_and_15_percent_longer_in_the_storm(
    ) {
        let calm = seconds_to_the_water(0.0);
        let storm = seconds_to_the_water(1.0);
        assert!(
            calm >= 1.2 * FALL_SECONDS_CALM_BEFORE,
            "calm: {calm} s, was {FALL_SECONDS_CALM_BEFORE} s"
        );
        assert!(
            storm >= 1.15 * FALL_SECONDS_STORM_BEFORE,
            "storm: {storm} s, was {FALL_SECONDS_STORM_BEFORE} s"
        );
    }

    /// The boat's top speed in columns a second, held to the right on a screen 400
    /// columns wide so that no edge stops it. Before this change it was 18.
    fn boat_top_speed() -> f32 {
        let mut play = Play::new(1, 0);
        play.spawn = Timer::every(1000.0);
        let mut rig = Rig::new((400, 24));
        rig.run(&mut play, 1);
        rig.press(KeyCode::Right);
        let mut top = 0.0_f32;
        for _ in 0..180 {
            rig.update(&mut play);
            top = top.max(play.boat.vx);
        }
        top
    }

    #[test]
    fn the_boat_sails_about_a_quarter_faster_than_its_old_top_speed_of_18_columns_a_second() {
        let top = boat_top_speed();
        assert!(
            (1.2 * 18.0..=1.4 * 18.0).contains(&top),
            "the top speed is {top} columns a second; it should be 1.2 to 1.4 times 18"
        );
    }

    /// Seconds for the boat to sail from 0.2 to 0.6 of an 80 column screen, held to
    /// the right. It was 2.017 seconds before the boat was made faster.
    fn crossing_seconds() -> f32 {
        let mut play = Play::new(1, 0);
        play.spawn = Timer::every(1000.0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        rig.press(KeyCode::Left);
        rig.run(&mut play, 600);
        rig.release(KeyCode::Left);
        assert!(
            (play.boat.x - 16.0).abs() < 0.01,
            "parked at 0.2: {}",
            play.boat.x
        );
        rig.press(KeyCode::Right);
        let mut ticks = 0;
        while play.boat.x < 48.0 - 1e-3 && ticks < 3000 {
            rig.update(&mut play);
            ticks += 1;
        }
        ticks as f32 / 60.0
    }

    #[test]
    fn the_boat_crosses_the_middle_of_the_screen_about_a_fifth_sooner() {
        let seconds = crossing_seconds();
        assert!(
            seconds <= 0.85 * 2.017,
            "the crossing took {seconds} s; it took 2.017 s before"
        );
    }

    #[test]
    fn the_title_controls_sit_in_a_framed_panel_like_the_other_screens() {
        let mut game = Headless::new(Title::new(0)).size(80, 24);
        game.step(3);
        let text = game.text();
        assert_eq!(text.matches('\u{25C6}').count(), 4, "four corners:\n{text}");
        assert!(
            text.contains("A/D or arrows steer   p pause   q quit"),
            "{text}"
        );
    }

    #[test]
    fn the_still_water_sits_low_at_seventy_two_hundredths_of_the_screen() {
        assert!((level(100.0) - 72.0).abs() < 1e-4, "{}", level(100.0));
    }

    #[test]
    fn the_stormy_troughs_stay_a_row_and_a_half_above_the_bottom_on_short_screens() {
        for rows in [20.0_f32, 24.0, 40.0] {
            for phase in [0.0_f32, 1.3] {
                for k in 0..600 {
                    let t = k as f32 / 10.0;
                    let waves = sea(1.0, t, phase, rows);
                    for x in 0..100 {
                        let y = waves.surface_y(x as f32, t, level(rows));
                        assert!(
                            y <= rows - 1.5 + 1e-3,
                            "{rows} rows: column {x} at {t} s is at row {y}"
                        );
                    }
                }
            }
        }
    }

    /// The column where `needle` starts in `line`, counted in characters.
    fn column_of(line: &str, needle: &str) -> usize {
        let byte = line.find(needle).expect("the text is on this line");
        line[..byte].chars().count()
    }

    #[test]
    fn the_game_over_text_sits_in_a_framed_panel_with_diamond_corners() {
        let mut game = Headless::new(Over::new(3, 9, 0.0, 0.0, 0.0)).size(80, 24);
        game.step(2);
        let text = game.text();
        let lines: Vec<&str> = text.lines().collect();
        let corner_rows: Vec<usize> = (0..lines.len())
            .filter(|&row| lines[row].contains('\u{25C6}'))
            .collect();
        assert_eq!(corner_rows.len(), 2, "a top and a bottom edge:\n{text}");
        let (top, bottom) = (corner_rows[0], corner_rows[1]);
        let left = column_of(lines[top], "\u{25C6}");
        let right = lines[top]
            .rfind('\u{25C6}')
            .map(|b| lines[top][..b].chars().count());
        let right = right.expect("a right corner");
        assert!(
            lines[top].contains('\u{2550}'),
            "a double line along the top:\n{text}"
        );
        for line in &lines[top + 1..bottom] {
            assert!(line.contains('\u{2551}'), "the sides:\n{text}");
        }
        for word in [
            "the sea wins this time",
            "score 3",
            "best 9",
            "Enter: sail again   q: quit",
        ] {
            let row = lines.iter().position(|l| l.contains(word)).expect(word);
            assert!(
                row > top && row < bottom,
                "{word} is inside the panel:\n{text}"
            );
            let start = column_of(lines[row], word);
            assert!(
                start > left && start + word.chars().count() < right,
                "{word} is inside the panel's sides:\n{text}"
            );
        }
    }

    /// The top left and bottom right diamonds of the panel on `screen`, as cells.
    fn panel_box(screen: &Screen) -> ((isize, isize), (isize, isize)) {
        let (width, height) = screen.size();
        let mut corners = Vec::new();
        for y in 0..height as isize {
            for x in 0..width as isize {
                if screen.display().pixel(x, y).unwrap().character() == '\u{25C6}' {
                    corners.push((x, y));
                }
            }
        }
        (corners[0], corners[corners.len() - 1])
    }

    #[test]
    fn nothing_shows_through_the_game_over_panel_with_the_moon_and_stars_behind_it() {
        let mut game = Headless::new(Over::new(3, 9, 90.0, 1.0, 0.4)).size(80, 24);
        game.step(2);
        let screen = game.screen();
        let ((x0, y0), (x1, y1)) = panel_box(screen);
        for y in y0 + 1..y1 {
            for x in x0 + 1..x1 {
                let cell = screen.display().pixel(x, y).unwrap();
                assert_eq!(cell.background(), PANEL_INSIDE, "({x}, {y}) is the panel's");
                assert!(
                    !matches!(
                        cell.character(),
                        '\u{2580}' | '\u{2584}' | '\u{2588}' | '\u{2591}' | '\u{2592}' | '\u{2593}'
                    ),
                    "a picture shows through at ({x}, {y}): {:?}",
                    cell.character()
                );
            }
        }
    }

    #[test]
    fn both_panels_are_whole_on_every_screen_from_60_by_18_to_200_by_50() {
        for width in (60..=200).step_by(5) {
            for height in 18..=50 {
                let mut over =
                    Headless::new(Over::new(123_456, 999_999, 90.0, 0.5, 0.4)).size(width, height);
                over.step(2);
                let text = over.text();
                assert_eq!(
                    text.matches('\u{25C6}').count(),
                    4,
                    "the game-over panel's four corners at {width}x{height}:\n{text}"
                );
                assert!(text.contains("Enter: sail again"), "{width}x{height}");

                let mut paused = Headless::new(Play::new(1, 0)).size(width, height);
                paused.step(2);
                paused.tap(KeyCode::Char('p'));
                let text = paused.text();
                assert_eq!(
                    text.matches('\u{25C6}').count(),
                    4,
                    "the pause panel's four corners at {width}x{height}:\n{text}"
                );
                assert!(text.contains("PAUSED  (p to resume)"), "{width}x{height}");
            }
        }
    }

    #[test]
    fn no_black_gap_shows_in_the_sky_above_a_trough_at_noon_in_the_roughest_sea() {
        // the troughs swell and die down, so look at a stretch of the bright middle of the day
        let mut play = Play::new(1, 0);
        play.distance = DISTANCE_MAX;
        let water = level(24.0);
        for k in 0..80 {
            let t = 20.0 + k as f32 * 0.25;
            play.elapsed = t;
            let screen = frame(&play, 80, 24);
            let waves = sea(1.0, t, play.phase, 24.0);
            for x in 0..80 {
                let surface = waves.surface_y(x as f32, t, water);
                for row in 1..24 {
                    if (row as f32) + 1.0 < surface - 0.5 {
                        let bg = screen.display().pixel(x, row).unwrap().background();
                        assert_ne!(bg, Colour::Black, "a gap at {t} s, column {x}, row {row}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_tall_screen_keeps_the_full_swell_and_a_short_one_scales_it_down() {
        assert_eq!(swell_fit(1.0, 40.0), 1.0, "40 rows: the full storm");
        assert!(swell_fit(1.0, 24.0) < 1.0, "24 rows: scaled a little");
        assert!(swell_fit(1.0, 20.0) < 1.0, "20 rows: scaled down");
        assert_eq!(swell_fit(0.0, 20.0), 1.0, "the calm sea always fits");
    }

    /// The cells of the screen as (character, colour, background), row by row.
    fn cells(game: &mut Headless) -> Vec<(char, Colour, Colour)> {
        cells_of(game.screen())
    }

    /// The cells of `screen` as (character, colour, background), row by row.
    fn cells_of(screen: &Screen) -> Vec<(char, Colour, Colour)> {
        let (width, height) = screen.size();
        let display = screen.display();
        let mut out = Vec::new();
        for y in 0..height as isize {
            for x in 0..width as isize {
                let pixel = display.pixel(x, y).expect("the cell is on screen");
                out.push((pixel.character(), pixel.colour(), pixel.background()));
            }
        }
        out
    }

    #[test]
    fn the_tuning_has_the_three_knobs_at_their_defaults() {
        let knobs = tuning().knobs();
        let names: Vec<&str> = knobs.iter().map(|k| k.name).collect();
        assert_eq!(names, ["fireball_rate", "wave_height", "boat_speed"]);
        assert!(knobs.iter().all(|k| k.default == 1.0), "{knobs:?}");
    }

    #[test]
    fn with_the_default_tuning_a_round_looks_exactly_as_it_did_before() {
        let mut plain = Headless::new(Title::new(0)).seed(1).size(80, 24);
        let mut tuned = Headless::new(start(tuning())).seed(1).size(80, 24);
        plain.tap(KeyCode::Enter);
        tuned.tap(KeyCode::Enter);
        plain.step(600);
        tuned.step(600);
        assert_eq!(tuned.text(), plain.text());
        assert_eq!(cells(&mut tuned), cells(&mut plain), "the colours too");
    }

    /// Paints `game` on a fresh screen and finishes the frame, as the loop does.
    fn painted(game: &impl Game, width: u32, height: u32) -> Screen {
        let mut screen = Screen::new(width, height);
        screen.clear(Colour::Black);
        game.draw(&mut screen);
        screen.finish_frame();
        screen
    }

    /// Whether the deepest rows of the sea look the same in every column.
    fn deep_water_is_flat(screen: &Screen) -> bool {
        let display = screen.display();
        let cell = |x: isize, y: isize| {
            display
                .pixel(x, y)
                .map(|p| (p.character(), p.colour(), p.background()))
        };
        (20..24).all(|y| (0..80).all(|x| cell(x, y) == cell(0, y)))
    }

    #[test]
    fn the_fireball_rate_sends_more_fireballs_when_it_is_higher() {
        fn in_the_air_after_900_ticks(rate: f32) -> usize {
            let knobs = tuning();
            knobs.set("fireball_rate", rate);
            let mut play = Play::new(1, 0).tuned(knobs);
            let mut rig = Rig::new((80, 24));
            rig.run(&mut play, 900);
            play.fireballs.len()
        }
        let high = in_the_air_after_900_ticks(3.0);
        let low = in_the_air_after_900_ticks(0.2);
        assert!(high > low, "at 3x there were {high}, at 0.2x {low}");
    }

    #[test]
    fn a_wave_height_of_nothing_makes_the_sea_flat() {
        let flat = sea_scaled(1.0, 3.7, 0.4, 24.0, 0.0);
        for x in 0..80 {
            assert_eq!(flat.height(x as f32, 3.7), 0.0, "column {x}");
        }
    }

    #[test]
    fn a_wave_height_of_two_raises_the_biggest_swell() {
        let biggest = |scale: f32| {
            let mut most = 0.0_f32;
            for i in 0..200 {
                let t = i as f32 * 0.1;
                let waves = sea_scaled(1.0, t, 0.4, 24.0, scale);
                for x in 0..80 {
                    most = most.max(waves.height(x as f32, t).abs());
                }
            }
            most
        };
        let normal = biggest(1.0);
        let doubled = biggest(2.0);
        assert!(doubled > 1.9 * normal, "{doubled} against {normal}");
    }

    #[test]
    fn the_round_reads_the_wave_height_on_every_update() {
        let knobs = tuning();
        knobs.set("wave_height", 0.0);
        let mut play = Play::new(1, 0).tuned(knobs.clone());
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 1);
        assert_eq!(play.wave_scale, 0.0);
        knobs.set("wave_height", 2.0);
        rig.run(&mut play, 1);
        assert_eq!(play.wave_scale, 2.0);
    }

    #[test]
    fn with_no_wave_height_the_water_is_drawn_flat_in_every_column() {
        let knobs = tuning();
        knobs.set("wave_height", 0.0);
        let mut play = Play::new(1, 0).tuned(knobs);
        play.spawn = Timer::every(1000.0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 300);
        assert!(deep_water_is_flat(&painted(&play, 80, 24)), "flat when 0");
    }

    #[test]
    fn the_drawn_sea_follows_the_wave_height_the_round_last_read() {
        let mut play = quiet(1);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 300);
        play.wave_scale = 0.0;
        let flat = cells_of(&painted(&play, 80, 24));
        play.wave_scale = 2.0;
        let high = cells_of(&painted(&play, 80, 24));
        assert_ne!(flat, high, "the picture ignored the wave height");
    }

    #[test]
    fn the_game_over_sea_keeps_the_flat_water_it_ended_with() {
        let over = Over::new(3, 9, 0.0, 0.0, 0.0).waves_at(0.0);
        assert!(deep_water_is_flat(&painted(&over, 80, 24)));
    }

    #[test]
    fn boat_speed_two_sails_farther_in_thirty_steps_than_one() {
        fn sailed_in_thirty_steps(speed: f32) -> f32 {
            let knobs = tuning();
            knobs.set("boat_speed", speed);
            let mut play = Play::new(1, 0).tuned(knobs);
            play.spawn = Timer::every(1000.0);
            let mut rig = Rig::new((80, 24));
            rig.run(&mut play, 1);
            let before = play.boat.x;
            rig.press(KeyCode::Right);
            rig.run(&mut play, 30);
            play.boat.x - before
        }
        let normal = sailed_in_thirty_steps(1.0);
        let fast = sailed_in_thirty_steps(2.0);
        assert!(fast > 1.5 * normal, "at 2x {fast} columns, at 1x {normal}");
    }

    #[test]
    fn a_knob_moved_during_a_round_changes_that_round_at_once() {
        fn sailed_in_the_last_thirty(change_to: Option<f32>) -> f32 {
            let knobs = tuning();
            let mut play = Play::new(1, 0).tuned(knobs.clone());
            play.spawn = Timer::every(1000.0);
            let mut rig = Rig::new((400, 24));
            rig.run(&mut play, 1);
            rig.press(KeyCode::Right);
            rig.run(&mut play, 60);
            if let Some(speed) = change_to {
                knobs.set("boat_speed", speed);
            }
            let before = play.boat.x;
            rig.run(&mut play, 30);
            play.boat.x - before
        }
        let unchanged = sailed_in_the_last_thirty(None);
        let changed = sailed_in_the_last_thirty(Some(2.0));
        assert!(
            changed > 1.3 * unchanged,
            "after the change {changed}, without it {unchanged}"
        );
    }

    #[test]
    fn the_title_hands_its_tuning_on_to_the_round_it_starts() {
        let knobs = tuning();
        let mut game = Headless::new(start(knobs.clone())).seed(1).size(80, 24);
        game.tap(KeyCode::Enter);
        knobs.set("wave_height", 0.0);
        game.step(120);
        let mut control = Headless::new(start(tuning())).seed(1).size(80, 24);
        control.tap(KeyCode::Enter);
        control.step(120);
        assert_ne!(
            game.text(),
            control.text(),
            "the round saw the new wave height"
        );
    }

    #[test]
    fn the_game_over_screen_hands_its_tuning_on_to_the_next_round() {
        let knobs = tuning();
        let over = Over::new(3, 9, 0.0, 0.0, 0.0).tuned(knobs.clone());
        let mut game = Headless::new(over).seed(1).size(80, 24);
        game.tap(KeyCode::Enter);
        knobs.set("wave_height", 0.0);
        game.step(120);
        let over = Over::new(3, 9, 0.0, 0.0, 0.0).tuned(tuning());
        let mut control = Headless::new(over).seed(1).size(80, 24);
        control.tap(KeyCode::Enter);
        control.step(120);
        assert_ne!(
            game.text(),
            control.text(),
            "the next round saw the new wave height"
        );
    }

    #[test]
    fn with_no_wave_height_the_boat_settles_on_the_flat_water_that_is_drawn() {
        let knobs = tuning();
        knobs.set("wave_height", 0.0);
        let mut play = Play::new(1, 0).tuned(knobs);
        play.distance = DISTANCE_MAX;
        play.spawn = Timer::every(1000.0);
        let mut rig = Rig::new((80, 24));
        rig.run(&mut play, 600);
        let flat_keel = 2.0 * level(24.0) + 1.0;
        assert!(
            (play.boat.keel - flat_keel).abs() < 0.5,
            "keel {} should sit at the flat water {flat_keel}",
            play.boat.keel
        );
    }

    #[test]
    fn a_settled_boat_keeps_its_keel_on_the_screen_in_the_roughest_sea() {
        for rows in [20_u32, 24, 40] {
            let mut play = Play::new(1, 0);
            play.distance = DISTANCE_MAX;
            play.spawn = Timer::every(1000.0);
            let mut rig = Rig::new((80, rows));
            rig.run(&mut play, 600);
            // fifty seconds of sailing to and fro, so the boat meets the troughs of many columns
            for tick in 0..3000 {
                if tick % 240 == 0 {
                    let (off, on) = if (tick / 240) % 2 == 0 {
                        (KeyCode::Left, KeyCode::Right)
                    } else {
                        (KeyCode::Right, KeyCode::Left)
                    };
                    rig.release(off);
                    rig.press(on);
                }
                rig.update(&mut play);
                assert!(
                    play.boat.keel < 2.0 * rows as f32,
                    "{rows} rows: the keel is at pixel {} of {}",
                    play.boat.keel,
                    2 * rows
                );
            }
        }
    }
}

//! GLOW: a small light drifts through a living plasma sky. Touch a glowing
//! mote and it bursts: your score goes up and a ring of light rolls out across
//! the sky, which ripples with it. You have 45 seconds.
//!
//! Arrows or WASD steer, hold the left mouse button to be pulled toward the
//! pointer, p pauses, q quits.

use std::f32::consts::TAU;
use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(start(tuning()))
}

/// Seconds in a round.
const ROUND_SECONDS: f32 = 45.0;
/// Columns a second. Rows move half as fast, because a cell is twice as tall as it is wide.
const SPEED: f32 = 24.0;
/// How hard a held mouse button pulls the light, in columns a second.
const PULL: f32 = 20.0;
/// The highest the light may go (see `steer`): below the HUD strip on row 0.
const TOP_ROW: f32 = 2.0;
/// How quickly the velocity eases toward what the keys and pointer ask for.
const EASE: f32 = 4.0;

/// The light is a disc of `ORB_RADIUS` pixels with a soft halo of `HALO_RADIUS`
/// pixels, which breathes by a tenth of its size. A mote is a smaller disc,
/// `MOTE_DISC` pixels, with a warm halo of `MOTE_HALO` pixels that twinkles.
/// Pixels are square, so these are round on screen (see `Screen::disc`).
const ORB_RADIUS: f32 = 2.5;
const HALO_RADIUS: f32 = 8.0;
const MOTE_DISC: f32 = 1.6;
const MOTE_HALO: f32 = 6.0;

/// The colours of a collect burst, dark to bright.
fn ember_palette() -> Palette {
    use Colour::*;
    Palette::new(&[Yellow, BrightYellow, BrightWhite])
}

/// The warm colours of a mote's halo, dark to bright.
fn mote_palette() -> Palette {
    use Colour::*;
    Palette::new(&[Yellow, BrightYellow])
}

/// The most motes, rings and sparks at once. The pools never grow past these.
const MOTE_LIMIT: usize = 24;
const PULSE_LIMIT: usize = 8;
const SPARK_LIMIT: usize = 160;
/// The sparks of a collect burst: how many can be alive, how many a touch
/// makes, how fast they fly (cells a second) and how long they last.
const EMBER_LIMIT: usize = 200;
const EMBER_COUNT: usize = 14;
const EMBER_SPEED: f32 = 12.0;
const EMBER_LIFE: f32 = 0.6;
/// How long a ring rolls, and how long a spark glows, in seconds.
const PULSE_LIFE: f32 = 1.6;
const SPARK_LIFE: f32 = 0.6;
/// How fast a ring's front moves, and the radii of the light and the motes,
/// all in square units (see `disc`).
const RING_SPEED: f32 = 14.0;
const LIGHT_RADIUS: f32 = 1.2;
const MOTE_RADIUS: f32 = 1.0;

/// The colour of a trail spark with `fade` of its life left (1.0 newborn, 0.0
/// spent): it steps down the palette, from bright to dark, as it ages.
fn fade_colour(palette: &Palette, fade: f32) -> Colour {
    let colours = palette.colours();
    let steps = colours.len() - 1;
    let fade = if fade.is_nan() {
        0.0
    } else {
        fade.clamp(0.0, 1.0)
    };
    colours[((fade * steps as f32).round() as usize).min(steps)]
}

/// The colours of the light's halo, dark to bright.
fn light_palette() -> Palette {
    use Colour::*;
    Palette::new(&[Cyan, BrightCyan, BrightWhite])
}

/// The colours of the trail, from newborn (last) to spent (first).
fn trail_palette() -> Palette {
    use Colour::*;
    Palette::new(&[Blue, Cyan, BrightCyan, BrightWhite])
}

/// What the keys can ask for during play.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Left,
    Right,
    Up,
    Down,
    Pause,
}

/// The player's light, in cells: `x` is the column and `y` the row.
#[derive(Clone, Copy, Debug, Default)]
struct Light {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// A glowing mote drifting across the sky.
#[derive(Clone, Copy)]
struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
}

/// A ring of light that rolls out from where a mote was touched.
#[derive(Clone, Copy)]
struct Pulse {
    x: f32,
    y: f32,
    age: f32,
}

/// A spark of the trail behind the light.
#[derive(Clone, Copy)]
struct Spark {
    x: f32,
    y: f32,
    life: f32,
}

/// The middle of the cell at `(x, y)`, in pixel units: a cell is one column
/// wide and two pixel rows tall, and a pixel's centre is half a unit in.
fn centre(x: f32, y: f32) -> (f32, f32) {
    (x + 0.5, 2.0 * y + 1.0)
}

/// A circle at cell `(x, y)` in square units. A column is half as wide as a
/// row is tall, so columns are scaled by one half when measuring distance.
fn disc(x: f32, y: f32, radius: f32) -> Hit {
    Hit::circle(x * 0.5, y, radius)
}

fn controls() -> Controls<Action> {
    Controls::new()
        .bind(Action::Left, &[KeyCode::Left, KeyCode::Char('a')])
        .bind(Action::Right, &[KeyCode::Right, KeyCode::Char('d')])
        .bind(Action::Up, &[KeyCode::Up, KeyCode::Char('w')])
        .bind(Action::Down, &[KeyCode::Down, KeyCode::Char('s')])
        .bind(Action::Pause, &[KeyCode::Char('p'), KeyCode::Esc])
}

/// The numbers the website lets you change while the game runs.
pub fn tuning() -> Tuning {
    Tuning::new()
        .knob("mote_rate", "motes per second (x)", 1.0, 0.25, 4.0)
        .knob(
            "round_length",
            "round length (seconds)",
            ROUND_SECONDS,
            10.0,
            120.0,
        )
}

/// The game with the website's knobs: the title screen, then a round that
/// reads `tuning` while it plays.
pub fn start(tuning: Tuning) -> impl Game {
    Opening {
        title: Title,
        tuning,
    }
}

/// The title screen of a game that reads `tuning`.
struct Opening {
    title: Title,
    tuning: Tuning,
}

impl Game for Opening {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed).with(self.tuning.clone()))
        } else {
            self.title.update(ctx)
        }
    }

    fn draw(&self, screen: &mut Screen) {
        self.title.draw(screen);
    }
}

/// The first screen: the sky, the name and how to start.
struct Title;

impl Game for Title {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        draw_sky(screen, &Pool::with_capacity(0));
        let (middle, width) = (screen.height() / 2, screen.width());
        // a dark band behind the word; one colour per letter, cycling with time,
        // and no spaces between the letters so the word stays whole
        screen.fill(0, middle - 1, width, 3, ' ', Colour::White, Colour::Black);
        let colours = [
            Colour::BrightRed,
            Colour::BrightYellow,
            Colour::BrightGreen,
            Colour::BrightCyan,
        ];
        let x = (width - 4) / 2;
        for (i, ch) in "GLOW".chars().enumerate() {
            let colour = colours[(i + (screen.time() * 3.0) as usize) % colours.len()];
            screen.put_on(x + i as i32, middle, ch, colour, Colour::Black);
        }
        plate(
            screen,
            middle + 2,
            "press Enter to play",
            Colour::BrightWhite,
        );
        plate(
            screen,
            middle + 3,
            "arrows or WASD or hold the mouse   p pause   q quit",
            Colour::White,
        );
    }
}

/// The game itself.
struct Play {
    score: u32,
    light: Light,
    motes: Pool<Mote>,
    pulses: Pool<Pulse>,
    sparks: Pool<Spark>,
    /// The sparks of a collect burst.
    embers: Particles,
    spawn: Timer,
    clock: Timer,
    rng: Rng,
    controls: Controls<Action>,
    /// The palettes of the light's halo, the trail and the motes' halos, built
    /// once rather than per frame.
    light_glow: Palette,
    trail: Palette,
    mote_glow: Palette,
    /// The website's knobs; an empty set gives today's round.
    tuning: Tuning,
}

impl Play {
    fn new(seed: u64) -> Play {
        Play {
            tuning: Tuning::new(),
            score: 0,
            light: Light::default(),
            motes: Pool::with_capacity(MOTE_LIMIT),
            pulses: Pool::with_capacity(PULSE_LIMIT),
            sparks: Pool::with_capacity(SPARK_LIMIT),
            embers: Particles::new(EMBER_LIMIT).palette(ember_palette()),
            spawn: Timer::every(0.5),
            clock: Timer::once(ROUND_SECONDS),
            rng: Rng::new(seed),
            controls: controls(),
            light_glow: light_palette(),
            trail: trail_palette(),
            mote_glow: mote_palette(),
        }
    }

    /// The same round, reading `tuning` for its knobs.
    fn with(mut self, tuning: Tuning) -> Play {
        self.tuning = tuning;
        self
    }

    /// Eases the velocity toward what the keys ask for (and the pointer, while
    /// its button is held), then moves the light and keeps it on the screen.
    fn steer(&mut self, ctx: &Context<'_>) {
        let across = self.controls.axis(ctx.keys, Action::Left, Action::Right) as f32;
        let down = self.controls.axis(ctx.keys, Action::Up, Action::Down) as f32;
        let (mut wanted_x, mut wanted_y) = (across * SPEED, down * SPEED * 0.5);
        if ctx.pointer.left {
            wanted_x += (ctx.pointer.x - self.light.x).clamp(-PULL, PULL);
            wanted_y += (ctx.pointer.y - self.light.y).clamp(-PULL, PULL) * 0.5;
        }
        let ease = (ctx.dt * EASE).min(1.0);
        let light = &mut self.light;
        light.vx += (wanted_x - light.vx) * ease;
        light.vy += (wanted_y - light.vy) * ease;
        // min then max, not clamp: clamp panics when the screen is empty
        let (right, bottom) = (ctx.size.0 as f32 - 1.0, ctx.size.1 as f32 - 1.0);
        light.x = (light.x + light.vx * ctx.dt).min(right).max(0.0);
        // the lowest row the light may be on: its orb reaches 2.5 pixels (1.25
        // rows) from its middle, so row 1 would still cover the score in row 0
        light.y = (light.y + light.vy * ctx.dt).min(bottom).max(TOP_ROW);
    }

    /// Moves the motes, with a slow sine wobble, and drops the ones that have
    /// drifted off the screen. The margin lets new ones enter from outside.
    fn drift(&mut self, ctx: &Context<'_>) {
        let (right, bottom) = (ctx.size.0 as f32 + 3.0, ctx.size.1 as f32 + 3.0);
        for mote in self.motes.iter_mut() {
            mote.x += mote.vx * ctx.dt;
            mote.y += (mote.vy + (ctx.time() * 1.5 + mote.phase).sin() * 0.5) * ctx.dt;
        }
        self.motes
            .retain(|m| m.x > -3.0 && m.x < right && m.y > -3.0 && m.y < bottom);
    }

    /// Puts a mote just outside a random edge, drifting in across the sky.
    fn spawn_mote(&mut self, size: (u32, u32)) {
        let (width, height) = (size.0 as f32, size.1 as f32);
        let rng = &mut self.rng;
        let speed = rng.range_f32(3.0, 6.0);
        let (x, y, vx, vy) = match rng.range_i32(0, 4) {
            0 => (-1.0, rng.range_f32(0.0, height), speed, 0.0),
            1 => (width, rng.range_f32(0.0, height), -speed, 0.0),
            2 => (rng.range_f32(0.0, width), -1.0, 0.0, 1.0),
            _ => (rng.range_f32(0.0, width), height, 0.0, -1.0),
        };
        let phase = rng.range_f32(0.0, TAU);
        let _ = self.motes.push(Mote {
            x,
            y,
            vx,
            vy,
            phase,
        });
    }

    /// Removes the motes the light touches: one point each, and a ring starts there.
    fn collect(&mut self) {
        let light = disc(self.light.x, self.light.y, LIGHT_RADIUS);
        let (score, pulses, embers, rng) = (
            &mut self.score,
            &mut self.pulses,
            &mut self.embers,
            &mut self.rng,
        );
        self.motes.retain(|mote| {
            let touched = light.overlaps(&disc(mote.x, mote.y, MOTE_RADIUS));
            if touched {
                *score += 1;
                let _ = pulses.push(Pulse {
                    x: mote.x,
                    y: mote.y,
                    age: 0.0,
                });
                embers.burst(rng, mote.x, mote.y, EMBER_COUNT, EMBER_SPEED, EMBER_LIFE);
            }
            !touched
        });
    }

    /// Leaves a spark where the light is, ages them all, and drops the spent ones.
    fn trail(&mut self, dt: f32) {
        let x = self.light.x + self.rng.range_f32(-0.6, 0.6);
        let _ = self.sparks.push(Spark {
            x,
            y: self.light.y,
            life: SPARK_LIFE,
        });
        for spark in self.sparks.iter_mut() {
            spark.life -= dt;
        }
        self.sparks.retain(|spark| spark.life > 0.0);
    }
}

impl Game for Play {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if self.controls.pressed(ctx.keys, Action::Pause) {
            return Next::push(Paused);
        }
        if ctx.keys.pressed(KeyCode::Char('q')) {
            return Next::quit();
        }
        if self.clock.fraction() == 0.0 {
            // the first update: start in the middle of the screen
            self.light.x = ctx.size.0 as f32 / 2.0;
            self.light.y = ctx.size.1 as f32 / 2.0;
        }
        // the round's length is live: a new length keeps the time already played
        self.clock.set_seconds(self.round_seconds());
        if self.clock.tick(ctx.dt) {
            return Next::switch(Ended {
                over: Over { score: self.score },
                tuning: self.tuning.clone(),
            });
        }
        self.steer(ctx);
        self.drift(ctx);
        // the spawn clock is every half second; the knob runs it faster
        if self.spawn.tick(ctx.dt * self.tuning.get("mote_rate", 1.0)) {
            self.spawn_mote(ctx.size);
        }
        self.collect();
        for pulse in self.pulses.iter_mut() {
            pulse.age += ctx.dt;
        }
        self.pulses.retain(|pulse| pulse.age < PULSE_LIFE);
        self.trail(ctx.dt);
        self.embers.step(ctx.dt);
        Next::stay()
    }

    fn draw(&self, screen: &mut Screen) {
        draw_sky(screen, &self.pulses);
        self.draw_halos(screen);
        hud(
            screen,
            self.score,
            self.clock.remaining(),
            self.round_seconds(),
        );
        self.draw_shapes(screen);
    }
}

impl Play {
    /// The round's length in seconds, from the website's knob (45 by default).
    fn round_seconds(&self) -> f32 {
        self.tuning.get("round_length", ROUND_SECONDS)
    }

    /// Whether the first update has put the light on the screen. Until then
    /// the light is at the corner, and a frame drawn now must not show it.
    fn placed(&self) -> bool {
        self.clock.fraction() > 0.0
    }

    /// The soft lights, which paint the cells they cover: the light's halo, and
    /// a warm halo around each mote that twinkles by changing size.
    fn draw_halos(&self, screen: &mut Screen) {
        let time = screen.time();
        if self.placed() {
            let (x, y) = centre(self.light.x, self.light.y);
            let breath = 1.0 + 0.1 * (time * 3.0).sin();
            screen.glow(x, y, HALO_RADIUS * breath, &self.light_glow);
        }
        for mote in self.motes.iter() {
            let (x, y) = centre(mote.x, mote.y);
            let twinkle = 1.0 + 0.15 * (time * 5.0 + mote.phase).sin();
            screen.glow(x, y, MOTE_HALO * twinkle, &self.mote_glow);
        }
    }

    /// The pixel shapes, drawn last so they show above the HUD: the trail, the
    /// motes and the light's orb.
    fn draw_shapes(&self, screen: &mut Screen) {
        for spark in self.sparks.iter() {
            let (x, y) = centre(spark.x, spark.y);
            let colour = fade_colour(&self.trail, spark.life / SPARK_LIFE);
            screen.pixel(x.floor() as i32, y.floor() as i32, colour);
        }
        for mote in self.motes.iter() {
            let (x, y) = centre(mote.x, mote.y);
            screen.disc(x, y, MOTE_DISC, Colour::BrightYellow);
        }
        if self.placed() {
            let (x, y) = centre(self.light.x, self.light.y);
            screen.disc(x, y, ORB_RADIUS, Colour::BrightWhite);
        }
        self.embers.draw(screen);
    }
}

/// The score and the time left, on a black strip along the top.
fn hud(screen: &mut Screen, score: u32, seconds_left: f32, round_seconds: f32) {
    let width = screen.width();
    screen.fill(0, 0, width, 1, ' ', Colour::White, Colour::Black);
    screen.text(1, 0, &format!("score {score}"), Colour::BrightYellow);
    screen.text(width - 31, 0, &format!("{seconds_left:.1}s"), Colour::White);
    let fraction = seconds_left / round_seconds;
    screen.bar(
        width - 24,
        0,
        20,
        fraction,
        Colour::BrightWhite,
        Colour::BrightBlack,
    );
}

/// The game-over screen, shown when the round's time runs out.
struct Over {
    score: u32,
}

impl Game for Over {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed.wrapping_add(ctx.tick)))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        draw_sky(screen, &Pool::with_capacity(0));
        let middle = screen.height() / 2;
        plate(screen, middle - 1, "TIME!", Colour::BrightYellow);
        plate(
            screen,
            middle,
            &format!("score {}", self.score),
            Colour::BrightWhite,
        );
        plate(screen, middle + 2, "Enter: again   q: quit", Colour::White);
    }
}

/// The game-over screen of a game that reads `tuning`: Enter starts a round
/// that reads it too.
struct Ended {
    over: Over,
    tuning: Tuning,
}

impl Game for Ended {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed.wrapping_add(ctx.tick)).with(self.tuning.clone()))
        } else {
            self.over.update(ctx)
        }
    }

    fn draw(&self, screen: &mut Screen) {
        self.over.draw(screen);
    }
}

/// Drawn over the game, which stays visible underneath while it is paused.
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
        // the game's pixels are finished now, so the text below shows above them
        screen.finish_frame();
        plate(
            screen,
            screen.height() / 2 - 2,
            "PAUSED  (p to resume)",
            Colour::BrightWhite,
        );
    }
}

/// A line of text centred on a black strip, so it reads over the sky.
fn plate(screen: &mut Screen, row: i32, text: &str, colour: Colour) {
    let width = screen.width();
    screen.fill(0, row, width, 1, ' ', colour, Colour::Black);
    screen.text_centred(row, text, colour);
}

/// Paints the whole screen with the sky.
fn draw_sky(screen: &mut Screen, pulses: &Pool<Pulse>) {
    let time = screen.time();
    sketch_frame(
        screen.display_mut(),
        &sky_palette(),
        time,
        &Pointer::default(),
        |cell| sky(cell, pulses),
    );
}

/// The palette of the sky: deep blues and magentas, up to bright cyan and white.
fn sky_palette() -> Palette {
    use Colour::*;
    Palette::new(&[
        Blue,
        Magenta,
        BrightMagenta,
        BrightBlue,
        Cyan,
        BrightCyan,
        BrightWhite,
    ])
}

/// The sky at one cell: drifting sines, plus a ring for each pulse. Returns
/// -1 to 1, which is what the palette shades.
fn sky(cell: &Cell, pulses: &Pool<Pulse>) -> f32 {
    let t = cell.t;
    let waves = ((cell.x * 0.15 + t * 0.6).sin()
        + (cell.y * 0.2 - t * 0.5).sin()
        + (cell.x * 0.07 + cell.y * 0.11 + t * 0.3).sin())
        / 3.0;
    let rings: f32 = pulses.iter().map(|pulse| ring(cell, pulse)).sum();
    (0.6 * waves + 0.8 * rings).clamp(-1.0, 1.0)
}

/// One ring of light: bright on its front, which moves outward as the pulse
/// ages, and dimmer as it rolls out. Measured in square units, so it looks round.
fn ring(cell: &Cell, pulse: &Pulse) -> f32 {
    let dx = (cell.x - pulse.x) * 0.5;
    let dy = (cell.y - pulse.y * 2.0) * 0.5; // sketch cells count half rows
    let distance = (dx * dx + dy * dy).sqrt();
    let front = pulse.age * RING_SPEED;
    let fade = 1.0 - pulse.age / PULSE_LIFE;
    (-(distance - front).abs() * 1.5).exp() * fade
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_title_shows_the_name_and_how_to_start() {
        let mut game = Headless::new(Title);
        let text = game.text();
        assert!(text.contains("GLOW"), "{text}");
        assert!(text.contains("press Enter"), "{text}");
    }

    #[test]
    fn q_on_the_title_quits() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Char('q'));
        assert!(game.quit());
    }

    /// What a cell shows after the frame is finished: character, foreground, background.
    fn cell_of(screen: &Screen, x: i32, y: i32) -> (char, Colour, Colour) {
        let pixel = screen
            .display()
            .pixel(x as isize, y as isize)
            .expect("the cell is on the screen");
        (pixel.character(), pixel.colour(), pixel.background())
    }

    #[test]
    fn a_mote_is_a_gold_orb_in_its_cell() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0);
        let mote = Mote {
            x: 10.0,
            y: 5.0,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
        };
        assert!(play.motes.push(mote).is_ok());
        let mut screen = Screen::new(40, 12);
        play.draw(&mut screen);
        screen.finish_frame();
        let (_, _, background) = cell_of(&screen, 10, 5);
        assert_eq!(background, Colour::BrightYellow, "the mote fills its cell");
    }

    /// A game with the light at the centre of an 80 by 24 screen, no new
    /// motes, and nothing else on the screen.
    fn quiet_play() -> Play {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0);
        play
    }

    /// The frame `play` draws, finished, on an 80 by 24 screen.
    fn frame(play: &Play) -> Screen {
        let mut screen = Screen::new(80, 24);
        play.draw(&mut screen);
        screen.finish_frame();
        screen
    }

    #[test]
    fn a_mote_has_a_warm_halo_that_stays_inside_its_radius() {
        let plain = frame(&quiet_play());
        let mut with_mote = quiet_play();
        let mote = Mote {
            x: 30.0,
            y: 10.0,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
        };
        assert!(with_mote.motes.push(mote).is_ok());
        let lit = frame(&with_mote);
        let (cx, cy) = centre(30.0, 10.0);
        let mut changed = 0;
        for y in 0..24 {
            for x in 0..80 {
                if cell_of(&plain, x, y) != cell_of(&lit, x, y) {
                    changed += 1;
                    // a cell's middle, in pixels, from the middle of the mote
                    let dx = x as f32 + 0.5 - cx;
                    let dy = 2.0 * y as f32 + 1.0 - cy;
                    let distance = (dx * dx + dy * dy).sqrt();
                    assert!(
                        distance <= MOTE_HALO * 1.15,
                        "cell ({x}, {y}) is {distance:.1} pixels from the mote"
                    );
                }
            }
        }
        assert!(changed > 10, "a halo, not only the disc: {changed} cells");
    }

    #[test]
    fn the_light_has_a_soft_halo_around_its_orb() {
        let mut lit = quiet_play();
        lit.sparks.clear();
        let mut away = quiet_play();
        away.sparks.clear();
        away.light = Light {
            x: 5.0,
            y: 20.0,
            vx: 0.0,
            vy: 0.0,
        };
        let (here, there) = (frame(&lit), frame(&away));
        let (cx, cy) = centre(40.0, 12.0);
        let mut near = 0;
        for y in 0..24 {
            for x in 0..80 {
                let dx = x as f32 + 0.5 - cx;
                let dy = 2.0 * y as f32 + 1.0 - cy;
                if dx * dx + dy * dy <= 100.0 && cell_of(&here, x, y) != cell_of(&there, x, y) {
                    near += 1;
                }
            }
        }
        assert!(near > 30, "a halo, not only the orb's disc: {near} cells");
    }

    #[test]
    fn the_pause_text_shows_above_the_pixels_under_it() {
        let mut screen = Screen::new(40, 10);
        screen.clear(Colour::Blue);
        // pixel rows 6 and 7 are cell row 3, where the PAUSED plate is drawn
        for x in 0..40 {
            screen.pixel(x, 6, Colour::Red);
            screen.pixel(x, 7, Colour::Red);
        }
        Paused.draw(&mut screen);
        screen.finish_frame();
        let row: String = (0..40).map(|x| cell_of(&screen, x, 3).0).collect();
        assert!(row.contains("PAUSED"), "{row:?}");
    }

    #[test]
    fn drawing_never_panics_with_the_light_and_its_parts_on_any_edge_or_corner() {
        let spots = [
            (0.0, 0.0),
            (11.0, 0.0),
            (0.0, 4.0),
            (11.0, 4.0),
            (5.0, -1.0),
            (-3.0, -3.0),
            (15.0, 8.0),
        ];
        for (x, y) in spots {
            let mut play = Play::new(1);
            update(&mut play, &Keys::new(), &Pointer::default());
            play.spawn = Timer::once(1000.0);
            play.light = Light {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
            };
            let mote = Mote {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                phase: 1.0,
            };
            assert!(play.motes.push(mote).is_ok());
            assert!(play.sparks.push(Spark { x, y, life: 0.3 }).is_ok());
            play.embers.emit(x, y, 1.0, 1.0, 0.5);
            let mut screen = Screen::new(12, 5);
            play.draw(&mut screen);
            screen.finish_frame();
        }
    }

    #[test]
    fn the_light_is_not_drawn_before_its_first_update_places_it() {
        // a new round draws once before its first update, when the light is
        // still at the corner: it must not cover the score in the top left
        let play = Play::new(1);
        let screen = frame(&play);
        assert_ne!(
            cell_of(&screen, 1, 0).2,
            Colour::BrightWhite,
            "{:?}",
            cell_of(&screen, 1, 0)
        );
        assert_ne!(
            cell_of(&screen, 2, 0).2,
            Colour::BrightWhite,
            "{:?}",
            cell_of(&screen, 2, 0)
        );
    }

    #[test]
    fn the_orb_is_drawn_where_the_light_is() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default()); // the light is at (40, 12)
        play.spawn = Timer::once(1000.0);
        let mut screen = Screen::new(80, 24);
        play.draw(&mut screen);
        screen.finish_frame();
        let (x, y) = (play.light.x as i32, play.light.y as i32);
        // the orb's radius is 2.5 pixels, so its middle cell is solid and it
        // reaches half a cell above and below; a row off would show here
        let (ch, _, background) = cell_of(&screen, x, y);
        assert_eq!(
            (ch, background),
            (' ', Colour::BrightWhite),
            "the orb's middle"
        );
        let (ch, colour, _) = cell_of(&screen, x, y - 1);
        assert_eq!(
            (ch, colour),
            ('\u{2584}', Colour::BrightWhite),
            "its top edge"
        );
        let (ch, colour, _) = cell_of(&screen, x, y + 1);
        assert_eq!(
            (ch, colour),
            ('\u{2580}', Colour::BrightWhite),
            "its bottom edge"
        );
    }

    #[test]
    fn the_trail_steps_down_from_bright_to_dim_as_it_ages() {
        let ramp = trail_palette();
        let index = |colour: Colour| ramp.colours().iter().position(|c| *c == colour);
        assert_eq!(
            fade_colour(&ramp, 1.0),
            Colour::BrightWhite,
            "a newborn spark"
        );
        assert_eq!(fade_colour(&ramp, 0.0), Colour::Blue, "a spent spark");
        let indices: Vec<Option<usize>> = (0..=20)
            .rev()
            .map(|i| index(fade_colour(&ramp, i as f32 / 20.0)))
            .collect();
        // from newborn to spent the index only ever goes down, and it goes all the way
        assert!(
            indices.windows(2).all(|pair| pair[0] >= pair[1]),
            "{indices:?}"
        );
        assert_eq!(indices.first(), Some(&Some(3)), "{indices:?}");
        assert_eq!(indices.last(), Some(&Some(0)), "{indices:?}");
    }

    #[test]
    fn a_touch_bursts_sparks_that_are_all_spent_within_two_thirds_of_a_second() {
        let mut play = mote_on_the_light();
        play.spawn = Timer::once(1000.0);
        update(&mut play, &Keys::new(), &Pointer::default());
        assert_eq!(play.embers.len(), EMBER_COUNT, "the touch makes a burst");
        for _ in 0..40 {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        assert!(play.embers.is_empty(), "the burst has died away");
    }

    #[test]
    fn a_young_spark_is_brighter_than_an_old_one() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0);
        let young = Spark {
            x: 10.0,
            y: 5.0,
            life: SPARK_LIFE,
        };
        let old = Spark {
            x: 30.0,
            y: 5.0,
            life: 0.05,
        };
        assert!(play.sparks.push(young).is_ok());
        assert!(play.sparks.push(old).is_ok());
        let mut screen = Screen::new(40, 12);
        play.draw(&mut screen);
        screen.finish_frame();
        // a spark is one pixel, so the lower half of its cell shows its colour
        let (_, young_colour, _) = cell_of(&screen, 10, 5);
        let (_, old_colour, _) = cell_of(&screen, 30, 5);
        assert_eq!(young_colour, Colour::BrightWhite, "newborn");
        assert_eq!(old_colour, Colour::Blue, "nearly spent");
    }

    #[test]
    fn q_quits_during_a_round_and_while_paused() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.tap(KeyCode::Char('q'));
        assert!(game.quit(), "q quits the round");

        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.tap(KeyCode::Char('p'));
        game.tap(KeyCode::Char('q'));
        assert!(game.quit(), "q quits from the pause screen");
    }

    #[test]
    fn enter_starts_a_game_with_no_score_yet() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "{text}");
    }

    /// Holds `code` for `ticks` updates of `play`, pressing it again every
    /// quarter second the way a keyboard repeats.
    fn hold(play: &mut Play, code: KeyCode, ticks: u64) {
        let pointer = Pointer::default();
        let mut keys = Keys::new();
        for tick in 0..ticks {
            let now = Duration::from_secs_f64(tick as f64 / 60.0);
            if tick % 15 == 0 {
                keys.event(&Event::Key(Key::new(code)), now);
            }
            keys.tick(now);
            let mut ctx = Context {
                keys: &keys,
                pointer: &pointer,
                dt: 1.0 / 60.0,
                tick,
                size: (80, 24),
                seed: 1,
            };
            play.update(&mut ctx);
            keys.end_tick();
        }
    }

    #[test]
    fn holding_right_moves_the_light_right() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default()); // places the light
        let before = play.light;
        hold(&mut play, KeyCode::Right, 30);
        assert!(
            play.light.x > before.x + 1.0,
            "{before:?} then {:?}",
            play.light
        );
        assert_eq!(play.light.y, before.y, "moving right keeps the row");
    }

    /// Runs one update of `play` with the given keys and pointer.
    fn update(play: &mut Play, keys: &Keys, pointer: &Pointer) -> Next {
        let mut ctx = Context {
            keys,
            pointer,
            dt: 1.0 / 60.0,
            tick: 0,
            size: (80, 24),
            seed: 1,
        };
        play.update(&mut ctx)
    }

    /// A game with the light placed and one mote on top of it, ready to touch.
    fn mote_on_the_light() -> Play {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default()); // places the light
        let light = play.light;
        let mote = Mote {
            x: light.x,
            y: light.y,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
        };
        assert!(play.motes.push(mote).is_ok());
        play
    }

    #[test]
    fn a_mote_appears_from_the_edge_every_half_second() {
        let mut play = Play::new(1);
        for _ in 0..65 {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        assert_eq!(play.motes.len(), 2, "one at 0.5 s and one at 1.0 s");
    }

    #[test]
    fn motes_drift_and_are_removed_once_they_leave_the_screen() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0); // no new motes in this test
        let drifting = Mote {
            x: 1.0,
            y: 5.0,
            vx: -10.0,
            vy: 0.0,
            phase: 0.0,
        };
        let still = Mote {
            x: 10.0,
            y: 5.0,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
        };
        assert!(play.motes.push(drifting).is_ok());
        assert!(play.motes.push(still).is_ok());
        for _ in 0..30 {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        let left: Vec<f32> = play.motes.iter().map(|mote| mote.x).collect();
        assert_eq!(left.len(), 1, "the one that left is gone: {left:?}");
        assert!(
            (left[0] - 10.0).abs() < 1.0,
            "the other stayed put: {left:?}"
        );
    }

    #[test]
    fn the_trail_is_a_few_fading_sparks_not_a_growing_pile() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0);
        for _ in 0..300 {
            update(&mut play, &Keys::new(), &Pointer::default()); // standing still
        }
        let alive = play.sparks.len();
        assert!(
            (30..=40).contains(&alive),
            "{alive} sparks, each lives 0.6 s"
        );
    }

    #[test]
    fn the_pools_never_grow_past_their_limits_over_3000_ticks() {
        const ARROWS: [KeyCode; 4] = [KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down];
        let mut play = Play::new(7);
        let mut keys = Keys::new();
        let mut script = Rng::new(3);
        let pointer = Pointer::default();
        let mut most_pulses = 0;
        for tick in 0..3000u64 {
            let now = Duration::from_secs_f64(tick as f64 / 60.0);
            if tick % 15 == 0 {
                // a new random arrow every quarter second, so the light wanders and touches motes
                for code in ARROWS {
                    keys.event(&Event::KeyUp(Key::new(code)), now);
                }
                let code = ARROWS[script.range_i32(0, 4) as usize];
                keys.event(&Event::Key(Key::new(code)), now);
            }
            keys.tick(now);
            let mut ctx = Context {
                keys: &keys,
                pointer: &pointer,
                dt: 1.0 / 60.0,
                tick,
                size: (80, 24),
                seed: 7,
            };
            play.update(&mut ctx);
            keys.end_tick();
            assert!(play.motes.len() <= MOTE_LIMIT, "motes at tick {tick}");
            assert!(play.pulses.len() <= PULSE_LIMIT, "pulses at tick {tick}");
            assert!(play.sparks.len() <= SPARK_LIMIT, "sparks at tick {tick}");
            assert!(play.embers.len() <= EMBER_LIMIT, "embers at tick {tick}");
            most_pulses = most_pulses.max(play.pulses.len());
        }
        assert!(play.score > 0, "the script touched some motes");
        assert!(most_pulses > 0, "and so some rings rolled out");
    }

    /// A sketch cell at `(x, y)`, the rest of its fields unused here.
    fn cell_at(x: f32, y: f32) -> Cell {
        Cell {
            x,
            y,
            t: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            width: 80.0,
            height: 48.0,
        }
    }

    #[test]
    fn the_sky_stays_finite_with_all_eight_rings_active() {
        let mut pulses = Pool::with_capacity(PULSE_LIMIT);
        for i in 0..PULSE_LIMIT {
            let pulse = Pulse {
                x: i as f32 * 9.0,
                y: 6.0,
                age: i as f32 * 0.2,
            };
            assert!(pulses.push(pulse).is_ok());
        }
        for t in [0.0, 0.5, 3.7, 1000.0] {
            for y in (0..48).step_by(2) {
                for x in 0..80 {
                    let mut cell = cell_at(x as f32, y as f32);
                    cell.t = t;
                    let value = sky(&cell, &pulses);
                    assert!(
                        value.is_finite() && (-1.0..=1.0).contains(&value),
                        "{value} at {cell:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_ring_is_round_and_bright_on_its_front() {
        // half a second in, the front is 7 units out: 14 columns across, but
        // only 7 rows down, because a row is twice as tall as a column is wide
        let pulse = Pulse {
            x: 40.0,
            y: 12.0,
            age: 0.5,
        };
        let across = ring(&cell_at(54.0, 24.0), &pulse);
        let down = ring(&cell_at(40.0, 38.0), &pulse);
        assert!(across > 0.6, "across: {across}");
        assert!(down > 0.6, "down: {down}");
        assert!(
            ring(&cell_at(40.0, 24.0), &pulse) < 0.05,
            "the middle is dark"
        );
    }

    /// Row `n` of a text screen.
    fn row(text: &str, n: usize) -> &str {
        text.lines().nth(n).unwrap_or("")
    }

    #[test]
    fn holding_up_never_covers_the_score_on_the_top_row() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.press(KeyCode::Left); // into the top left corner, where the score is
        game.press(KeyCode::Up);
        game.step(300); // far more than the screen is wide or high
        let text = game.text();
        assert!(row(&text, 0).starts_with(" score "), "{text}");
    }

    #[test]
    fn p_pauses_the_round_and_p_again_resumes_it() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.step(1);
        game.tap(KeyCode::Char('p'));
        let frozen = game.text();
        assert!(frozen.contains("PAUSED"), "{frozen}");
        game.step(120);
        let later = game.text();
        assert_eq!(row(&later, 0), row(&frozen, 0), "the clock and score stop");
        game.tap(KeyCode::Char('p'));
        assert!(!game.text().contains("PAUSED"), "p resumes");
        game.step(60);
        assert_ne!(
            row(&game.text(), 0),
            row(&frozen, 0),
            "the clock runs again"
        );
    }

    #[test]
    fn the_round_ends_after_45_seconds_on_the_game_over_screen() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.step(45 * 60 + 2);
        let text = game.text();
        assert!(text.contains("TIME!"), "{text}");
        assert!(text.contains("score "), "{text}");
    }

    #[test]
    fn the_game_over_screen_shows_the_score_of_the_round() {
        let mut game = Headless::new(Over { score: 3 });
        let text = game.text();
        assert!(text.contains("score 3"), "{text}");
        assert!(text.contains("Enter: again"), "{text}");
    }

    #[test]
    fn enter_on_the_game_over_screen_starts_a_fresh_round() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.step(45 * 60 + 2);
        assert!(game.text().contains("TIME!"));
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(!text.contains("TIME!"), "{text}");
        assert!(text.contains("45.0s"), "the clock is full again: {text}");
    }

    #[test]
    fn touching_a_mote_scores_it() {
        let mut play = mote_on_the_light();
        update(&mut play, &Keys::new(), &Pointer::default());
        assert_eq!(play.score, 1);
        assert!(play.motes.is_empty(), "a touched mote is removed");
    }

    #[test]
    fn a_touch_rolls_out_a_ring_that_is_gone_after_1_6_seconds() {
        let mut play = mote_on_the_light();
        update(&mut play, &Keys::new(), &Pointer::default());
        assert_eq!(play.pulses.len(), 1, "the touch starts a ring");
        for _ in 0..60 {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        assert_eq!(play.pulses.len(), 1, "still rolling after one second");
        for _ in 0..40 {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        assert!(play.pulses.is_empty(), "gone after 1.6 seconds");
    }

    #[test]
    fn holding_the_mouse_pulls_the_light_toward_the_pointer() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default()); // places the light at the centre
        let pointer = Pointer {
            x: 70.0,
            y: 12.0,
            left: true,
            ..Pointer::default()
        };
        for _ in 0..60 {
            update(&mut play, &Keys::new(), &pointer);
        }
        assert!(play.light.x > 50.0, "pulled right: {:?}", play.light);
    }

    /// Every cell's character and colours, row by row: the whole picture,
    /// including the motes and the light, which are not text.
    fn picture(game: &mut Headless) -> String {
        let screen = game.screen();
        let (width, height) = screen.size();
        let mut out = String::new();
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let (ch, fg, bg) = cell_of(screen, x, y);
                out.push_str(&format!("{ch}{fg:?}{bg:?} "));
            }
            out.push('\n');
        }
        out
    }

    /// Plays 1200 ticks of scripted keys and a click, and returns the picture.
    fn scripted_game(seed: u64) -> String {
        let mut game = Headless::new(Title).seed(seed);
        game.tap(KeyCode::Enter);
        for tick in 0..1200 {
            match tick % 90 {
                0 => game.press(KeyCode::Right),
                45 => game.release(KeyCode::Right),
                _ => {}
            }
            if tick == 600 {
                game.click(10, 5);
            }
            game.step(1);
        }
        picture(&mut game)
    }

    #[test]
    fn the_same_seed_and_inputs_give_the_same_screen_after_1200_ticks() {
        assert_eq!(scripted_game(7), scripted_game(7));
        assert_ne!(
            scripted_game(7),
            scripted_game(8),
            "another seed is another game"
        );
    }

    #[test]
    fn the_website_gets_two_knobs_named_mote_rate_and_round_length() {
        let names: Vec<&str> = tuning().knobs().iter().map(|k| k.name).collect();
        assert_eq!(names, ["mote_rate", "round_length"]);
    }

    #[test]
    fn the_default_knob_values_are_todays_numbers() {
        let knobs = tuning().knobs();
        assert_eq!(knobs[0].default, 1.0, "one times today's spawn rate");
        assert_eq!(knobs[1].default, ROUND_SECONDS);
    }

    #[test]
    fn the_website_start_plays_exactly_like_the_plain_title_with_the_defaults() {
        let mut plain = Headless::new(Title).seed(1);
        plain.tap(KeyCode::Enter);
        plain.step(300);
        let mut tuned = Headless::new(start(tuning())).seed(1);
        tuned.tap(KeyCode::Enter);
        tuned.step(300);
        assert_eq!(picture(&mut tuned), picture(&mut plain));
    }

    /// How many motes a round with `rate` has after `updates` updates.
    fn motes_after(rate: f32, updates: u32) -> usize {
        let knobs = tuning();
        knobs.set("mote_rate", rate);
        let mut play = Play::new(1).with(knobs);
        for _ in 0..updates {
            update(&mut play, &Keys::new(), &Pointer::default());
        }
        play.motes.len()
    }

    #[test]
    fn a_higher_mote_rate_gives_more_motes_in_the_same_time() {
        let slow = motes_after(0.25, 120);
        let fast = motes_after(4.0, 120);
        assert!(fast > slow, "fast {fast} slow {slow}");
    }

    #[test]
    fn a_round_length_of_10_seconds_ends_the_round_after_10_seconds() {
        let knobs = tuning();
        knobs.set("round_length", 10.0);
        let mut game = Headless::new(start(knobs));
        game.tap(KeyCode::Enter);
        game.step(9 * 60);
        assert!(!game.text().contains("TIME!"), "still playing at 9 s");
        game.step(2 * 60);
        assert!(game.text().contains("TIME!"), "the round is over by 11 s");
    }

    /// The seconds the HUD shows on row 0, e.g. `44.1` from `... 44.1s`.
    fn shown_seconds(text: &str) -> f32 {
        row(text, 0)
            .split_whitespace()
            .find_map(|word| word.strip_suffix('s')?.parse::<f32>().ok())
            .unwrap_or(f32::NAN)
    }

    #[test]
    fn moving_the_round_length_mid_round_changes_the_time_shown() {
        let knobs = tuning();
        let mut game = Headless::new(start(knobs.clone()));
        game.tap(KeyCode::Enter);
        game.step(60);
        assert!(shown_seconds(&game.text()) > 43.0, "{}", game.text());
        knobs.set("round_length", 20.0);
        game.step(1);
        let shown = shown_seconds(&game.text());
        assert!(
            shown > 18.0 && shown < 19.5,
            "shown {shown}: {}",
            game.text()
        );
    }

    /// How many of the 20 cells of the HUD's time bar are full. Full cells are
    /// bright white and empty ones bright black, both drawn as blocks.
    fn full_bar_cells(game: &mut Headless) -> usize {
        let screen = game.screen();
        let width = screen.size().0 as i32;
        (width - 24..width - 4)
            .filter(|&x| cell_of(screen, x, 0).2 == Colour::BrightWhite)
            .count()
    }

    #[test]
    fn the_time_bar_shows_the_share_of_a_longer_round_that_is_left() {
        let knobs = tuning();
        knobs.set("round_length", 20.0);
        let mut game = Headless::new(start(knobs));
        game.tap(KeyCode::Enter);
        game.step(60);
        let full = full_bar_cells(&mut game);
        assert!(full >= 17, "about 19 of 20 cells: {full}");
    }

    #[test]
    fn again_after_a_10_second_round_is_another_10_second_round() {
        let knobs = tuning();
        knobs.set("round_length", 10.0);
        let mut game = Headless::new(start(knobs));
        game.tap(KeyCode::Enter);
        game.step(10 * 60 + 2);
        assert!(game.text().contains("TIME!"), "the first round is over");
        game.tap(KeyCode::Enter);
        game.step(9 * 60);
        assert!(!game.text().contains("TIME!"), "still playing at 9 s");
        game.step(2 * 60);
        assert!(
            game.text().contains("TIME!"),
            "the second round is 10 s too"
        );
    }

    #[test]
    fn the_light_stays_on_the_screen() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default()); // places the light
        hold(&mut play, KeyCode::Left, 300); // far more than the screen is wide
        hold(&mut play, KeyCode::Up, 300); // or high
                                           // the top is row 2, below the HUD strip, not row 0
        assert_eq!(
            (play.light.x, play.light.y),
            (0.0, TOP_ROW),
            "{:?}",
            play.light
        );
    }
}

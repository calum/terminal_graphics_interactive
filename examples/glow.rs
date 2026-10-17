//! GLOW: a small light drifts through a living plasma sky. Touch a glowing
//! mote and it bursts: your score goes up and a ring of light rolls out across
//! the sky, which ripples with it. You have 45 seconds.
//!
//! Arrows or WASD steer, hold the left mouse button to be pulled toward the
//! pointer, p pauses, q quits.

use std::f32::consts::TAU;
use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(Title)
}

/// Seconds in a round.
const ROUND_SECONDS: f32 = 45.0;
/// Columns a second. Rows move half as fast, because a cell is twice as tall as it is wide.
const SPEED: f32 = 24.0;
/// How hard a held mouse button pulls the light, in columns a second.
const PULL: f32 = 20.0;
/// How quickly the velocity eases toward what the keys and pointer ask for.
const EASE: f32 = 4.0;

/// The most motes, rings and sparks at once. The pools never grow past these.
const MOTE_LIMIT: usize = 24;
const PULSE_LIMIT: usize = 8;
const SPARK_LIMIT: usize = 160;
/// How long a ring rolls, and how long a spark glows, in seconds.
const PULSE_LIFE: f32 = 1.6;
const SPARK_LIFE: f32 = 0.6;
/// How fast a ring's front moves, and the radii of the light and the motes,
/// all in square units (see `disc`).
const RING_SPEED: f32 = 14.0;
const LIGHT_RADIUS: f32 = 1.2;
const MOTE_RADIUS: f32 = 1.0;

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

/// The cell a position falls in.
fn cell(v: f32) -> i32 {
    v.round() as i32
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
    spawn: Timer,
    clock: Timer,
    rng: Rng,
    controls: Controls<Action>,
}

impl Play {
    fn new(seed: u64) -> Play {
        Play {
            score: 0,
            light: Light::default(),
            motes: Pool::with_capacity(MOTE_LIMIT),
            pulses: Pool::with_capacity(PULSE_LIMIT),
            sparks: Pool::with_capacity(SPARK_LIMIT),
            spawn: Timer::every(0.5),
            clock: Timer::once(ROUND_SECONDS),
            rng: Rng::new(seed),
            controls: controls(),
        }
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
        light.y = (light.y + light.vy * ctx.dt).min(bottom).max(0.0);
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
        let (score, pulses) = (&mut self.score, &mut self.pulses);
        self.motes.retain(|mote| {
            let touched = light.overlaps(&disc(mote.x, mote.y, MOTE_RADIUS));
            if touched {
                *score += 1;
                let _ = pulses.push(Pulse {
                    x: mote.x,
                    y: mote.y,
                    age: 0.0,
                });
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
        if self.clock.tick(ctx.dt) {
            return Next::switch(Over { score: self.score });
        }
        self.steer(ctx);
        self.drift(ctx);
        if self.spawn.tick(ctx.dt) {
            self.spawn_mote(ctx.size);
        }
        self.collect();
        for pulse in self.pulses.iter_mut() {
            pulse.age += ctx.dt;
        }
        self.pulses.retain(|pulse| pulse.age < PULSE_LIFE);
        self.trail(ctx.dt);
        Next::stay()
    }

    fn draw(&self, screen: &mut Screen) {
        draw_sky(screen, &self.pulses);
        for spark in self.sparks.iter() {
            let fade = spark.life / SPARK_LIFE;
            let ch = if fade > 0.66 {
                '*'
            } else if fade > 0.33 {
                ':'
            } else {
                '.'
            };
            screen.put(cell(spark.x), cell(spark.y), ch, Colour::BrightCyan);
        }
        for mote in self.motes.iter() {
            // twinkles between a star and a ring
            let ch = if (screen.time() * 8.0 + mote.phase).sin() > 0.0 {
                '*'
            } else {
                'o'
            };
            screen.put(cell(mote.x), cell(mote.y), ch, Colour::BrightYellow);
        }
        hud(screen, self.score, self.clock.remaining());
        screen.put(
            cell(self.light.x),
            cell(self.light.y),
            '@',
            Colour::BrightWhite,
        );
    }
}

/// The score and the time left, on a black strip along the top.
fn hud(screen: &mut Screen, score: u32, seconds_left: f32) {
    let width = screen.width();
    screen.fill(0, 0, width, 1, ' ', Colour::White, Colour::Black);
    screen.text(1, 0, &format!("score {score}"), Colour::BrightYellow);
    screen.text(width - 31, 0, &format!("{seconds_left:.1}s"), Colour::White);
    let fraction = seconds_left / ROUND_SECONDS;
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

    #[test]
    fn a_mote_is_drawn_as_a_star_or_an_o_in_its_cell() {
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
        let ch = screen.display().pixel(10, 5).unwrap().character();
        assert!(ch == '*' || ch == 'o', "{ch:?}");
    }

    #[test]
    fn a_spark_is_drawn_as_a_fading_dot_in_its_cell() {
        let mut play = Play::new(1);
        update(&mut play, &Keys::new(), &Pointer::default());
        play.spawn = Timer::once(1000.0);
        let spark = Spark {
            x: 10.0,
            y: 5.0,
            life: 0.05,
        };
        assert!(play.sparks.push(spark).is_ok());
        let mut screen = Screen::new(40, 12);
        play.draw(&mut screen);
        let ch = screen.display().pixel(10, 5).unwrap().character();
        assert_eq!(ch, '.', "an old spark is a dot");
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

    /// Where the light is drawn: (column, row), counted in characters.
    fn light_at(text: &str) -> Option<(usize, usize)> {
        text.lines().enumerate().find_map(|(row, line)| {
            line.chars()
                .position(|ch| ch == '@')
                .map(|column| (column, row))
        })
    }

    #[test]
    fn holding_right_moves_the_light_right() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.step(1);
        let before = light_at(&game.text()).expect("the light is drawn");
        game.press(KeyCode::Right);
        game.step(30);
        game.release(KeyCode::Right);
        let after = light_at(&game.text()).expect("the light is still drawn");
        assert!(after.0 > before.0, "{before:?} then {after:?}");
        assert_eq!(after.1, before.1, "moving right keeps the row");
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
        assert_eq!(light_at(&later), light_at(&frozen), "the light stops");
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

    /// Plays 1200 ticks of scripted keys and a click, and returns the screen.
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
        game.text()
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
    fn the_light_stays_on_the_screen() {
        let mut game = Headless::new(Title);
        game.tap(KeyCode::Enter);
        game.press(KeyCode::Left);
        game.press(KeyCode::Up);
        game.step(300); // far more than the screen is wide or high
        assert_eq!(light_at(&game.text()), Some((0, 0)));
    }
}

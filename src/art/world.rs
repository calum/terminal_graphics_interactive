//! A small physics playground: bouncing balls you describe with a few settings.

use super::palette::Palette;
use super::stage::{run_loop, Pointer};
use crate::Display;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use terminal_graphics::Colour;

/// The most balls in a world, so clicking cannot slow it down forever.
const MAX_BALLS: usize = 64;

/// How many frames a trail lasts when [`World::trails`] is on.
const TRAIL_FRAMES: u8 = 14;

/// A world of bouncing balls, described with a few chained settings and
/// started with [`World::run`].
///
/// Balls bounce off the walls and off each other. Move the mouse and they are
/// pulled towards it (with [`World::mouse_pulls`]); hold the right button and
/// they are pushed away. Click to add a ball. Press `q` to quit.
///
/// ```no_run
/// use terminal_graphics_interactive::World;
///
/// World::new()
///     .balls(5)        // five balls
///     .size(3.5)       // big ones
///     .gravity(0.02)   // they fall and bounce
///     .mouse_pulls()   // the pointer attracts them
///     .trails()        // glowing tails
///     .run()?;
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct World {
    count: usize,
    radius: f32,
    gravity: f32,
    pull: bool,
    trail: u8,
    palette: Palette,
}

impl Default for World {
    fn default() -> World {
        World::new()
    }
}

impl World {
    /// A world with 30 small balls and no gravity.
    pub fn new() -> World {
        World {
            count: 30,
            radius: 1.0,
            gravity: 0.0,
            pull: false,
            trail: 1,
            palette: Palette::fire(),
        }
    }

    /// How many balls to start with.
    pub fn balls(mut self, count: usize) -> World {
        self.count = count.min(MAX_BALLS);
        self
    }

    /// How big each ball is, as a radius in cells. Try 3.5 for big ones. Never
    /// less than 0.5, and never bigger than the window.
    pub fn size(mut self, radius: f32) -> World {
        self.radius = radius.max(0.5);
        self
    }

    /// A downward pull each frame. Try 0.02.
    pub fn gravity(mut self, gravity: f32) -> World {
        self.gravity = gravity;
        self
    }

    /// The mouse attracts the balls; holding the right button pushes them away.
    pub fn mouse_pulls(mut self) -> World {
        self.pull = true;
        self
    }

    /// Balls leave glowing tails.
    pub fn trails(mut self) -> World {
        self.trail = TRAIL_FRAMES;
        self
    }

    /// The colours balls are drawn in, by speed.
    pub fn palette(mut self, palette: Palette) -> World {
        self.palette = palette;
        self
    }

    /// The world as a [`Simulation`] you advance yourself, with no terminal:
    /// for tests, for benchmarks, or to draw frames some other way. `seed`
    /// decides where the balls start and which way clicks send them, so the
    /// same seed always gives the same world.
    ///
    /// ```
    /// use terminal_graphics_interactive::{Display, Pointer, World};
    ///
    /// let mut simulation = World::new().balls(3).size(2.0).gravity(0.02).simulation(1);
    /// simulation.resize(40, 12);
    /// let mut display = Display::new(40, 12);
    /// for _ in 0..30 {
    ///     simulation.step(&Pointer::default());
    ///     simulation.render(&mut display);
    /// }
    /// assert_eq!(simulation.positions().len(), 3);
    /// ```
    pub fn simulation(&self, seed: u32) -> Simulation {
        Simulation::new(self, seed)
    }

    /// Runs the world until the user presses `q`.
    pub fn run(self) -> io::Result<()> {
        let mut simulation = Simulation::new(&self, clock_seed());
        run_loop(|display, _seconds, pointer| {
            let (width, height) = display.size();
            simulation.resize(width, height);
            simulation.step(pointer);
            simulation.render(display);
        })
    }
}

/// A seed that differs every run.
fn clock_seed() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |since| since.subsec_nanos())
        ^ std::process::id()
}

#[derive(Clone, Debug, PartialEq)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// The world's state and rules, with no terminal in sight so it can be tested.
///
/// Positions are in cells: `x` along, `y` down in rows. Rows are twice as tall
/// as cells are wide, so wherever distance matters `y` is doubled first and a
/// ball is round.
pub struct Simulation {
    world: World,
    balls: Vec<Ball>,
    /// For every cell: frames of tail left, and the speed of the ball that made it.
    tails: Vec<(u8, f32)>,
    width: u32,
    height: u32,
    /// Whether the starting balls have been created yet.
    started: bool,
    seed: u32,
}

impl Simulation {
    pub(crate) fn new(world: &World, seed: u32) -> Simulation {
        Simulation {
            world: world.clone(),
            balls: Vec::new(),
            tails: Vec::new(),
            width: 0,
            height: 0,
            started: false,
            seed: seed | 1,
        }
    }

    /// The radius actually used: the one asked for, shrunk if the window is
    /// too small to hold a ball that big.
    fn radius(&self) -> f32 {
        let (width, height) = (self.width as f32, self.height as f32);
        self.world
            .radius
            .min((width - 1.0) / 2.0)
            .min(height - 1.0)
            .max(0.5)
    }

    /// The range a ball's centre can be in, so its edge touches the wall:
    /// `(left, right, top, bottom)`. Rows count half as much as columns.
    fn bounds(&self) -> (f32, f32, f32, f32) {
        let radius = self.radius();
        let (width, height) = (self.width as f32, self.height as f32);
        (
            radius,
            (width - 1.0 - radius).max(radius),
            radius / 2.0,
            (height - 1.0 - radius / 2.0).max(radius / 2.0),
        )
    }

    /// A pseudo random number from 0.0 up to (not including) 1.0.
    fn random(&mut self) -> f32 {
        // xorshift32
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Adds a ball at `(x, y)`, moved inside the window, going in a random
    /// direction. Does nothing once there are `MAX_BALLS`.
    fn add_ball(&mut self, x: f32, y: f32, speed: f32) {
        if self.balls.len() >= MAX_BALLS {
            return;
        }
        let (left, right, top, bottom) = self.bounds();
        let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
        let vx = (self.random() - 0.5) * 2.0 * speed;
        let vy = (self.random() - 0.5) * speed;
        self.balls.push(Ball {
            x: finite(x).clamp(left, right),
            y: finite(y).clamp(top, bottom),
            vx,
            vy,
        });
    }

    /// Where the centre of every ball is, as `(column, row)`.
    pub fn positions(&self) -> Vec<(f32, f32)> {
        self.balls.iter().map(|ball| (ball.x, ball.y)).collect()
    }

    /// Tells the simulation the window size. Does nothing if it is unchanged.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.width, self.height) {
            self.width = width;
            self.height = height;
            self.tails = vec![(0, 0.0); width as usize * height as usize];
        }
    }

    /// Moves everything one frame.
    pub fn step(&mut self, pointer: &Pointer) {
        let (left, right, top, bottom) = self.bounds();

        // the starting balls, once we know how big the window is
        if !self.started && self.width > 0 && self.height > 0 {
            self.started = true;
            for _ in 0..self.world.count {
                let x = left + self.random() * (right - left);
                let y = top + self.random() * (bottom - top) * 0.5;
                self.add_ball(x, y, 0.6);
            }
        }
        for &(x, y) in &pointer.clicks {
            self.add_ball(x, y, 0.75);
        }

        for (frames_left, _) in &mut self.tails {
            *frames_left = frames_left.saturating_sub(1);
        }

        // the pointer, if it can be made sense of
        let pointer_at = (!pointer.x.is_nan() && !pointer.y.is_nan())
            .then(|| (pointer.x.clamp(-1e6, 1e6), pointer.y.clamp(-1e6, 1e6)));

        for ball in &mut self.balls {
            ball.vy += self.world.gravity;
            if let (true, Some((px, py))) = (self.world.pull, pointer_at) {
                let dx = px - ball.x;
                let dy = (py - ball.y) * 2.0;
                let distance = (dx * dx + dy * dy).sqrt().max(0.5);
                let direction = if pointer.right { -1.0 } else { 1.0 };
                let strength = direction * 0.9 / (distance + 4.0);
                ball.vx += dx / distance * strength;
                ball.vy += dy / distance * strength * 0.5;
            }
            ball.x += ball.vx;
            ball.y += ball.vy;

            // bounce off the walls by the edge of the ball, losing a little energy
            if ball.x < left || ball.x > right {
                ball.vx = -ball.vx * 0.95;
                ball.x = ball.x.clamp(left, right);
            }
            if ball.y < top || ball.y > bottom {
                ball.vy = -ball.vy * 0.92;
                ball.y = ball.y.clamp(top, bottom);
            }

            // never let a bad number (a NaN gravity, say) spread
            let sane = ball.x.is_finite()
                && ball.y.is_finite()
                && ball.vx.is_finite()
                && ball.vy.is_finite();
            if !sane {
                *ball = Ball {
                    x: (left + right) / 2.0,
                    y: (top + bottom) / 2.0,
                    vx: 0.0,
                    vy: 0.0,
                };
            }
        }

        self.collide();

        // Pushing overlapping balls apart can shove one back through a wall
        // that was already enforced above, so keep everyone inside once more.
        for ball in &mut self.balls {
            ball.x = ball.x.clamp(left, right);
            ball.y = ball.y.clamp(top, bottom);
        }

        self.mark_tails();
    }

    /// Balls bounce off each other as equal weights. Work with rows doubled so
    /// a ball is round.
    fn collide(&mut self) {
        let radius = self.radius();
        for i in 0..self.balls.len() {
            for j in (i + 1)..self.balls.len() {
                let (a, b) = (&self.balls[i], &self.balls[j]);
                let (dx, dy) = (b.x - a.x, (b.y - a.y) * 2.0);
                let distance = (dx * dx + dy * dy).sqrt();
                if distance >= 2.0 * radius {
                    continue;
                }
                // balls on exactly the same spot have no direction: pick one
                let (nx, ny) = if distance > 0.0 {
                    (dx / distance, dy / distance)
                } else {
                    (1.0, 0.0)
                };
                let approach = (b.vx - a.vx) * nx + (b.vy - a.vy) * 2.0 * ny;

                // push them apart so they do not stick together
                let overlap = (2.0 * radius - distance) / 2.0;
                self.balls[i].x -= nx * overlap;
                self.balls[i].y -= ny * overlap / 2.0;
                self.balls[j].x += nx * overlap;
                self.balls[j].y += ny * overlap / 2.0;

                if approach < 0.0 {
                    self.balls[i].vx += approach * nx;
                    self.balls[i].vy += approach * ny / 2.0;
                    self.balls[j].vx -= approach * nx;
                    self.balls[j].vy -= approach * ny / 2.0;
                }
            }
        }
    }

    /// Paints each ball as a disc of cells into the tails.
    fn mark_tails(&mut self) {
        let radius = self.radius();
        let reach_x = radius.ceil() as isize + 1;
        let reach_y = (radius / 2.0).ceil() as isize + 1;
        let (width, height) = (self.width as isize, self.height as isize);

        for ball in &self.balls {
            let speed = (ball.vx * ball.vx + (ball.vy * 2.0).powi(2)).sqrt();
            let (centre_x, centre_y) = (ball.x.round() as isize, ball.y.round() as isize);
            for row in (centre_y - reach_y)..=(centre_y + reach_y) {
                for column in (centre_x - reach_x)..=(centre_x + reach_x) {
                    let dx = column as f32 - ball.x;
                    let dy = (row as f32 - ball.y) * 2.0;
                    let on_screen = (0..width).contains(&column) && (0..height).contains(&row);
                    if on_screen && dx * dx + dy * dy <= radius * radius {
                        let index = (row * width + column) as usize;
                        self.tails[index] = (self.world.trail, speed);
                    }
                }
            }
        }
    }

    /// Draws the balls and their trails into `display`.
    pub fn render(&self, display: &mut Display) {
        for row in 0..self.height {
            for column in 0..self.width {
                let (frames_left, speed) = self.tails[(row * self.width + column) as usize];
                let (block, foreground, background) = if frames_left == 0 {
                    (' ', Colour::Black, Colour::Black)
                } else {
                    let fresh = frames_left as f32 / self.world.trail as f32;
                    let brightness = (speed / 1.5).clamp(0.15, 1.0) * (0.4 + 0.6 * fresh);
                    let (block, foreground, background) = self.world.palette.shade(brightness);
                    // the newest cell of a ball is solid
                    let block = if frames_left == self.world.trail {
                        '█'
                    } else {
                        block
                    };
                    (block, foreground, background)
                };
                display.set_pixel(column as isize, row as isize, block, foreground, background);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics::Colour;

    /// A ball of radius 3 in an 80 by 24 window. Its centre can range over
    /// x in 3..=76 and y in 1.5..=21.5.
    fn world() -> World {
        World::new().balls(0).size(3.0)
    }

    fn sim(world: &World) -> Simulation {
        let mut sim = Simulation::new(world, 1);
        sim.resize(80, 24);
        sim.started = true;
        sim
    }

    fn ball(x: f32, y: f32, vx: f32, vy: f32) -> Ball {
        Ball { x, y, vx, vy }
    }

    fn still() -> Pointer {
        Pointer::default()
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    // -------------------------------------------------------------- setup

    #[test]
    fn the_starting_balls_appear_inside_the_window() {
        let mut sim = Simulation::new(&World::new().balls(5).size(3.0), 7);
        sim.resize(80, 24);
        sim.step(&still());
        assert_eq!(sim.balls.len(), 5);
        for ball in &sim.balls {
            assert!((3.0..=76.0).contains(&ball.x), "x = {}", ball.x);
            assert!((1.5..=21.5).contains(&ball.y), "y = {}", ball.y);
        }
    }

    #[test]
    fn the_starting_balls_are_only_made_once() {
        let mut sim = Simulation::new(&World::new().balls(3), 7);
        sim.resize(80, 24);
        for _ in 0..10 {
            sim.step(&still());
        }
        assert_eq!(sim.balls.len(), 3);
    }

    #[test]
    fn a_world_with_no_balls_stays_empty() {
        let mut sim = Simulation::new(&World::new().balls(0), 7);
        sim.resize(80, 24);
        sim.step(&still());
        assert!(sim.balls.is_empty());
    }

    #[test]
    fn the_builder_clamps_the_numbers_it_is_given() {
        assert_eq!(World::new().size(0.0).radius, 0.5);
        assert_eq!(World::new().size(-4.0).radius, 0.5);
        assert_eq!(World::new().size(f32::NAN).radius, 0.5);
        assert_eq!(World::new().balls(10_000).count, MAX_BALLS);
    }

    #[test]
    fn positions_lists_the_centre_of_every_ball() {
        let mut sim = sim(&world());
        assert!(sim.positions().is_empty());
        sim.balls.push(ball(10.0, 5.0, 0.0, 0.0));
        sim.balls.push(ball(30.5, 8.25, 1.0, 1.0));
        assert_eq!(sim.positions(), vec![(10.0, 5.0), (30.5, 8.25)]);
    }

    #[test]
    fn resizing_makes_room_for_the_tails_of_every_cell() {
        let mut sim = Simulation::new(&World::new(), 1);
        sim.resize(10, 4);
        assert_eq!(sim.tails.len(), 40);
        sim.resize(20, 5);
        assert_eq!(sim.tails.len(), 100);
    }

    // ------------------------------------------------------------- motion

    #[test]
    fn a_ball_moves_by_its_velocity_each_frame() {
        let mut sim = sim(&world());
        sim.balls.push(ball(40.0, 12.0, 0.5, 0.25));
        sim.step(&still());
        assert!(near(sim.balls[0].x, 40.5) && near(sim.balls[0].y, 12.25));
    }

    #[test]
    fn gravity_speeds_a_ball_up_downwards_each_frame() {
        let mut sim = sim(&world().gravity(0.02));
        sim.balls.push(ball(40.0, 10.0, 0.0, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].vy, 0.02));
        assert!(near(sim.balls[0].y, 10.02));
        sim.step(&still());
        assert!(near(sim.balls[0].vy, 0.04));
    }

    // -------------------------------------------------------------- walls

    #[test]
    fn a_ball_bounces_off_the_right_wall_by_its_edge_and_loses_a_little_energy() {
        let mut sim = sim(&world());
        sim.balls.push(ball(75.8, 12.0, 0.5, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].x, 76.0));
        assert!(near(sim.balls[0].vx, -0.475));
    }

    #[test]
    fn a_ball_bounces_off_the_left_wall() {
        let mut sim = sim(&world());
        sim.balls.push(ball(3.2, 12.0, -0.5, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].x, 3.0));
        assert!(near(sim.balls[0].vx, 0.475));
    }

    #[test]
    fn a_ball_bounces_off_the_ceiling() {
        let mut sim = sim(&world());
        sim.balls.push(ball(40.0, 1.7, 0.0, -0.4));
        sim.step(&still());
        assert!(near(sim.balls[0].y, 1.5));
        assert!(near(sim.balls[0].vy, 0.368));
    }

    #[test]
    fn a_ball_bounces_off_the_floor() {
        let mut sim = sim(&world());
        sim.balls.push(ball(40.0, 21.4, 0.0, 0.4));
        sim.step(&still());
        assert!(near(sim.balls[0].y, 21.5));
        assert!(near(sim.balls[0].vy, -0.368));
    }

    #[test]
    fn balls_never_leave_the_window_however_long_it_runs() {
        let mut sim = Simulation::new(
            &World::new().balls(8).size(3.0).gravity(0.02).mouse_pulls(),
            99,
        );
        sim.resize(80, 24);
        for frame in 0..5000 {
            let pointer = Pointer {
                x: (frame * 7 % 80) as f32,
                y: (frame * 3 % 24) as f32,
                right: frame % 50 < 10,
                ..Pointer::default()
            };
            sim.step(&pointer);
            for ball in &sim.balls {
                assert!(
                    (3.0..=76.0).contains(&ball.x),
                    "frame {frame}: x = {}",
                    ball.x
                );
                assert!(
                    (1.5..=21.5).contains(&ball.y),
                    "frame {frame}: y = {}",
                    ball.y
                );
            }
        }
    }

    #[test]
    fn a_ball_bigger_than_the_window_is_shrunk_to_fit_not_thrown_out() {
        let mut sim = Simulation::new(&World::new().balls(3).size(1000.0).gravity(0.02), 5);
        sim.resize(20, 10);
        for _ in 0..500 {
            sim.step(&still());
            for ball in &sim.balls {
                assert!((0.0..=19.0).contains(&ball.x), "x = {}", ball.x);
                assert!((0.0..=9.0).contains(&ball.y), "y = {}", ball.y);
            }
        }
    }

    // -------------------------------------------------------------- mouse

    #[test]
    fn the_mouse_pulls_balls_towards_it() {
        let mut sim = sim(&world().mouse_pulls());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&Pointer {
            x: 70.0,
            y: 12.0,
            ..Pointer::default()
        });
        assert!(sim.balls[0].vx > 0.0);
        assert!(sim.balls[0].vy.abs() < 1e-4);
    }

    #[test]
    fn the_right_button_pushes_balls_away() {
        let mut sim = sim(&world().mouse_pulls());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&Pointer {
            x: 70.0,
            y: 12.0,
            right: true,
            ..Pointer::default()
        });
        assert!(sim.balls[0].vx < 0.0);
    }

    #[test]
    fn without_mouse_pulls_the_mouse_does_nothing() {
        let mut sim = sim(&world());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&Pointer {
            x: 70.0,
            y: 12.0,
            right: true,
            ..Pointer::default()
        });
        assert_eq!(sim.balls[0], ball(40.0, 12.0, 0.0, 0.0));
    }

    #[test]
    fn a_pointer_exactly_on_a_ball_does_not_blow_it_up() {
        let mut sim = sim(&world().mouse_pulls());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&Pointer {
            x: 40.0,
            y: 12.0,
            ..Pointer::default()
        });
        assert!(sim.balls[0].x.is_finite() && sim.balls[0].vx.is_finite());
    }

    #[test]
    fn a_click_adds_a_ball_inside_the_window() {
        let mut sim = sim(&world());
        sim.step(&Pointer {
            clicks: vec![(10.0, 5.0)],
            ..Pointer::default()
        });
        assert_eq!(sim.balls.len(), 1);
        let ball = &sim.balls[0];
        assert!((8.0..=12.0).contains(&ball.x) && (3.0..=7.0).contains(&ball.y));
        assert!(ball.vx != 0.0 || ball.vy != 0.0, "it should move");
    }

    #[test]
    fn a_click_outside_the_window_adds_a_ball_just_inside_it() {
        let mut sim = sim(&world());
        sim.step(&Pointer {
            clicks: vec![(500.0, -500.0)],
            ..Pointer::default()
        });
        let ball = &sim.balls[0];
        assert!((3.0..=76.0).contains(&ball.x) && (1.5..=21.5).contains(&ball.y));
    }

    #[test]
    fn clicking_cannot_add_more_than_the_maximum_number_of_balls() {
        let mut sim = sim(&world());
        sim.step(&Pointer {
            clicks: vec![(10.0, 5.0); 500],
            ..Pointer::default()
        });
        assert_eq!(sim.balls.len(), MAX_BALLS);
    }

    // --------------------------------------------------------- collisions

    fn distance(a: &Ball, b: &Ball) -> f32 {
        ((b.x - a.x).powi(2) + ((b.y - a.y) * 2.0).powi(2)).sqrt()
    }

    #[test]
    fn a_moving_ball_hitting_a_still_one_hands_over_its_speed() {
        let mut sim = sim(&world());
        sim.balls.push(ball(30.0, 12.0, 1.0, 0.0));
        sim.balls.push(ball(35.5, 12.0, 0.0, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].vx, 0.0), "vx = {}", sim.balls[0].vx);
        assert!(near(sim.balls[1].vx, 1.0), "vx = {}", sim.balls[1].vx);
    }

    #[test]
    fn overlapping_balls_are_pushed_apart_so_they_do_not_stick() {
        let mut sim = sim(&world());
        sim.balls.push(ball(30.0, 12.0, 1.0, 0.0));
        sim.balls.push(ball(35.5, 12.0, 0.0, 0.0));
        sim.step(&still());
        assert!(distance(&sim.balls[0], &sim.balls[1]) >= 6.0 - 1e-3);
    }

    #[test]
    fn a_glancing_collision_conserves_momentum_and_energy() {
        let mut sim = sim(&world());
        sim.balls.push(ball(30.0, 10.0, 0.7, 0.2));
        sim.balls.push(ball(34.0, 11.0, -0.3, -0.1));
        let momentum = |s: &Simulation| {
            s.balls
                .iter()
                .fold((0.0, 0.0), |(x, y), b| (x + b.vx, y + b.vy * 2.0))
        };
        let energy = |s: &Simulation| {
            s.balls
                .iter()
                .map(|b| b.vx * b.vx + (b.vy * 2.0).powi(2))
                .sum::<f32>()
        };
        let (before_p, before_e) = (momentum(&sim), energy(&sim));
        sim.step(&still());
        let (after_p, after_e) = (momentum(&sim), energy(&sim));
        assert!(near(before_p.0, after_p.0) && near(before_p.1, after_p.1));
        assert!(
            (before_e - after_e).abs() < 1e-3,
            "{before_e} then {after_e}"
        );
        // and they really did collide: the velocities changed
        assert!(!near(sim.balls[0].vx, 0.7));
    }

    #[test]
    fn balls_that_are_apart_do_not_affect_each_other() {
        let mut sim = sim(&world());
        sim.balls.push(ball(20.0, 12.0, 0.5, 0.0));
        sim.balls.push(ball(60.0, 12.0, -0.5, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].vx, 0.5) && near(sim.balls[1].vx, -0.5));
    }

    #[test]
    fn balls_moving_apart_while_overlapping_are_not_slowed_or_reversed() {
        let mut sim = sim(&world());
        sim.balls.push(ball(30.0, 12.0, -0.3, 0.0));
        sim.balls.push(ball(34.0, 12.0, 0.3, 0.0));
        sim.step(&still());
        assert!(near(sim.balls[0].vx, -0.3) && near(sim.balls[1].vx, 0.3));
    }

    #[test]
    fn two_balls_on_exactly_the_same_spot_do_not_produce_nan() {
        let mut sim = sim(&world());
        sim.balls.push(ball(40.0, 12.0, 0.1, 0.0));
        sim.balls.push(ball(40.0, 12.0, -0.1, 0.0));
        sim.step(&still());
        for ball in &sim.balls {
            assert!(ball.x.is_finite() && ball.y.is_finite() && ball.vx.is_finite());
        }
    }

    // -------------------------------------------------------------- trails

    fn marked(sim: &Simulation) -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for y in 0..sim.height {
            for x in 0..sim.width {
                if sim.tails[(y * sim.width + x) as usize].0 > 0 {
                    cells.push((x as i32, y as i32));
                }
            }
        }
        cells
    }

    #[test]
    fn a_ball_is_drawn_as_a_round_disc_of_cells() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&still());
        let cells = marked(&sim);
        // pi r^2 / 2 is about 14, because a row is twice as tall as a cell is wide
        assert!((10..=20).contains(&cells.len()), "{} cells", cells.len());
        for &(x, y) in &cells {
            assert!(cells.contains(&(80 - x, y)), "not symmetric left to right");
            assert!(cells.contains(&(x, 24 - y)), "not symmetric top to bottom");
        }
        assert!(
            cells.contains(&(43, 12)) && cells.contains(&(37, 12)),
            "3 cells wide each way"
        );
        assert!(
            !cells.contains(&(40, 14)),
            "rows are twice as tall, so 2 rows is too far"
        );
        assert!(cells.contains(&(40, 13)));
    }

    #[test]
    fn a_disc_at_the_edge_is_clipped_not_wrapped() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(3.0, 1.5, 0.0, 0.0));
        sim.step(&still());
        for (x, y) in marked(&sim) {
            assert!((0..80).contains(&x) && (0..24).contains(&y));
        }
        assert!(
            !marked(&sim).iter().any(|&(x, _)| x > 10),
            "nothing wrapped to the far side"
        );
    }

    #[test]
    fn a_fresh_trail_fades_away_over_exactly_the_trail_length() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&still());
        let at = (12 * 80 + 40) as usize;
        assert_eq!(sim.tails[at].0, TRAIL_FRAMES);
        sim.balls.clear();
        for frame in 1..=TRAIL_FRAMES {
            sim.step(&still());
            assert_eq!(sim.tails[at].0, TRAIL_FRAMES - frame);
        }
        assert!(marked(&sim).is_empty());
    }

    #[test]
    fn without_trails_only_the_ball_itself_is_left_behind() {
        let mut sim = sim(&world());
        sim.balls.push(ball(20.0, 12.0, 1.0, 0.0));
        sim.step(&still());
        sim.step(&still());
        let cells = marked(&sim);
        // the ball is at x = 22 with radius 3, so its disc reaches back to 19; a
        // tail would reach back to 18, where the first frame's disc began
        assert!(
            cells.iter().all(|&(x, _)| x >= 19),
            "no tail behind the ball: {cells:?}"
        );
    }

    // --------------------------------------------------------------- render

    fn rendered(sim: &Simulation) -> Display {
        let mut display = Display::new(sim.width, sim.height);
        sim.render(&mut display);
        display
    }

    #[test]
    fn the_centre_of_a_fresh_ball_is_a_solid_block_and_empty_space_is_black() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&still());
        let display = rendered(&sim);
        assert_eq!(display.pixel(40, 12).unwrap().character(), '█');
        let empty = display.pixel(10, 3).unwrap();
        assert_eq!(empty.character(), ' ');
        assert_eq!(empty.background(), Colour::Black);
    }

    #[test]
    fn a_fading_trail_is_drawn_as_a_lighter_block_than_a_fresh_one() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        sim.step(&still());
        sim.balls.clear();
        sim.step(&still());
        let character = rendered(&sim).pixel(40, 12).unwrap().character();
        assert!(['░', '▒', '▓'].contains(&character), "got {character:?}");
    }

    #[test]
    fn faster_balls_are_drawn_brighter_than_slower_ones() {
        let brightness = |speed: f32| {
            let mut sim = sim(&world().palette(Palette::mono()));
            sim.balls.push(ball(40.0, 12.0, speed, 0.0));
            sim.step(&still());
            let display = rendered(&sim);
            let colour = display.pixel(41, 12).unwrap().colour();
            Palette::mono()
                .colours()
                .iter()
                .position(|c| *c == colour)
                .unwrap()
        };
        assert!(brightness(1.4) > brightness(0.05));
    }

    #[test]
    fn the_display_may_be_a_different_size_from_the_simulation() {
        let mut sim = sim(&world().trails());
        sim.balls.push(ball(70.0, 12.0, 0.0, 0.0));
        sim.step(&still());
        let mut small = Display::new(10, 5);
        sim.render(&mut small);
        let mut huge = Display::new(300, 100);
        sim.render(&mut huge);
    }

    // ------------------------------------------------------------ robustness

    #[test]
    fn windows_of_any_size_including_none_never_panic() {
        for (w, h) in [(0, 0), (0, 5), (5, 0), (1, 1), (2, 1), (1, 9), (5, 2)] {
            let mut sim = Simulation::new(
                &World::new()
                    .balls(6)
                    .size(4.0)
                    .gravity(0.05)
                    .mouse_pulls()
                    .trails(),
                3,
            );
            sim.resize(w, h);
            let mut display = Display::new(w, h);
            for frame in 0..200 {
                sim.step(&Pointer {
                    x: frame as f32,
                    y: 1.0,
                    right: frame % 2 == 0,
                    clicks: if frame % 40 == 0 {
                        vec![(3.0, 3.0)]
                    } else {
                        vec![]
                    },
                    ..Pointer::default()
                });
                sim.render(&mut display);
            }
        }
    }

    #[test]
    fn a_pointer_with_nan_or_huge_coordinates_does_not_poison_the_world() {
        let mut sim = sim(&world().mouse_pulls());
        sim.balls.push(ball(40.0, 12.0, 0.0, 0.0));
        for x in [f32::NAN, f32::INFINITY, -1e30, 1e30] {
            sim.step(&Pointer {
                x,
                y: x,
                ..Pointer::default()
            });
        }
        let ball = &sim.balls[0];
        assert!(ball.x.is_finite() && ball.y.is_finite(), "{ball:?}");
        assert!((3.0..=76.0).contains(&ball.x) && (1.5..=21.5).contains(&ball.y));
    }

    #[test]
    fn the_same_seed_gives_the_same_world() {
        let run = || {
            let mut sim = Simulation::new(&World::new().balls(6).size(3.0).gravity(0.02), 1234);
            sim.resize(80, 24);
            for _ in 0..300 {
                sim.step(&still());
            }
            sim.balls
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let positions = |seed| {
            let mut sim = Simulation::new(&World::new().balls(6), seed);
            sim.resize(80, 24);
            sim.step(&still());
            sim.balls
        };
        assert_ne!(positions(1), positions(2));
    }
}

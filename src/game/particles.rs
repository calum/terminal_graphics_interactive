//! [`Particles`]: pooled bursts, sparks and spray, with no allocation after
//! construction.

use super::{Pool, Rng, Screen};
use crate::Palette;
use std::f32::consts::TAU;

/// One particle: where it is, how fast it moves (cells and cells a second),
/// and how old it is out of how long it lives (seconds).
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let mut particles = Particles::new(8);
/// particles.emit(4.0, 2.0, 1.0, 0.0, 0.5);
/// let p = particles.iter().next().unwrap();
/// assert_eq!((p.x, p.y), (4.0, 2.0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Column.
    pub x: f32,
    /// Row (grows downwards).
    pub y: f32,
    /// Columns a second.
    pub vx: f32,
    /// Rows a second.
    pub vy: f32,
    /// Seconds since it was emitted.
    pub age: f32,
    /// Seconds it lives before it is removed.
    pub life: f32,
}

/// A pool of particles that move, fall, slow down and fade.
///
/// Storage is fixed when you call [`Particles::new`]. A full system ignores
/// new particles, so a busy scene never allocates and never slows down.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let mut sparks = Particles::new(64).gravity(9.0);
/// sparks.emit(10.0, 5.0, 3.0, -2.0, 0.8);
/// sparks.step(0.1);
/// assert_eq!(sparks.len(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct Particles {
    live: Pool<Particle>,
    gravity: f32,
    drag: f32,
    palette: Palette,
}

impl Particles {
    /// An empty system that holds up to `capacity` particles.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let particles = Particles::new(100);
    /// assert!(particles.is_empty());
    /// ```
    pub fn new(capacity: usize) -> Particles {
        Particles {
            live: Pool::with_capacity(capacity),
            gravity: 0.0,
            drag: 0.0,
            palette: Palette::fire(),
        }
    }

    /// The colours a particle fades through, from bright when it is born to
    /// dark when it dies. The default is [`Palette::fire`].
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let embers = Particles::new(16).palette(Palette::fire());
    /// assert!(embers.is_empty());
    /// ```
    pub fn palette(mut self, palette: Palette) -> Particles {
        self.palette = palette;
        self
    }

    /// Paints every particle on `screen`. A newborn particle uses the bright
    /// end of the palette and a dying one the dark end. Particles off the
    /// screen are clipped, so nothing panics.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut spark = Particles::new(4);
    /// spark.emit(2.0, 0.0, 0.0, 0.0, 1.0);
    /// let mut screen = Screen::new(4, 1);
    /// spark.draw(&mut screen);
    /// assert_ne!(screen.display().pixel(2, 0).unwrap().character(), ' ');
    /// ```
    pub fn draw(&self, screen: &mut Screen) {
        for p in self.live.iter() {
            let fade = 1.0 - p.age / p.life;
            let (ch, fg, _bg) = self.palette.shade(fade);
            screen.put(p.x.round() as i32, p.y.round() as i32, ch, fg);
        }
    }

    /// The fraction of their speed particles lose each second: 0.0 keeps
    /// every particle at full speed, 1.0 stops them at once. Values outside
    /// that range are clamped, and NaN counts as 0.0.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut smoke = Particles::new(4).drag(0.5);
    /// smoke.emit(0.0, 0.0, 8.0, 0.0, 2.0);
    /// smoke.step(1.0); // half the speed is gone after a second
    /// assert_eq!(smoke.iter().next().unwrap().vx, 4.0);
    /// ```
    pub fn drag(mut self, d: f32) -> Particles {
        self.drag = if d.is_nan() { 0.0 } else { d.clamp(0.0, 1.0) };
        self
    }

    /// Pull every particle's row by `g` rows a second each second. Positive
    /// falls down the screen (rows grow downwards). A value that is not a
    /// finite number is ignored.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut sparks = Particles::new(4).gravity(20.0);
    /// sparks.emit(0.0, 0.0, 0.0, 0.0, 1.0);
    /// sparks.step(0.5);
    /// assert_eq!(sparks.iter().next().unwrap().vy, 10.0);
    /// ```
    pub fn gravity(mut self, g: f32) -> Particles {
        if g.is_finite() {
            self.gravity = g;
        }
        self
    }

    /// Adds one particle at column `x`, row `y`, moving at `vx`, `vy`, which
    /// lives for `life` seconds. It is ignored when the system is full, when
    /// `life` is not above zero, or when any value is not a finite number.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut particles = Particles::new(1);
    /// particles.emit(0.0, 0.0, 1.0, 1.0, 1.0);
    /// particles.emit(5.0, 5.0, 1.0, 1.0, 1.0); // full: silently ignored
    /// assert_eq!(particles.len(), 1);
    /// ```
    pub fn emit(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32) {
        let finite = [x, y, vx, vy, life].iter().all(|v| v.is_finite());
        if !finite || life <= 0.0 {
            return;
        }
        let _ = self.live.push(Particle {
            x,
            y,
            vx,
            vy,
            age: 0.0,
            life,
        });
    }

    /// Moves every particle for `dt` seconds, then removes the ones that have
    /// lived out their `life`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut particles = Particles::new(4);
    /// particles.emit(0.0, 0.0, 10.0, 0.0, 0.5);
    /// particles.step(0.25);
    /// assert_eq!(particles.iter().next().unwrap().x, 2.5);
    /// ```
    pub fn step(&mut self, dt: f32) {
        if !dt.is_finite() || dt < 0.0 {
            return;
        }
        let gravity = self.gravity;
        let keep = (1.0 - self.drag).powf(dt);
        for p in self.live.iter_mut() {
            p.vy += gravity * dt;
            p.vx *= keep;
            p.vy *= keep;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.age += dt;
        }
        // a particle pushed past the largest number is dropped, not kept as inf
        self.live
            .retain(|p| p.age < p.life && [p.x, p.y, p.vx, p.vy].iter().all(|v| v.is_finite()));
    }

    /// A radial burst: `count` particles leave `(x, y)` in random directions,
    /// at a speed between half of `speed` and `speed`, living between 70% and
    /// 100% of `life`. Pass the randomness from your own [`Rng`], so a replay
    /// with the same seed gives the same burst.
    ///
    /// The horizontal speed is doubled, because a terminal cell is about twice
    /// as tall as it is wide: this makes the burst look round. Only as many
    /// particles as fit are made, and bad values (a non-finite speed, or a
    /// `life` that is not positive) make none.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut rng = Rng::new(42);
    /// let mut fireworks = Particles::new(64);
    /// fireworks.burst(&mut rng, 40.0, 12.0, 30, 12.0, 1.5);
    /// assert_eq!(fireworks.len(), 30);
    /// ```
    pub fn burst(&mut self, rng: &mut Rng, x: f32, y: f32, count: usize, speed: f32, life: f32) {
        if !(x.is_finite() && y.is_finite() && speed.is_finite() && life > 0.0 && life.is_finite())
        {
            return;
        }
        let room = self.live.capacity() - self.live.len();
        for _ in 0..count.min(room) {
            let angle = rng.range_f32(0.0, TAU);
            let s = speed * rng.range_f32(0.5, 1.0);
            let lived = life * rng.range_f32(0.7, 1.0);
            self.emit(x, y, 2.0 * s * angle.cos(), s * angle.sin(), lived);
        }
    }

    /// How many particles are alive.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut particles = Particles::new(4);
    /// particles.emit(0.0, 0.0, 0.0, 0.0, 1.0);
    /// assert_eq!(particles.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Remove every particle. The settings (gravity, drag, palette) and the
    /// capacity stay the same.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut particles = Particles::new(4);
    /// particles.emit(0.0, 0.0, 0.0, 0.0, 1.0);
    /// particles.clear();
    /// assert!(particles.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.live.clear();
    }

    /// True when no particle is alive.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let particles = Particles::new(4);
    /// assert!(particles.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// The live particles, oldest first.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut particles = Particles::new(4);
    /// particles.emit(1.0, 1.0, 0.0, 0.0, 1.0);
    /// assert_eq!(particles.iter().count(), 1);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &Particle> {
        self.live.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics::Colour;

    #[test]
    fn a_particle_moves_by_its_velocity_times_the_time_step() {
        let mut particles = Particles::new(4);
        particles.emit(10.0, 5.0, 2.0, -1.0, 1.0);
        particles.step(0.5);
        let only = particles.iter().next().unwrap();
        assert!((only.x - 11.0).abs() < 1e-5, "x was {}", only.x);
        assert!((only.y - 4.5).abs() < 1e-5, "y was {}", only.y);
    }

    #[test]
    fn gravity_speeds_up_falling_particles_every_step() {
        let mut particles = Particles::new(4).gravity(10.0);
        particles.emit(0.0, 0.0, 0.0, 0.0, 5.0);
        particles.step(0.5);
        assert!((particles.iter().next().unwrap().vy - 5.0).abs() < 1e-5);
        particles.step(0.5);
        let p = particles.iter().next().unwrap();
        assert!((p.vy - 10.0).abs() < 1e-5, "vy was {}", p.vy);
        assert!(p.y > 0.0, "positive gravity falls down the screen");
    }

    #[test]
    fn emit_ignores_a_life_that_is_not_positive_and_any_non_finite_value() {
        let mut particles = Particles::new(8);
        particles.emit(0.0, 0.0, 0.0, 0.0, 0.0);
        particles.emit(0.0, 0.0, 0.0, 0.0, -1.0);
        particles.emit(0.0, 0.0, 0.0, 0.0, f32::NAN);
        particles.emit(f32::NAN, 0.0, 0.0, 0.0, 1.0);
        particles.emit(0.0, f32::INFINITY, 0.0, 0.0, 1.0);
        particles.emit(0.0, 0.0, f32::NAN, 0.0, 1.0);
        particles.emit(0.0, 0.0, 0.0, f32::NEG_INFINITY, 1.0);
        assert!(particles.is_empty());
    }

    #[test]
    fn a_negative_or_non_finite_time_step_changes_nothing() {
        let mut particles = Particles::new(4).gravity(9.0);
        particles.emit(3.0, 3.0, 1.0, 1.0, 2.0);
        let before: Vec<Particle> = particles.iter().copied().collect();
        particles.step(-0.5);
        particles.step(f32::NAN);
        particles.step(f32::INFINITY);
        let after: Vec<Particle> = particles.iter().copied().collect();
        assert_eq!(after, before);
    }

    #[test]
    fn a_huge_time_step_or_speed_never_leaves_a_non_finite_particle() {
        let mut particles = Particles::new(8).gravity(1e38);
        particles.emit(0.0, 0.0, 3e38, 3e38, 1e30);
        particles.emit(0.0, 0.0, 1.0, 1.0, 1.0);
        particles.step(1e9);
        for p in particles.iter() {
            assert!(
                [p.x, p.y, p.vx, p.vy].iter().all(|v| v.is_finite()),
                "{p:?}"
            );
        }
    }

    #[test]
    fn a_burst_is_the_same_for_the_same_seed_and_different_for_another() {
        let make = |seed: u64| {
            let mut particles = Particles::new(50);
            particles.burst(&mut Rng::new(seed), 0.0, 0.0, 20, 10.0, 1.0);
            particles.iter().copied().collect::<Vec<Particle>>()
        };
        assert_eq!(make(7), make(7));
        assert_ne!(make(7), make(8));
    }

    #[test]
    fn a_newborn_particle_is_drawn_bright_and_a_dying_one_dim() {
        let mut particles =
            Particles::new(8).palette(Palette::new(&[Colour::Black, Colour::White]));
        particles.emit(1.0, 0.0, 0.0, 0.0, 1.0);
        particles.step(0.9);
        particles.emit(3.0, 0.0, 0.0, 0.0, 1.0);

        let mut screen = Screen::new(5, 1);
        particles.draw(&mut screen);
        let dying = screen.display().pixel(1, 0).unwrap().character();
        let newborn = screen.display().pixel(3, 0).unwrap().character();
        assert_eq!(newborn, '█', "a new particle is the brightest block");
        assert_eq!(dying, ' ', "an old particle has faded to the dimmest");
        assert_eq!(
            screen.display().pixel(3, 0).unwrap().colour(),
            Colour::White
        );
    }

    #[test]
    fn a_burst_is_round_on_screen_because_its_x_speed_is_doubled() {
        let mut particles = Particles::new(2000);
        particles.burst(&mut Rng::new(3), 0.0, 0.0, 2000, 10.0, 1.0);
        let n = particles.len() as f32;
        let (mut sum_x, mut sum_y, mut abs_x, mut abs_y) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
        for p in particles.iter() {
            sum_x += p.vx;
            sum_y += p.vy;
            abs_x += p.vx.abs();
            abs_y += p.vy.abs();
        }
        assert!((sum_x / n).abs() < 0.5, "mean vx {}", sum_x / n);
        assert!((sum_y / n).abs() < 0.5, "mean vy {}", sum_y / n);
        let ratio = abs_x / abs_y;
        assert!(
            (1.8..2.2).contains(&ratio),
            "x speed to y speed was {ratio}"
        );
    }

    #[test]
    fn a_burst_has_speeds_and_lives_within_the_stated_ranges() {
        let mut particles = Particles::new(500);
        particles.burst(&mut Rng::new(5), 0.0, 0.0, 500, 10.0, 2.0);
        for p in particles.iter() {
            // undo the doubled x speed to get the real speed in cells
            let speed = ((p.vx / 2.0).powi(2) + p.vy.powi(2)).sqrt();
            assert!((5.0 - 1e-3..=10.0 + 1e-3).contains(&speed), "speed {speed}");
            assert!(
                (1.4 - 1e-3..=2.0 + 1e-3).contains(&p.life),
                "life {}",
                p.life
            );
        }
    }

    #[test]
    fn a_burst_makes_no_more_than_fits_and_nothing_for_bad_values() {
        let mut particles = Particles::new(10);
        particles.burst(&mut Rng::new(1), 0.0, 0.0, usize::MAX, 4.0, 1.0);
        assert_eq!(
            particles.len(),
            10,
            "stops when full, even for a huge count"
        );

        let mut none = Particles::new(10);
        none.burst(&mut Rng::new(1), 0.0, 0.0, 5, f32::NAN, 1.0);
        none.burst(&mut Rng::new(1), 0.0, 0.0, 5, 4.0, 0.0);
        none.burst(&mut Rng::new(1), f32::INFINITY, 0.0, 5, 4.0, 1.0);
        assert!(none.is_empty());
    }

    #[test]
    fn survivors_keep_the_order_they_were_emitted_in() {
        let mut particles = Particles::new(8);
        particles.emit(0.0, 0.0, 0.0, 0.0, 9.0);
        particles.emit(1.0, 0.0, 0.0, 0.0, 0.5); // dies first, in the middle
        particles.emit(2.0, 0.0, 0.0, 0.0, 9.0);
        particles.emit(3.0, 0.0, 0.0, 0.0, 9.0);
        particles.step(1.0);
        let xs: Vec<f32> = particles.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![0.0, 2.0, 3.0]);
    }

    #[test]
    fn particles_off_the_screen_are_clipped_and_zero_steps_change_nothing() {
        let mut particles = Particles::new(4);
        particles.emit(-3.0, 0.0, 0.0, 0.0, 1.0);
        particles.emit(99.0, 0.0, 0.0, 0.0, 1.0);
        particles.step(0.0);
        let mut screen = Screen::new(4, 1);
        particles.draw(&mut screen);
        for x in 0..4 {
            assert_eq!(screen.display().pixel(x, 0).unwrap().character(), ' ');
        }
        assert_eq!(particles.len(), 2);
    }

    #[test]
    fn a_drag_above_one_stops_particles_and_a_nan_drag_does_nothing() {
        let mut stopped = Particles::new(4).drag(7.0);
        stopped.emit(0.0, 0.0, 5.0, 0.0, 9.0);
        stopped.step(0.1);
        assert_eq!(stopped.iter().next().unwrap().vx, 0.0);

        let mut free = Particles::new(4).drag(f32::NAN);
        free.emit(0.0, 0.0, 5.0, 0.0, 9.0);
        free.step(0.1);
        assert_eq!(free.iter().next().unwrap().vx, 5.0);
    }

    #[test]
    fn clear_removes_every_particle_and_keeps_the_settings() {
        let mut particles = Particles::new(4).gravity(3.0);
        particles.emit(0.0, 0.0, 0.0, 0.0, 1.0);
        particles.clear();
        assert!(particles.is_empty());
        particles.emit(0.0, 0.0, 0.0, 0.0, 5.0);
        particles.step(1.0);
        assert_eq!(
            particles.iter().next().unwrap().vy,
            3.0,
            "gravity is still set"
        );
    }

    #[test]
    fn a_full_system_ignores_new_particles_and_keeps_the_old_ones() {
        let mut particles = Particles::new(2);
        particles.emit(1.0, 1.0, 0.0, 0.0, 5.0);
        particles.emit(2.0, 2.0, 0.0, 0.0, 5.0);
        particles.emit(9.0, 9.0, 0.0, 0.0, 5.0);
        let xs: Vec<f32> = particles.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![1.0, 2.0]);
    }

    #[test]
    fn the_capacity_holds_through_a_long_run_of_emits_and_steps() {
        // three emits a step with long lives fill the pool well before the end
        let mut particles = Particles::new(1000);
        for i in 0..100_000 {
            for _ in 0..3 {
                particles.emit(i as f32 % 80.0, 10.0, 3.0, -4.0, 10.0);
            }
            particles.step(1.0 / 60.0);
            assert!(particles.len() <= 1000);
        }
        assert_eq!(particles.len(), 1000, "the pool was filled");
        assert_eq!(particles.live.capacity(), 1000);
    }

    #[test]
    fn a_particle_is_removed_once_it_has_lived_its_life() {
        let mut particles = Particles::new(4);
        particles.emit(0.0, 0.0, 0.0, 0.0, 1.0);
        particles.step(0.5);
        assert_eq!(
            particles.len(),
            1,
            "half way through its life it is still here"
        );
        particles.step(0.5);
        assert!(particles.is_empty(), "at the end of its life it is gone");
    }

    #[test]
    fn drag_loses_the_given_fraction_of_velocity_each_second() {
        let mut particles = Particles::new(4).drag(0.5);
        particles.emit(0.0, 0.0, 8.0, 0.0, 9.0);
        particles.step(1.0);
        let vx = particles.iter().next().unwrap().vx;
        assert!((vx - 4.0).abs() < 1e-5, "vx was {vx}");
    }
}

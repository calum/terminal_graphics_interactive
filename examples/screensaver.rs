//! A comet-trail screensaver. Dots bounce around the window leaving fading
//! trails. Click to add another dot; press any key to quit.
//!
//! It shows the shape of a typical animation: a loop that draws a frame, then
//! spends the time until the next frame handling input, then advances the
//! animation. Run it with `cargo run --example screensaver`.

use std::io;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use terminal_graphics_interactive::{Colour, Display, Event, MouseKind, Terminal};

/// The time between frames: about 30 frames a second.
const FRAME: Duration = Duration::from_millis(33);

/// How many frames a trail lasts before it has faded away.
const TRAIL_LENGTH: u8 = 12;

/// The most dots on screen, so clicking cannot slow the animation forever.
const MAX_DOTS: usize = 64;

const COLOURS: [Colour; 6] = [
    Colour::BrightRed,
    Colour::BrightYellow,
    Colour::BrightGreen,
    Colour::BrightCyan,
    Colour::BrightBlue,
    Colour::BrightMagenta,
];

/// What the screensaver should do after an event.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Continue,
    Quit,
}

struct Dot {
    x: f32,
    y: f32,
    /// Cells moved per frame. Terminal cells are about twice as tall as they
    /// are wide, so the vertical speed is smaller to look the same.
    dx: f32,
    dy: f32,
    colour: Colour,
}

struct Screensaver {
    width: isize,
    height: isize,
    dots: Vec<Dot>,
    /// For every cell: the colour of the dot that last passed, and how many
    /// frames of trail it has left (0 means nothing there).
    trail: Vec<(Colour, u8)>,
    /// State for a tiny random number generator, so no crate is needed.
    seed: u32,
}

impl Screensaver {
    fn new(width: u32, height: u32, seed: u32) -> Screensaver {
        let mut saver = Screensaver {
            width: width as isize,
            height: height as isize,
            dots: Vec::new(),
            trail: vec![(Colour::Black, 0); width as usize * height as usize],
            seed: seed | 1,
        };
        for _ in 0..4 {
            saver.add_dot(width as f32 / 2.0, height as f32 / 2.0);
        }
        saver
    }

    /// A pseudo random number from 0.0 up to (not including) 1.0.
    fn random(&mut self) -> f32 {
        // xorshift32
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// The largest x and y a dot can have: the last cell of the window.
    fn limits(&self) -> (f32, f32) {
        (
            (self.width - 1).max(0) as f32,
            (self.height - 1).max(0) as f32,
        )
    }

    /// Adds a dot at `(x, y)` (moved inside the window if it is outside)
    /// moving in a random direction. Does nothing once there are `MAX_DOTS`.
    fn add_dot(&mut self, x: f32, y: f32) {
        if self.dots.len() >= MAX_DOTS {
            return;
        }
        let (max_x, max_y) = self.limits();
        let sign = |random: f32| if random < 0.5 { -1.0 } else { 1.0 };
        let dx = (0.2 + 0.5 * self.random()) * sign(self.random());
        let dy = (0.1 + 0.25 * self.random()) * sign(self.random());
        self.dots.push(Dot {
            x: x.clamp(0.0, max_x),
            y: y.clamp(0.0, max_y),
            dx,
            dy,
            colour: COLOURS[self.dots.len() % COLOURS.len()],
        });
    }

    /// Moves every dot one frame, bounces them off the walls and fades trails.
    fn step(&mut self) {
        for (_, frames_left) in &mut self.trail {
            *frames_left = frames_left.saturating_sub(1);
        }

        let (max_x, max_y) = self.limits();
        for dot in &mut self.dots {
            dot.x += dot.dx;
            dot.y += dot.dy;
            bounce(&mut dot.x, &mut dot.dx, max_x);
            bounce(&mut dot.y, &mut dot.dy, max_y);

            if self.width > 0 && self.height > 0 {
                let index = dot.y.round() as usize * self.width as usize + dot.x.round() as usize;
                self.trail[index] = (dot.colour, TRAIL_LENGTH);
            }
        }
    }

    /// Reacts to input: any key quits, and a click adds a dot.
    fn handle(&mut self, event: &Event) -> Action {
        match event {
            Event::Key(_) => Action::Quit,
            Event::Mouse(mouse) if matches!(mouse.kind, MouseKind::Down(_)) => {
                self.add_dot(mouse.x as f32, mouse.y as f32);
                Action::Continue
            }
            Event::Mouse(_) => Action::Continue,
            // pasted text is not a key press, and more kinds of event may come
            _ => Action::Continue,
        }
    }

    /// Draws the trails into `display`, fresher parts as denser blocks.
    fn render(&self, display: &mut Display) {
        for y in 0..self.height {
            for x in 0..self.width {
                let (colour, frames_left) = self.trail[(y * self.width + x) as usize];
                let block = match frames_left {
                    0 => ' ',
                    n if n == TRAIL_LENGTH => '█',
                    n if n * 3 >= TRAIL_LENGTH * 2 => '▓',
                    n if n * 3 >= TRAIL_LENGTH => '▒',
                    _ => '░',
                };
                display.set_pixel(x, y, block, colour, Colour::Black);
            }
        }
    }
}

/// Keeps `position` between 0 and `max`, turning `speed` around at a wall.
fn bounce(position: &mut f32, speed: &mut f32, max: f32) {
    if *position < 0.0 {
        *position = -*position;
        *speed = -*speed;
    } else if *position > max {
        *position = 2.0 * max - *position;
        *speed = -*speed;
    }
    // a window narrower than a step would otherwise throw the dot out
    *position = position.clamp(0.0, max);
}

fn main() -> io::Result<()> {
    let mut terminal = Terminal::start()?;

    // `draw` needs a display at most one row shorter than the window
    let (width, height) = terminal.size();
    let mut display = Display::new(width, height.saturating_sub(1));

    // any number will do as a seed; the clock makes each run different
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |since| since.subsec_nanos())
        ^ std::process::id();
    let mut saver = Screensaver::new(display.width(), display.height(), seed);

    let mut next_frame = Instant::now() + FRAME;
    loop {
        saver.render(&mut display);
        terminal.draw(&display)?;

        // spend the time until the next frame handling input
        loop {
            let wait = next_frame.saturating_duration_since(Instant::now());
            match terminal.poll(wait)? {
                Some(event) => {
                    if saver.handle(&event) == Action::Quit {
                        return Ok(());
                    }
                }
                None => break,
            }
        }

        saver.step();
        // if we fell behind, do not try to catch up with a burst of frames
        next_frame = (next_frame + FRAME).max(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics_interactive::{Key, KeyCode, Modifiers, Mouse, MouseButton};

    /// A screensaver with no dots, so tests control everything.
    fn empty(width: u32, height: u32) -> Screensaver {
        let mut saver = Screensaver::new(width, height, 1);
        saver.dots.clear();
        saver
    }

    fn dot(x: f32, y: f32, dx: f32, dy: f32) -> Dot {
        Dot {
            x,
            y,
            dx,
            dy,
            colour: Colour::BrightRed,
        }
    }

    fn click(x: isize, y: isize) -> Event {
        Event::Mouse(Mouse {
            kind: MouseKind::Down(MouseButton::Left),
            x,
            y,
            modifiers: Modifiers::NONE,
        })
    }

    fn cell(saver: &Screensaver, x: isize, y: isize) -> (Colour, u8) {
        saver.trail[(y * saver.width + x) as usize]
    }

    #[test]
    fn a_dot_moves_by_its_speed_each_step() {
        let mut saver = empty(40, 20);
        saver.dots.push(dot(10.0, 5.0, 0.5, 0.25));
        saver.step();
        assert_eq!((saver.dots[0].x, saver.dots[0].y), (10.5, 5.25));
    }

    #[test]
    fn a_dot_bounces_off_each_wall() {
        let mut saver = empty(10, 6);
        saver.dots.push(dot(8.8, 2.0, 0.5, 0.0));
        saver.step();
        assert!(saver.dots[0].dx < 0.0, "right wall");
        assert!(saver.dots[0].x <= 9.0);

        let mut saver = empty(10, 6);
        saver.dots.push(dot(0.2, 2.0, -0.5, 0.0));
        saver.step();
        assert!(saver.dots[0].dx > 0.0, "left wall");
        assert!(saver.dots[0].x >= 0.0);

        let mut saver = empty(10, 6);
        saver.dots.push(dot(3.0, 0.1, 0.0, -0.4));
        saver.step();
        assert!(saver.dots[0].dy > 0.0, "top wall");
        assert!(saver.dots[0].y >= 0.0);

        let mut saver = empty(10, 6);
        saver.dots.push(dot(3.0, 4.9, 0.0, 0.4));
        saver.step();
        assert!(saver.dots[0].dy < 0.0, "bottom wall");
        assert!(saver.dots[0].y <= 5.0);
    }

    #[test]
    fn dots_never_leave_the_window_however_long_it_runs() {
        let mut saver = Screensaver::new(30, 10, 12345);
        for _ in 0..5000 {
            saver.step();
            for dot in &saver.dots {
                assert!((0.0..=29.0).contains(&dot.x), "x = {}", dot.x);
                assert!((0.0..=9.0).contains(&dot.y), "y = {}", dot.y);
            }
        }
    }

    #[test]
    fn the_cell_under_a_dot_gets_a_fresh_trail_that_then_fades_away() {
        let mut saver = empty(20, 10);
        saver.dots.push(dot(5.0, 5.0, 0.0, 0.0));
        saver.step();
        assert_eq!(cell(&saver, 5, 5), (Colour::BrightRed, TRAIL_LENGTH));

        saver.dots.clear();
        for step in 1..=TRAIL_LENGTH {
            saver.step();
            assert_eq!(cell(&saver, 5, 5).1, TRAIL_LENGTH - step);
        }
        saver.step();
        assert_eq!(cell(&saver, 5, 5).1, 0, "it stays gone");
    }

    #[test]
    fn a_moving_dot_leaves_a_trail_that_is_fresher_near_the_dot() {
        let mut saver = empty(40, 10);
        saver.dots.push(dot(2.0, 5.0, 1.0, 0.0));
        for _ in 0..5 {
            saver.step();
        }
        assert!(cell(&saver, 7, 5).1 > cell(&saver, 4, 5).1);
        assert!(cell(&saver, 4, 5).1 > cell(&saver, 3, 5).1);
    }

    #[test]
    fn clicking_adds_a_dot_there_and_the_count_is_capped() {
        let mut saver = empty(40, 20);
        assert_eq!(saver.handle(&click(7, 3)), Action::Continue);
        assert_eq!(saver.dots.len(), 1);
        assert_eq!((saver.dots[0].x, saver.dots[0].y), (7.0, 3.0));

        for _ in 0..(MAX_DOTS * 2) {
            saver.handle(&click(7, 3));
        }
        assert_eq!(saver.dots.len(), MAX_DOTS);
    }

    #[test]
    fn a_click_outside_the_window_is_clamped_inside_it() {
        let mut saver = empty(10, 6);
        saver.handle(&click(500, -500));
        assert_eq!((saver.dots[0].x, saver.dots[0].y), (9.0, 0.0));
    }

    #[test]
    fn new_dots_get_a_direction_and_are_not_stuck() {
        let mut saver = Screensaver::new(40, 20, 99);
        assert_eq!(saver.dots.len(), 4);
        for dot in &saver.dots {
            assert!(dot.dx.abs() > 0.05 || dot.dy.abs() > 0.05, "a stuck dot");
            assert!(dot.dx.abs() <= 1.0 && dot.dy.abs() <= 0.5);
        }
        let colours: std::collections::HashSet<_> = saver.dots.iter().map(|d| d.colour).collect();
        assert!(colours.len() > 1, "all the same colour");
        saver.add_dot(1.0, 1.0);
    }

    #[test]
    fn any_key_quits_but_mouse_and_movement_do_not() {
        let mut saver = empty(10, 6);
        for key in [
            KeyCode::Char('x'),
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Up,
        ] {
            assert_eq!(
                saver.handle(&Event::Key(Key::new(key))),
                Action::Quit,
                "{key:?}"
            );
        }
        assert_eq!(saver.handle(&click(1, 1)), Action::Continue);
        let scroll = Event::Mouse(Mouse {
            kind: MouseKind::ScrollUp,
            x: 0,
            y: 0,
            modifiers: Modifiers::NONE,
        });
        assert_eq!(saver.handle(&scroll), Action::Continue);
    }

    #[test]
    fn pasted_text_neither_quits_nor_adds_dots() {
        let mut saver = empty(10, 6);
        assert_eq!(
            saver.handle(&Event::Paste("anything".into())),
            Action::Continue
        );
        assert!(saver.dots.is_empty());
    }

    #[test]
    fn render_draws_fading_blocks_in_the_trail_colour_and_black_elsewhere() {
        let mut saver = empty(20, 5);
        saver.trail[0] = (Colour::BrightCyan, TRAIL_LENGTH);
        saver.trail[1] = (Colour::BrightCyan, 1);
        let mut display = Display::new(20, 5);
        saver.render(&mut display);

        let fresh = display.pixel(0, 0).unwrap();
        assert_eq!(fresh.character(), '█');
        assert_eq!(fresh.colour(), Colour::BrightCyan);
        let fading = display.pixel(1, 0).unwrap();
        assert_ne!(fading.character(), '█');
        assert_ne!(fading.character(), ' ');
        let empty_cell = display.pixel(2, 0).unwrap();
        assert_eq!(empty_cell.character(), ' ');
        assert_eq!(empty_cell.background(), Colour::Black);
    }

    #[test]
    fn a_window_of_any_size_works_including_nothing() {
        for (w, h) in [(0, 0), (0, 5), (5, 0), (1, 1), (2, 1)] {
            let mut saver = Screensaver::new(w, h, 7);
            let mut display = Display::new(w, h);
            for _ in 0..50 {
                saver.step();
                saver.render(&mut display);
            }
            saver.handle(&click(3, 3));
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_animation() {
        let mut a = Screensaver::new(30, 10, 5);
        let mut b = Screensaver::new(30, 10, 5);
        for _ in 0..100 {
            a.step();
            b.step();
        }
        let positions = |s: &Screensaver| s.dots.iter().map(|d| (d.x, d.y)).collect::<Vec<_>>();
        assert_eq!(positions(&a), positions(&b));
    }
}

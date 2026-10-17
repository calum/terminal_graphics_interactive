//! [`Headless`]: play a game with no terminal, for tests, replays and AI agents.

use super::scene::{time_of, Scenes};
use super::{Game, Keys, Screen};
use crate::{Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind, Pointer};

/// Plays a [`Game`] with no terminal: you press keys and click, you step time,
/// and you read what the game drew. It runs the same updates as the real
/// [`run`](super::run) loop, so a test of a headless game tests the game.
///
/// Time moves only when you call [`step`](Headless::step) (or
/// [`tap`](Headless::tap)), one update per tick, so a run is the same every
/// time.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// struct Counter {
///     count: u32,
/// }
///
/// impl Game for Counter {
///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
///         if ctx.keys.held(KeyCode::Right) {
///             self.count += 1;
///         }
///         Next::stay()
///     }
///     fn draw(&self, screen: &mut Screen) {
///         screen.text(0, 0, &format!("count {}", self.count), Colour::White);
///     }
/// }
///
/// let mut game = Headless::new(Counter { count: 0 }).size(12, 1);
/// game.press(KeyCode::Right);
/// game.step(3);
/// game.release(KeyCode::Right);
/// game.step(3);
/// assert_eq!(game.text(), "count 3");
/// ```
pub struct Headless {
    scenes: Scenes,
    keys: Keys,
    pointer: Pointer,
    tick: u64,
    seed: u64,
    screen: Screen,
}

impl Headless {
    /// A headless game of 80 by 24 cells with seed 1. A key pressed stays held
    /// until you call [`release`](Headless::release).
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Hello;
    /// impl Game for Hello {
    ///     fn draw(&self, screen: &mut Screen) {
    ///         screen.text(0, 0, "hello", Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Hello);
    /// assert_eq!(game.text().lines().next(), Some("hello"));
    /// ```
    pub fn new(game: impl Game + 'static) -> Headless {
        Headless {
            scenes: Scenes::new(Box::new(game)),
            keys: Keys::new().real_release(true),
            pointer: Pointer::default(),
            tick: 0,
            seed: 1,
            screen: Screen::new(80, 24),
        }
    }

    /// Makes the screen `width` columns by `height` rows (the default is 80
    /// by 24). Give the size your real terminal has, to see the same layout.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Hello;
    /// impl Game for Hello {}
    ///
    /// let mut game = Headless::new(Hello).size(20, 5);
    /// assert_eq!(game.screen().size(), (20, 5));
    /// ```
    pub fn size(mut self, width: u32, height: u32) -> Headless {
        self.screen = Screen::new(width, height);
        self
    }

    /// Uses `seed` as the run's random seed (see [`Context::seed`]). The same
    /// seed and the same inputs always give the same game.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Seeded;
    /// impl Game for Seeded {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         assert_eq!(ctx.seed, 99);
    ///         Next::quit()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Seeded).seed(99);
    /// game.step(1);
    /// assert!(game.quit());
    /// ```
    pub fn seed(mut self, seed: u64) -> Headless {
        self.seed = seed;
        self
    }

    /// Presses `code` before the next update. It stays held until
    /// [`release`](Headless::release).
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Hold(u32);
    /// impl Game for Hold {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.held(KeyCode::Right) {
    ///             self.0 += 1;
    ///         }
    ///         Next::stay()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Hold(0));
    /// game.press(KeyCode::Right);
    /// game.step(3); // still held for all three updates
    /// game.release(KeyCode::Right);
    /// game.step(3);
    /// assert!(!game.quit());
    /// ```
    pub fn press(&mut self, code: KeyCode) {
        self.keys
            .event(&Event::Key(Key::new(code)), time_of(self.tick));
    }

    /// Lets go of `code` before the next update.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Counter { count: u32 }
    /// impl Game for Counter {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.held(KeyCode::Right) {
    ///             self.count += 1;
    ///         }
    ///         Next::stay()
    ///     }
    ///     fn draw(&self, screen: &mut Screen) {
    ///         screen.text(0, 0, &format!("count {}", self.count), Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Counter { count: 0 }).size(12, 1);
    /// game.press(KeyCode::Right);
    /// game.step(2);
    /// game.release(KeyCode::Right);
    /// game.step(5); // no longer held
    /// assert_eq!(game.text(), "count 2");
    /// ```
    pub fn release(&mut self, code: KeyCode) {
        self.keys
            .event(&Event::KeyUp(Key::new(code)), time_of(self.tick));
    }

    /// Presses and releases `code` in two updates: the game sees it pressed
    /// once, and not held afterwards.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Counter { count: u32 }
    /// impl Game for Counter {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.pressed(KeyCode::Right) {
    ///             self.count += 1;
    ///         }
    ///         Next::stay()
    ///     }
    ///     fn draw(&self, screen: &mut Screen) {
    ///         screen.text(0, 0, &format!("count {}", self.count), Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Counter { count: 0 }).size(12, 1);
    /// game.tap(KeyCode::Right); // one press
    /// game.tap(KeyCode::Right); // another press
    /// assert_eq!(game.text(), "count 2");
    /// ```
    pub fn tap(&mut self, code: KeyCode) {
        self.press(code);
        self.step(1);
        self.release(code);
        self.step(1);
    }

    /// Moves the mouse pointer to cell `(x, y)` before the next update. The
    /// position stays until the mouse moves again.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Watch;
    /// impl Game for Watch {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         assert_eq!((ctx.pointer.x, ctx.pointer.y), (4.0, 2.0));
    ///         Next::quit()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Watch);
    /// game.pointer_to(4, 2);
    /// game.step(1);
    /// assert!(game.quit());
    /// ```
    pub fn pointer_to(&mut self, x: i32, y: i32) {
        self.pointer.update(&Mouse {
            kind: MouseKind::Moved,
            x: x as isize,
            y: y as isize,
            modifiers: Modifiers::NONE,
        });
    }

    /// Clicks the left button at cell `(x, y)`. The game sees the click in
    /// `ctx.pointer.clicks` for exactly one update.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// // a game that puts a dot wherever the pointer was clicked
    /// struct Dots {
    ///     dots: Vec<(f32, f32)>,
    /// }
    /// impl Game for Dots {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         self.dots.extend(ctx.pointer.clicks.iter().copied());
    ///         Next::stay()
    ///     }
    ///     fn draw(&self, screen: &mut Screen) {
    ///         for &(x, y) in &self.dots {
    ///             screen.put(x as i32, y as i32, '*', Colour::White);
    ///         }
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Dots { dots: Vec::new() }).size(10, 3);
    /// game.click(2, 1);
    /// game.step(2); // the click is seen once, on the first update
    /// assert_eq!(game.text(), "\n  *\n");
    /// ```
    pub fn click(&mut self, x: i32, y: i32) {
        self.pointer_to(x, y);
        self.pointer.update(&Mouse {
            kind: MouseKind::Down(MouseButton::Left),
            x: x as isize,
            y: y as isize,
            modifiers: Modifiers::NONE,
        });
        self.pointer.update(&Mouse {
            kind: MouseKind::Up(MouseButton::Left),
            x: x as isize,
            y: y as isize,
            modifiers: Modifiers::NONE,
        });
    }

    /// Feeds one mouse event to the pointer, exactly as the real loop does.
    /// Use it for buttons held down and drags, which [`click`](Headless::click)
    /// and [`pointer_to`](Headless::pointer_to) cannot do. A button goes down
    /// with `Down` and stays down until `Up`, and `ctx.pointer.left` is true on
    /// every update in between.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// // counts the updates with the button down, and remembers where the pointer is
    /// struct Drag {
    ///     held: u32,
    ///     at: (f32, f32),
    /// }
    /// impl Game for Drag {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.pointer.left {
    ///             self.held += 1;
    ///         }
    ///         self.at = (ctx.pointer.x, ctx.pointer.y);
    ///         Next::stay()
    ///     }
    ///     fn draw(&self, screen: &mut Screen) {
    ///         let (x, y) = self.at;
    ///         screen.text(0, 0, &format!("held {} at {} {}", self.held, x, y), Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Drag { held: 0, at: (0.0, 0.0) }).size(30, 1);
    /// game.mouse(&Mouse {
    ///     kind: MouseKind::Down(MouseButton::Left),
    ///     x: 1,
    ///     y: 2,
    ///     modifiers: Modifiers::NONE,
    /// });
    /// game.step(2);
    /// game.mouse(&Mouse {
    ///     kind: MouseKind::Drag(MouseButton::Left),
    ///     x: 5,
    ///     y: 2,
    ///     modifiers: Modifiers::NONE,
    /// });
    /// game.step(1);
    /// game.mouse(&Mouse {
    ///     kind: MouseKind::Up(MouseButton::Left),
    ///     x: 5,
    ///     y: 2,
    ///     modifiers: Modifiers::NONE,
    /// });
    /// game.step(1);
    /// assert_eq!(game.text(), "held 3 at 5 2");
    /// ```
    pub fn mouse(&mut self, mouse: &Mouse) {
        self.pointer.update(mouse);
    }

    /// Runs `ticks` updates, or fewer if the game quits first.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Leave;
    /// impl Game for Leave {
    ///     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
    ///         Next::quit()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Leave);
    /// game.step(10); // stops after the first update, which quits
    /// assert_eq!(game.tick(), 1);
    /// assert!(game.quit());
    /// ```
    pub fn step(&mut self, ticks: u32) {
        for _ in 0..ticks {
            if self.scenes.is_empty() {
                break;
            }
            let alive = self.scenes.step(
                &mut self.keys,
                &mut self.pointer,
                self.tick,
                self.screen.size(),
                self.seed,
            );
            self.tick += 1;
            if !alive {
                break;
            }
        }
    }

    /// What the game draws now, as a [`Screen`]. Drawing does not change the
    /// game, so you can call it at any time.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Hello;
    /// impl Game for Hello {
    ///     fn draw(&self, screen: &mut Screen) {
    ///         screen.text(0, 0, "hi", Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Hello).size(6, 2);
    /// assert_eq!(game.screen().size(), (6, 2));
    /// assert_eq!(game.screen().display().pixel(1, 0).unwrap().character(), 'i');
    /// ```
    pub fn screen(&mut self) -> &Screen {
        self.scenes.draw(&mut self.screen, self.tick);
        &self.screen
    }

    /// What the game draws now, as text: one line per row, with trailing
    /// spaces removed. This is how a test or an AI agent sees the game.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Hello;
    /// impl Game for Hello {
    ///     fn draw(&self, screen: &mut Screen) {
    ///         screen.text(0, 0, "hi", Colour::White);
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Hello).size(4, 2);
    /// assert_eq!(game.text(), "hi\n");
    /// ```
    pub fn text(&mut self) -> String {
        let screen = self.screen();
        let (width, height) = screen.size();
        let display = screen.display();
        let mut out = String::new();
        for y in 0..height {
            if y > 0 {
                out.push('\n');
            }
            let mut row = String::with_capacity(width as usize);
            for x in 0..width {
                let ch = display
                    .pixel(x as isize, y as isize)
                    .map_or(' ', |pixel| pixel.character());
                row.push(ch);
            }
            out.push_str(row.trim_end_matches(' '));
        }
        out
    }

    /// Whether the game has ended.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Leave;
    /// impl Game for Leave {
    ///     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
    ///         Next::quit()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Leave);
    /// assert!(!game.quit());
    /// game.step(1);
    /// assert!(game.quit());
    /// ```
    pub fn quit(&self) -> bool {
        self.scenes.is_empty()
    }

    /// How many updates have run.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Idle;
    /// impl Game for Idle {}
    ///
    /// let mut game = Headless::new(Idle);
    /// game.step(90);
    /// assert_eq!(game.tick(), 90); // 90 updates is 1.5 seconds of game time
    /// ```
    pub fn tick(&self) -> u64 {
        self.tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Context, Next, Rng};
    use crate::Colour;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    type Shared<T> = Rc<RefCell<T>>;

    /// Counts how many updates saw Right as pressed.
    struct Presses {
        count: Rc<Cell<u32>>,
    }

    impl Game for Presses {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            if ctx.keys.pressed(KeyCode::Right) {
                self.count.set(self.count.get() + 1);
            }
            Next::Stay
        }
    }

    #[test]
    fn a_tap_is_one_press_that_the_game_sees() {
        let count = Rc::new(Cell::new(0));
        let mut game = Headless::new(Presses {
            count: Rc::clone(&count),
        });
        game.tap(KeyCode::Right);
        assert_eq!(count.get(), 1);
    }

    /// Counts how many updates saw Right as held.
    struct Holds {
        held: Rc<Cell<u32>>,
    }

    impl Game for Holds {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            if ctx.keys.held(KeyCode::Right) {
                self.held.set(self.held.get() + 1);
            }
            Next::Stay
        }
    }

    #[test]
    fn a_held_key_is_seen_on_every_update_until_it_is_released() {
        let held = Rc::new(Cell::new(0));
        let mut game = Headless::new(Holds {
            held: Rc::clone(&held),
        });
        game.press(KeyCode::Right);
        game.step(30);
        assert_eq!(held.get(), 30);
        game.release(KeyCode::Right);
        game.step(30);
        assert_eq!(held.get(), 30, "no update sees it held after release");
    }

    #[test]
    fn a_tap_is_held_for_one_update_only() {
        let held = Rc::new(Cell::new(0));
        let mut game = Headless::new(Holds {
            held: Rc::clone(&held),
        });
        game.tap(KeyCode::Right); // updates: pressed and held, then released
        game.step(5);
        assert_eq!(held.get(), 1, "a tap is not held after its release");
    }

    /// Records every click the updates see, with the update that saw it.
    struct Clicks {
        seen: Shared<Vec<(usize, (f32, f32))>>,
    }

    impl Game for Clicks {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            for &click in &ctx.pointer.clicks {
                self.seen.borrow_mut().push((ctx.tick as usize, click));
            }
            Next::Stay
        }
    }

    #[test]
    fn a_click_reaches_the_pointer_for_exactly_one_update() {
        let seen: Shared<Vec<(usize, (f32, f32))>> = Rc::new(RefCell::new(Vec::new()));
        let mut game = Headless::new(Clicks {
            seen: Rc::clone(&seen),
        });
        game.click(3, 4);
        game.step(3);
        assert_eq!(*seen.borrow(), vec![(0, (3.0, 4.0))]);
    }

    /// Records whether the left button was down on each update.
    struct LeftDown(Shared<Vec<bool>>);

    impl Game for LeftDown {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            self.0.borrow_mut().push(ctx.pointer.left);
            Next::Stay
        }
    }

    #[test]
    fn a_mouse_button_held_down_is_seen_on_every_update_until_it_is_up() {
        let seen: Shared<Vec<bool>> = Rc::new(RefCell::new(Vec::new()));
        let mut game = Headless::new(LeftDown(Rc::clone(&seen)));
        game.mouse(&Mouse {
            kind: MouseKind::Down(MouseButton::Left),
            x: 1,
            y: 1,
            modifiers: Modifiers::NONE,
        });
        game.step(3);
        game.mouse(&Mouse {
            kind: MouseKind::Up(MouseButton::Left),
            x: 1,
            y: 1,
            modifiers: Modifiers::NONE,
        });
        game.step(2);
        assert_eq!(*seen.borrow(), vec![true, true, true, false, false]);
    }

    /// Records the pointer position on each update.
    struct PointerAt(Shared<Vec<(f32, f32)>>);

    impl Game for PointerAt {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            self.0.borrow_mut().push((ctx.pointer.x, ctx.pointer.y));
            Next::Stay
        }
    }

    #[test]
    fn a_drag_moves_the_pointer() {
        let seen: Shared<Vec<(f32, f32)>> = Rc::new(RefCell::new(Vec::new()));
        let mut game = Headless::new(PointerAt(Rc::clone(&seen)));
        game.mouse(&Mouse {
            kind: MouseKind::Drag(MouseButton::Left),
            x: 7,
            y: 3,
            modifiers: Modifiers::NONE,
        });
        game.step(1);
        assert_eq!(*seen.borrow(), vec![(7.0, 3.0)]);
    }

    #[test]
    fn a_mouse_down_is_one_click_that_the_game_sees_for_one_update() {
        let seen: Shared<Vec<(usize, (f32, f32))>> = Rc::new(RefCell::new(Vec::new()));
        let mut game = Headless::new(Clicks {
            seen: Rc::clone(&seen),
        });
        game.mouse(&Mouse {
            kind: MouseKind::Down(MouseButton::Left),
            x: 6,
            y: 2,
            modifiers: Modifiers::NONE,
        });
        game.step(3);
        assert_eq!(*seen.borrow(), vec![(0, (6.0, 2.0))]);
    }

    /// Draws one line of text at row 1.
    struct Hello;

    impl Game for Hello {
        fn draw(&self, screen: &mut Screen) {
            screen.text(2, 1, "hi", Colour::White);
        }
    }

    #[test]
    fn text_is_what_the_game_drew_one_row_per_line_with_trailing_spaces_trimmed() {
        let mut game = Headless::new(Hello);
        let text = game.text();
        let rows: Vec<&str> = text.split('\n').collect();
        assert_eq!(rows.len(), 24);
        assert_eq!(rows[0], "");
        assert_eq!(rows[1], "  hi");
    }

    /// Quits on the update whose tick is `at`.
    struct QuitAt(u64);

    impl Game for QuitAt {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            if ctx.tick == self.0 {
                Next::quit()
            } else {
                Next::stay()
            }
        }
    }

    #[test]
    fn step_stops_as_soon_as_the_game_quits() {
        let mut game = Headless::new(QuitAt(3));
        assert!(!game.quit());
        game.step(100);
        assert!(game.quit());
        assert_eq!(game.tick(), 4, "updates 0 to 3 ran, then it stopped");
    }

    /// Remembers the seed it was given.
    struct SeedSeen(Rc<Cell<u64>>);

    impl Game for SeedSeen {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            self.0.set(ctx.seed);
            Next::Stay
        }
    }

    #[test]
    fn the_game_is_given_the_seed_it_was_built_with() {
        let seen = Rc::new(Cell::new(0));
        let mut game = Headless::new(SeedSeen(Rc::clone(&seen))).seed(42);
        game.step(1);
        assert_eq!(seen.get(), 42);
    }

    /// Draws a mark in the bottom right corner.
    struct Corner;

    impl Game for Corner {
        fn draw(&self, screen: &mut Screen) {
            let (width, height) = screen.size();
            screen.put(width as i32 - 1, height as i32 - 1, '#', Colour::White);
        }
    }

    #[test]
    fn size_sets_the_screen_the_game_draws_on() {
        let mut game = Headless::new(Corner).size(10, 3);
        let text = game.text();
        let rows: Vec<&str> = text.split('\n').collect();
        assert_eq!(rows, vec!["", "", "         #"]);
    }

    /// Rolls dice with the run's seed, and shows the rolls on screen.
    struct Dice {
        rng: Option<Rng>,
        state: Rc<Cell<u64>>,
        rolls: u32,
        last: i32,
    }

    impl Game for Dice {
        fn update(&mut self, ctx: &mut Context<'_>) -> Next {
            let rng = self.rng.get_or_insert_with(|| Rng::new(ctx.seed));
            if rng.chance(0.3) {
                self.rolls += 1;
                self.last = rng.range_i32(1, 6);
            }
            self.state.set(rng.state());
            Next::Stay
        }

        fn draw(&self, screen: &mut Screen) {
            screen.text(
                0,
                0,
                &format!("rolls {} last {}", self.rolls, self.last),
                Colour::White,
            );
        }
    }

    /// Plays 600 ticks with the same scripted inputs, and returns what was
    /// drawn and the RNG state at the end.
    fn play(seed: u64) -> (String, u64) {
        let state = Rc::new(Cell::new(0));
        let mut game = Headless::new(Dice {
            rng: None,
            state: Rc::clone(&state),
            rolls: 0,
            last: 0,
        })
        .seed(seed);
        for _ in 0..6 {
            game.tap(KeyCode::Right); // two ticks
            game.step(98);
        }
        assert_eq!(game.tick(), 600);
        (game.text(), state.get())
    }

    #[test]
    fn the_same_seed_and_inputs_give_the_same_game_after_600_ticks() {
        let first = play(7);
        assert_eq!(first, play(7));
        assert_ne!(play(8).0, first.0, "another seed plays a different game");
    }
}

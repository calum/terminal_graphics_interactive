//! Scenes: the stack of screens a game is made of, and the one update path
//! that both the real loop and the headless runner use.

use super::{Context, Keys, Screen};
use crate::Pointer;
use std::time::Duration;

/// One update is 1/60 of a second.
pub(crate) const STEP: Duration = Duration::from_nanos(16_666_667);

/// One update in seconds, as `Context::dt`.
pub(crate) const DT: f32 = 1.0 / 60.0;

/// A screen of your game: it updates, and it draws itself.
///
/// Both methods have defaults, so a screen can be only a `draw` or only an
/// `update`. Implement it on a struct that holds the game's state.
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
/// ```
pub trait Game {
    /// Runs once per update (60 times a second). Change the state here and
    /// return what should happen next.
    fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
        Next::Stay
    }

    /// Draws the current state. It takes `&self`, so drawing cannot change the
    /// game. Clear the screen first if the background should be plain.
    fn draw(&self, _screen: &mut Screen) {}
}

/// What happens after a game's `update`. Build one with the constructors:
/// [`Next::stay`], [`Next::switch`], [`Next::push`], [`Next::pop`] and
/// [`Next::quit`].
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// struct Shop;
/// impl Game for Shop {
///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
///         if ctx.keys.pressed(KeyCode::Esc) {
///             Next::pop() // leave the shop
///         } else {
///             Next::stay()
///         }
///     }
/// }
///
/// let mut game = Headless::new(Shop);
/// game.tap(KeyCode::Esc);
/// assert!(game.quit());
/// ```
pub enum Next {
    /// Keep the current scene.
    Stay,
    /// Replace the current scene with this one.
    Switch(Box<dyn Game>),
    /// Put this scene on top; the one below is kept and paused.
    Push(Box<dyn Game>),
    /// Remove the current scene and go back to the one below.
    Pop,
    /// End the game.
    Quit,
}

impl Next {
    /// Keep the current scene as it is. This is what `update` returns on most
    /// updates.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Still;
    /// impl Game for Still {
    ///     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
    ///         Next::stay()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Still);
    /// game.step(60);
    /// assert!(!game.quit());
    /// ```
    pub fn stay() -> Next {
        Next::Stay
    }

    /// Replace the current scene with `game`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Menu;
    /// struct Play;
    /// impl Game for Menu {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.pressed(KeyCode::Enter) { Next::switch(Play) } else { Next::stay() }
    ///     }
    /// }
    /// impl Game for Play {}
    ///
    /// let mut game = Headless::new(Menu);
    /// game.tap(KeyCode::Enter);
    /// assert!(!game.quit());
    /// ```
    pub fn switch(game: impl Game + 'static) -> Next {
        Next::Switch(Box::new(game))
    }

    /// Put `game` on top of the current scene. The scene below is kept and
    /// does not update until `Next::pop()` brings it back to the top.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Play;
    /// struct Pause;
    /// impl Game for Play {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.pressed(KeyCode::Esc) { Next::push(Pause) } else { Next::stay() }
    ///     }
    /// }
    /// impl Game for Pause {
    ///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
    ///         if ctx.keys.pressed(KeyCode::Esc) { Next::pop() } else { Next::stay() }
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Play);
    /// game.tap(KeyCode::Esc); // pause
    /// game.tap(KeyCode::Esc); // resume
    /// assert!(!game.quit());
    /// ```
    pub fn push(game: impl Game + 'static) -> Next {
        Next::Push(Box::new(game))
    }

    /// Remove the current scene and go back to the one below. Popping the
    /// last scene ends the game.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Done;
    /// impl Game for Done {
    ///     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
    ///         Next::pop()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Done);
    /// game.step(1);
    /// assert!(game.quit());
    /// ```
    pub fn pop() -> Next {
        Next::Pop
    }

    /// End the game now, whatever scenes are on the stack.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// struct Exit;
    /// impl Game for Exit {
    ///     fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
    ///         Next::quit()
    ///     }
    /// }
    ///
    /// let mut game = Headless::new(Exit);
    /// game.step(5);
    /// assert!(game.quit());
    /// assert_eq!(game.tick(), 1, "no updates run after quitting");
    /// ```
    pub fn quit() -> Next {
        Next::Quit
    }
}

/// The stack of scenes. The top scene is the one that updates.
pub(crate) struct Scenes {
    stack: Vec<Box<dyn Game>>,
}

impl Scenes {
    /// A stack holding just `first`.
    pub(crate) fn new(first: Box<dyn Game>) -> Scenes {
        Scenes { stack: vec![first] }
    }

    /// How many scenes are on the stack.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.stack.len()
    }

    /// Whether the stack is empty, which means the game has ended.
    pub(crate) fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Carries out what a scene asked for.
    pub(crate) fn apply(&mut self, next: Next) {
        match next {
            Next::Switch(game) => {
                if let Some(top) = self.stack.last_mut() {
                    *top = game;
                } else {
                    self.stack.push(game);
                }
            }
            Next::Push(game) => self.stack.push(game),
            Next::Pop => {
                self.stack.pop();
            }
            Next::Quit => self.stack.clear(),
            Next::Stay => {}
        }
    }

    /// Draws every scene, bottom to top, so a pause screen pushed over the
    /// game is drawn on top of it. Then finishes the frame, so the pixels
    /// drawn by any scene show above the text and the sprites of all of them.
    pub(crate) fn draw(&self, screen: &mut Screen, tick: u64) {
        screen.set_clock(tick as f32 * DT, tick);
        for scene in &self.stack {
            scene.draw(screen);
        }
        screen.finish_frame();
    }

    /// Runs one update of the top scene and applies what it returns.
    /// Returns `false` once no scenes are left. This is the one update path,
    /// used by both the real loop and [`Headless`](super::Headless).
    pub(crate) fn step(
        &mut self,
        keys: &mut Keys,
        pointer: &mut Pointer,
        tick: u64,
        size: (u32, u32),
        seed: u64,
    ) -> bool {
        keys.tick(time_of(tick));
        let next = match self.stack.last_mut() {
            Some(top) => {
                let mut ctx = Context {
                    keys: &*keys,
                    pointer: &*pointer,
                    dt: DT,
                    tick,
                    size,
                    seed,
                };
                top.update(&mut ctx)
            }
            None => return false,
        };
        self.apply(next);
        keys.end_tick();
        pointer.end_frame();
        !self.is_empty()
    }
}

/// The game time of update `tick`, for timing key events.
pub(crate) fn time_of(tick: u64) -> Duration {
    Duration::from_nanos((STEP.as_nanos() as u64).saturating_mul(tick))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use terminal_graphics::Colour;

    type Log = Rc<RefCell<Vec<&'static str>>>;

    /// A scene that records its name whenever it is updated.
    struct Probe {
        name: &'static str,
        log: Log,
    }

    impl Probe {
        fn boxed(name: &'static str, log: &Log) -> Box<dyn Game> {
            Box::new(Probe {
                name,
                log: Rc::clone(log),
            })
        }
    }

    impl Game for Probe {
        fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
            self.log.borrow_mut().push(self.name);
            Next::Stay
        }
    }

    fn one_update(scenes: &mut Scenes) -> bool {
        scenes.step(&mut Keys::new(), &mut Pointer::default(), 0, (10, 5), 1)
    }

    fn take(log: &Log) -> Vec<&'static str> {
        std::mem::take(&mut *log.borrow_mut())
    }

    #[test]
    fn the_frame_is_finished_after_every_scene_draws_so_pixels_show_above_text() {
        struct Floor;
        impl Game for Floor {
            fn draw(&self, screen: &mut Screen) {
                screen.pixel(0, 0, Colour::Red);
            }
        }
        struct Words;
        impl Game for Words {
            fn draw(&self, screen: &mut Screen) {
                screen.text(0, 0, "xy", Colour::White);
            }
        }
        let mut scenes = Scenes::new(Box::new(Floor));
        scenes.apply(Next::push(Words));
        let mut screen = Screen::new(2, 1);
        screen.clear(Colour::Blue);
        scenes.draw(&mut screen, 0);
        let cell = screen.display().pixel(0, 0).unwrap();
        assert_eq!(cell.character(), '\u{2580}', "the pixel is above the text");
        assert_eq!(screen.display().pixel(1, 0).unwrap().character(), 'y');
    }

    #[test]
    fn switch_replaces_the_top_scene() {
        let log: Log = Rc::default();
        let mut scenes = Scenes::new(Probe::boxed("menu", &log));
        scenes.apply(Next::switch(Probe {
            name: "play",
            log: Rc::clone(&log),
        }));
        assert_eq!(scenes.len(), 1);
        one_update(&mut scenes);
        assert_eq!(take(&log), vec!["play"]);
    }

    #[test]
    fn push_keeps_the_scene_below_and_only_the_top_updates() {
        let log: Log = Rc::default();
        let mut scenes = Scenes::new(Probe::boxed("play", &log));
        scenes.apply(Next::push(Probe {
            name: "pause",
            log: Rc::clone(&log),
        }));
        assert_eq!(scenes.len(), 2);
        one_update(&mut scenes);
        assert_eq!(take(&log), vec!["pause"], "play is paused, not updated");
    }

    #[test]
    fn pop_returns_to_the_scene_below() {
        let log: Log = Rc::default();
        let mut scenes = Scenes::new(Probe::boxed("play", &log));
        scenes.apply(Next::push(Probe {
            name: "pause",
            log: Rc::clone(&log),
        }));
        scenes.apply(Next::pop());
        assert_eq!(scenes.len(), 1);
        one_update(&mut scenes);
        assert_eq!(take(&log), vec!["play"]);
    }

    /// A scene that always asks to be popped.
    struct Leave;

    impl Game for Leave {
        fn update(&mut self, _ctx: &mut Context<'_>) -> Next {
            Next::pop()
        }
    }

    #[test]
    fn popping_the_last_scene_ends_the_game() {
        let mut scenes = Scenes::new(Box::new(Leave));
        assert!(!one_update(&mut scenes), "step reports the game is over");
        assert!(scenes.is_empty());
    }

    #[test]
    fn quit_empties_the_whole_stack() {
        let log: Log = Rc::default();
        let mut scenes = Scenes::new(Probe::boxed("play", &log));
        scenes.apply(Next::push(Probe {
            name: "pause",
            log: Rc::clone(&log),
        }));
        scenes.apply(Next::quit());
        assert!(scenes.is_empty());
    }

    /// A scene that records its name whenever it is drawn.
    struct Painter {
        name: &'static str,
        log: Log,
    }

    impl Game for Painter {
        fn draw(&self, _screen: &mut Screen) {
            self.log.borrow_mut().push(self.name);
        }
    }

    #[test]
    fn draw_calls_every_scene_from_the_bottom_up() {
        let log: Log = Rc::default();
        let mut scenes = Scenes::new(Box::new(Painter {
            name: "play",
            log: Rc::clone(&log),
        }));
        scenes.apply(Next::push(Painter {
            name: "pause",
            log: Rc::clone(&log),
        }));
        scenes.draw(&mut Screen::new(4, 2), 0);
        assert_eq!(take(&log), vec!["play", "pause"]);
    }

    /// A scene that remembers the clock it was drawn with.
    struct ClockSeen(Rc<Cell<(f32, u64)>>);

    impl Game for ClockSeen {
        fn draw(&self, screen: &mut Screen) {
            self.0.set((screen.time(), screen.tick()));
        }
    }

    #[test]
    fn draw_sets_the_screen_clock_from_the_tick() {
        let seen = Rc::new(Cell::new((0.0, 0)));
        let scenes = Scenes::new(Box::new(ClockSeen(Rc::clone(&seen))));
        scenes.draw(&mut Screen::new(4, 2), 90);
        let (time, tick) = seen.get();
        assert_eq!(tick, 90);
        assert!((time - 1.5).abs() < 1e-5, "time {time}");
    }
}

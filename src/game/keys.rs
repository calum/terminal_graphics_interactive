use crate::{Event, KeyCode};
use std::time::Duration;

/// The most keys tracked at once. Storage is allocated once, in [`Keys::new`].
const MAX_KEYS: usize = 32;

/// One key the tracker knows about, until `end_tick` forgets it.
#[derive(Clone, Copy, Debug)]
struct Tracked {
    code: KeyCode,
    /// Held right now.
    down: bool,
    /// Went down since the last `end_tick`.
    pressed: bool,
    /// Went up (or timed out) since the last `end_tick`.
    released: bool,
    /// When an emulated hold runs out. `None` means only a key release ends it.
    expires: Option<Duration>,
}

/// Which keys are pressed, held or released, timed by the caller's clock.
///
/// A terminal sends a key press when a key goes down, then repeats while it
/// stays down, and (usually) never says when it comes up. `Keys` turns that
/// stream of [`Event`]s into "is Left held right now?". Call it like this in
/// your game loop:
///
/// ```
/// use std::time::Duration;
/// use terminal_graphics_interactive::game::*;
///
/// let mut keys = Keys::new();
/// // in your loop: feed every event, then tick once per update
/// keys.event(&Event::Key(Key::new(KeyCode::Left)), Duration::from_millis(0));
/// keys.tick(Duration::from_millis(0));
/// if keys.pressed(KeyCode::Enter) {
///     // a one-shot action, such as a jump
/// }
/// assert!(keys.held(KeyCode::Left)); // movement
/// assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), -1);
/// keys.end_tick(); // at the end of the update
/// ```
///
/// **Two modes.** By default (*emulated*) a press counts as held until
/// `first_hold` (300 ms) passes with no repeat, and each repeat extends it by
/// `repeat_hold` (100 ms). When the terminal supports the kitty keyboard
/// protocol, call `real_release(true)` (with the result of
/// `Terminal::enable_key_release`): a press is then held until the terminal
/// sends [`Event::KeyUp`], and time never ends it.
///
/// **Honest limits of emulation.** A tapped key stays held for `first_hold`,
/// not for the length of the tap. A key held down can stutter once, between
/// the OS's first press and its first repeat, when the OS repeat delay is
/// longer than `first_hold`. Use [`pressed`](Keys::pressed) for one-shot
/// actions such as jumping, [`held`](Keys::held) for movement, and enable real
/// releases when the terminal supports them.
///
/// `Keys` reads no clock and no terminal: every time is a `Duration` since the
/// start of the game, so tests can feed it fake times. Modifiers are ignored,
/// and `Char` keys match case-insensitively. It tracks at most 32 keys at once;
/// a 33rd distinct key is ignored. Ctrl+C is just a key here: quitting is the
/// game loop's job.
#[derive(Clone, Debug)]
pub struct Keys {
    real_release: bool,
    first_hold: Duration,
    repeat_hold: Duration,
    tracked: Vec<Tracked>,
}

impl Keys {
    /// A tracker for emulated holds: 300 ms for the first hold, 100 ms per
    /// repeat.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Right)), Duration::from_millis(0));
    /// keys.tick(Duration::from_millis(100));
    /// assert!(keys.held(KeyCode::Right));
    /// ```
    pub fn new() -> Keys {
        Keys {
            real_release: false,
            first_hold: Duration::from_millis(300),
            repeat_hold: Duration::from_millis(100),
            tracked: Vec::with_capacity(MAX_KEYS),
        }
    }

    /// Use real key releases (`true`) or emulated holds (`false`, the default).
    /// Pass the result of `Terminal::enable_key_release`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new().real_release(true);
    /// keys.event(&Event::Key(Key::new(KeyCode::Up)), Duration::from_millis(0));
    /// keys.tick(Duration::from_secs(60));
    /// assert!(keys.held(KeyCode::Up)); // no timeout any more
    /// keys.event(&Event::KeyUp(Key::new(KeyCode::Up)), Duration::from_secs(61));
    /// assert!(!keys.held(KeyCode::Up));
    /// ```
    pub fn real_release(mut self, on: bool) -> Keys {
        self.real_release = on;
        self
    }

    /// How long an emulated hold lasts after the first press (`first`) and
    /// after each repeat (`repeat`). Only matters when real releases are off.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// // a slow OS repeat delay needs a longer first hold
    /// let mut keys = Keys::new().timeouts(Duration::from_millis(500), Duration::from_millis(80));
    /// keys.event(&Event::Key(Key::new(KeyCode::Left)), Duration::from_millis(0));
    /// keys.tick(Duration::from_millis(400));
    /// assert!(keys.held(KeyCode::Left)); // still inside the 500 ms first hold
    /// ```
    pub fn timeouts(mut self, first: Duration, repeat: Duration) -> Keys {
        self.first_hold = first;
        self.repeat_hold = repeat;
        self
    }

    /// Feeds one terminal event at time `now`. Key presses, repeats and key
    /// releases are used; every other event is ignored, so you can pass
    /// everything.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Char('w'))), Duration::from_millis(0));
    /// assert!(keys.pressed(KeyCode::Char('w')));
    /// ```
    pub fn event(&mut self, event: &Event, now: Duration) {
        match event {
            Event::Key(key) => self.press(key.code, now),
            Event::KeyUp(key) => self.release(key.code),
            _ => {}
        }
    }

    /// Ends emulated holds that ran out by `now`. Call it once at the start
    /// of each update, before reading the keys.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Down)), Duration::from_millis(0));
    /// keys.tick(Duration::from_millis(400));
    /// assert!(!keys.held(KeyCode::Down));
    /// assert!(keys.released(KeyCode::Down));
    /// ```
    pub fn tick(&mut self, now: Duration) {
        for key in self.tracked.iter_mut().filter(|key| key.down) {
            if matches!(key.expires, Some(at) if at <= now) {
                key.down = false;
                key.released = true;
                key.expires = None;
            }
        }
    }

    /// Whether `code` went down since the last [`end_tick`](Keys::end_tick).
    /// Repeats do not count. Use it for one-shot actions.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Enter)), Duration::from_millis(0));
    /// assert!(keys.pressed(KeyCode::Enter));
    /// keys.end_tick();
    /// assert!(!keys.pressed(KeyCode::Enter));
    /// ```
    pub fn pressed(&self, code: KeyCode) -> bool {
        self.find(code).is_some_and(|key| key.pressed)
    }

    /// Whether `code` is down now. Use it for movement.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Char(' '))), Duration::from_millis(0));
    /// assert!(keys.held(KeyCode::Char(' ')));
    /// ```
    pub fn held(&self, code: KeyCode) -> bool {
        self.find(code).is_some_and(|key| key.down)
    }

    /// Whether `code` went up (or timed out) since the last
    /// [`end_tick`](Keys::end_tick).
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new().real_release(true);
    /// keys.event(&Event::Key(Key::new(KeyCode::Left)), Duration::from_millis(0));
    /// keys.event(&Event::KeyUp(Key::new(KeyCode::Left)), Duration::from_millis(50));
    /// assert!(keys.released(KeyCode::Left));
    /// ```
    pub fn released(&self, code: KeyCode) -> bool {
        self.find(code).is_some_and(|key| key.released)
    }

    /// `held(positive) - held(negative)`: `-1`, `0` or `1`. Zero when neither
    /// or both are held.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Right)), Duration::from_millis(0));
    /// assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), 1);
    /// ```
    pub fn axis(&self, negative: KeyCode, positive: KeyCode) -> i32 {
        i32::from(self.held(positive)) - i32::from(self.held(negative))
    }

    /// Forgets the `pressed` and `released` flags, and keys that are up. Call
    /// it once at the end of each update.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Up)), Duration::from_millis(0));
    /// keys.end_tick();
    /// assert!(keys.held(KeyCode::Up));
    /// assert!(!keys.pressed(KeyCode::Up));
    /// ```
    pub fn end_tick(&mut self) {
        for key in &mut self.tracked {
            key.pressed = false;
            key.released = false;
        }
        self.tracked.retain(|key| key.down);
    }

    fn find(&self, code: KeyCode) -> Option<&Tracked> {
        let code = normalise(code);
        self.tracked.iter().find(|key| key.code == code)
    }

    fn press(&mut self, code: KeyCode, now: Duration) {
        let code = normalise(code);
        let first = self.hold_until(now, self.first_hold);
        let repeat = self.hold_until(now, self.repeat_hold);
        if let Some(index) = self.tracked.iter().position(|key| key.code == code) {
            let key = &mut self.tracked[index];
            if key.down {
                // an OS repeat: still held, not a new press
                key.expires = repeat;
            } else {
                key.down = true;
                key.pressed = true;
                key.expires = first;
            }
        } else if self.tracked.len() < MAX_KEYS {
            self.tracked.push(Tracked {
                code,
                down: true,
                pressed: true,
                released: false,
                expires: first,
            });
        }
    }

    fn release(&mut self, code: KeyCode) {
        let code = normalise(code);
        if let Some(key) = self
            .tracked
            .iter_mut()
            .find(|key| key.code == code && key.down)
        {
            key.down = false;
            key.released = true;
            key.expires = None;
        }
    }

    fn hold_until(&self, now: Duration, span: Duration) -> Option<Duration> {
        if self.real_release {
            None
        } else {
            Some(now.saturating_add(span))
        }
    }
}

impl Default for Keys {
    fn default() -> Keys {
        Keys::new()
    }
}

/// Letters compare without case, so Shift+a and a are the same key.
fn normalise(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_lowercase().next().unwrap_or(c)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, KeyCode, Modifiers};

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn press(code: KeyCode) -> Event {
        Event::Key(Key::new(code))
    }

    fn release(code: KeyCode) -> Event {
        Event::KeyUp(Key::new(code))
    }

    #[test]
    fn a_fresh_press_is_pressed_and_held_then_only_held_after_end_tick() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), ms(0));
        assert!(keys.pressed(KeyCode::Left));
        assert!(keys.held(KeyCode::Left));
        keys.end_tick();
        assert!(!keys.pressed(KeyCode::Left));
        assert!(keys.held(KeyCode::Left));
    }

    #[test]
    fn an_emulated_press_ends_on_the_first_tick_after_first_hold() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), ms(0));
        keys.tick(ms(299));
        assert!(keys.held(KeyCode::Left));
        keys.tick(ms(300));
        assert!(!keys.held(KeyCode::Left));
        assert!(keys.released(KeyCode::Left));
    }

    #[test]
    fn an_emulated_repeat_extends_the_hold_without_pressing_again() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), ms(0));
        keys.end_tick();
        keys.event(&press(KeyCode::Left), ms(250));
        assert!(!keys.pressed(KeyCode::Left), "a repeat is not a new press");
        keys.tick(ms(300));
        assert!(
            keys.held(KeyCode::Left),
            "the repeat pushed the end to 350 ms"
        );
        keys.tick(ms(350));
        assert!(!keys.held(KeyCode::Left));
    }

    #[test]
    fn os_repeats_every_30_ms_keep_a_key_held_and_letting_go_ends_it_soon() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Right), ms(0));
        let mut t = 0;
        while t < 1000 {
            t += 30;
            keys.event(&press(KeyCode::Right), ms(t));
            keys.tick(ms(t));
            assert!(keys.held(KeyCode::Right), "held at {t} ms");
            keys.end_tick();
        }
        keys.tick(ms(t + 100));
        assert!(!keys.held(KeyCode::Right));
        assert!(keys.released(KeyCode::Right));
    }

    #[test]
    fn real_release_holds_with_no_timeout_until_key_up() {
        let mut keys = Keys::new().real_release(true);
        keys.event(&press(KeyCode::Left), ms(0));
        keys.tick(ms(60_000));
        assert!(keys.held(KeyCode::Left), "ticks never expire a real hold");
        keys.end_tick();
        keys.event(&release(KeyCode::Left), ms(60_001));
        assert!(!keys.held(KeyCode::Left));
        assert!(
            keys.released(KeyCode::Left),
            "released on the tick it went up"
        );
    }

    #[test]
    fn real_release_repeats_do_not_press_again_and_stray_key_ups_are_ignored() {
        let mut keys = Keys::new().real_release(true);
        keys.event(&press(KeyCode::Left), ms(0));
        keys.end_tick();
        keys.event(&press(KeyCode::Left), ms(30));
        assert!(!keys.pressed(KeyCode::Left));
        assert!(keys.held(KeyCode::Left));
        keys.event(&release(KeyCode::Up), ms(40));
        assert!(!keys.released(KeyCode::Up));
        assert!(!keys.held(KeyCode::Up));
        assert!(keys.held(KeyCode::Left));
    }

    #[test]
    fn a_key_up_also_releases_an_emulated_hold() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), ms(0));
        keys.event(&release(KeyCode::Left), ms(10));
        assert!(!keys.held(KeyCode::Left));
        assert!(keys.released(KeyCode::Left));
    }

    #[test]
    fn modifiers_are_ignored_and_chars_match_case_insensitively() {
        let mut keys = Keys::new();
        let shift_a = Event::Key(Key {
            code: KeyCode::Char('A'),
            modifiers: Modifiers {
                shift: true,
                alt: false,
                ctrl: false,
            },
        });
        keys.event(&shift_a, ms(0));
        assert!(keys.held(KeyCode::Char('a')));
        assert!(keys.pressed(KeyCode::Char('A')));
        let ctrl_a = Event::Key(Key {
            code: KeyCode::Char('a'),
            modifiers: Modifiers {
                shift: false,
                alt: false,
                ctrl: true,
            },
        });
        keys.event(&ctrl_a, ms(5));
        assert!(keys.held(KeyCode::Char('a')));
    }

    #[test]
    fn axis_is_minus_one_zero_or_one_and_zero_when_both_are_held() {
        let mut keys = Keys::new();
        assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), 0);
        keys.event(&press(KeyCode::Left), ms(0));
        assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), -1);
        keys.event(&press(KeyCode::Right), ms(0));
        assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), 0);
        keys.event(&release(KeyCode::Left), ms(1));
        assert_eq!(keys.axis(KeyCode::Left, KeyCode::Right), 1);
    }

    #[test]
    fn two_keys_are_held_independently() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), ms(0));
        keys.tick(ms(200));
        keys.event(&press(KeyCode::Up), ms(200));
        keys.tick(ms(350));
        assert!(!keys.held(KeyCode::Left));
        assert!(keys.held(KeyCode::Up));
    }

    #[test]
    fn at_most_32_keys_are_tracked_and_storage_never_grows() {
        let mut keys = Keys::new();
        let before = keys.tracked.capacity();
        for i in 0..1000u32 {
            let c = char::from_u32(0x4e00 + i).unwrap();
            keys.event(&press(KeyCode::Char(c)), ms(0));
        }
        assert_eq!(keys.tracked.capacity(), before);
        assert!(keys.tracked.len() <= 32);
        assert!(keys.held(char::from_u32(0x4e00).map(KeyCode::Char).unwrap()));
        assert!(!keys.held(char::from_u32(0x4e00 + 999).map(KeyCode::Char).unwrap()));
    }

    #[test]
    fn ctrl_c_is_just_a_key_the_tracker_does_not_quit() {
        let mut keys = Keys::new();
        let ctrl_c = Event::Key(Key {
            code: KeyCode::Char('c'),
            modifiers: Modifiers {
                shift: false,
                alt: false,
                ctrl: true,
            },
        });
        keys.event(&ctrl_c, ms(0));
        assert!(keys.pressed(KeyCode::Char('c')));
    }

    #[test]
    fn absurd_times_do_not_panic() {
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), Duration::MAX);
        keys.tick(Duration::MAX);
        keys.event(&press(KeyCode::Left), Duration::MAX);
        keys.tick(Duration::MAX);
        keys.end_tick();
    }
}

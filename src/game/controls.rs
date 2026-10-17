use super::Keys;
use crate::KeyCode;

/// Named actions (an enum you write) bound to keys. Players can change the
/// bindings, and the game asks about actions rather than keys:
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// #[derive(Clone, Copy, PartialEq)]
/// enum Action { Left, Right, Jump }
///
/// let controls = Controls::new()
///     .bind(Action::Left, &[KeyCode::Left, KeyCode::Char('a')])
///     .bind(Action::Right, &[KeyCode::Right, KeyCode::Char('d')])
///     .bind(Action::Jump, &[KeyCode::Char(' ')]);
///
/// let keys = Keys::new();
/// // in the update: move by the axis, jump on the press
/// let steer = controls.axis(&keys, Action::Left, Action::Right);
/// if controls.pressed(&keys, Action::Jump) {
///     // jump
/// }
/// assert_eq!(steer, 0);
/// ```
///
/// Every action can have several keys (here arrows and WASD). Binding an
/// action again adds keys; [`rebind`](Controls::rebind) replaces them. An
/// action that was never bound is never pressed, held or released.
#[derive(Clone, Debug)]
pub struct Controls<A> {
    bindings: Vec<(A, Vec<KeyCode>)>,
}

impl<A: Copy + PartialEq> Controls<A> {
    /// An empty set of bindings.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Fire }
    ///
    /// let controls: Controls<Action> = Controls::new().bind(Action::Fire, &[KeyCode::Char('f')]);
    /// assert_eq!(controls.keys_for(Action::Fire), &[KeyCode::Char('f')]);
    /// ```
    pub fn new() -> Controls<A> {
        Controls {
            bindings: Vec::new(),
        }
    }

    /// Binds `keys` to `action`, keeping any keys it already has. A key that
    /// is already bound to the action is not added twice.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Jump }
    ///
    /// let controls = Controls::new()
    ///     .bind(Action::Jump, &[KeyCode::Char(' ')])
    ///     .bind(Action::Jump, &[KeyCode::Up]);
    /// assert_eq!(controls.keys_for(Action::Jump), &[KeyCode::Char(' '), KeyCode::Up]);
    /// ```
    pub fn bind(mut self, action: A, keys: &[KeyCode]) -> Controls<A> {
        let index = self.slot(action);
        for &key in keys {
            push_unique(&mut self.bindings[index].1, key);
        }
        self
    }

    /// The keys bound to `action`, in the order they were bound. Empty if the
    /// action is not bound. Use it to show "press Enter" on a menu.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Start }
    ///
    /// let controls: Controls<Action> = Controls::new();
    /// assert!(controls.keys_for(Action::Start).is_empty());
    /// ```
    pub fn keys_for(&self, action: A) -> &[KeyCode] {
        let empty: &[KeyCode] = &[];
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == action)
            .map_or(empty, |(_, keys)| keys.as_slice())
    }

    /// Whether any key bound to `action` went down since the last
    /// [`Keys::end_tick`]. Use it for one-shot actions.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Jump }
    ///
    /// let controls = Controls::new().bind(Action::Jump, &[KeyCode::Char(' ')]);
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Char(' '))), Duration::ZERO);
    /// assert!(controls.pressed(&keys, Action::Jump));
    /// ```
    pub fn pressed(&self, keys: &Keys, action: A) -> bool {
        self.keys_for(action).iter().any(|&key| keys.pressed(key))
    }

    /// Whether any key bound to `action` is down now. Use it for movement.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Left }
    ///
    /// let controls = Controls::new().bind(Action::Left, &[KeyCode::Left, KeyCode::Char('a')]);
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Char('a'))), Duration::ZERO);
    /// assert!(controls.held(&keys, Action::Left));
    /// ```
    pub fn held(&self, keys: &Keys, action: A) -> bool {
        self.keys_for(action).iter().any(|&key| keys.held(key))
    }

    /// Whether any key bound to `action` went up (or timed out) since the last
    /// [`Keys::end_tick`].
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Charge }
    ///
    /// let controls = Controls::new().bind(Action::Charge, &[KeyCode::Char('c')]);
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Char('c'))), Duration::ZERO);
    /// keys.tick(Duration::from_millis(400)); // the emulated hold ran out
    /// assert!(controls.released(&keys, Action::Charge));
    /// ```
    pub fn released(&self, keys: &Keys, action: A) -> bool {
        self.keys_for(action).iter().any(|&key| keys.released(key))
    }

    /// `held(positive) - held(negative)`: `-1`, `0` or `1`, using the keys
    /// bound to each action.
    ///
    /// ```
    /// use std::time::Duration;
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Left, Right }
    ///
    /// let controls = Controls::new()
    ///     .bind(Action::Left, &[KeyCode::Left])
    ///     .bind(Action::Right, &[KeyCode::Right]);
    /// let mut keys = Keys::new();
    /// keys.event(&Event::Key(Key::new(KeyCode::Left)), Duration::ZERO);
    /// assert_eq!(controls.axis(&keys, Action::Left, Action::Right), -1);
    /// ```
    pub fn axis(&self, keys: &Keys, negative: A, positive: A) -> i32 {
        i32::from(self.held(keys, positive)) - i32::from(self.held(keys, negative))
    }

    /// Replaces the keys bound to `action`. An action that was not bound is
    /// bound now. Use it to let players change a key in a settings menu.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// #[derive(Clone, Copy, PartialEq)]
    /// enum Action { Jump }
    ///
    /// let mut controls = Controls::new().bind(Action::Jump, &[KeyCode::Char(' ')]);
    /// controls.rebind(Action::Jump, &[KeyCode::Char('j')]);
    /// assert_eq!(controls.keys_for(Action::Jump), &[KeyCode::Char('j')]);
    /// ```
    pub fn rebind(&mut self, action: A, keys: &[KeyCode]) {
        let index = self.slot(action);
        let bound = &mut self.bindings[index].1;
        bound.clear();
        for &key in keys {
            push_unique(bound, key);
        }
    }

    /// The index of `action`'s entry, adding an empty one if it is new.
    fn slot(&mut self, action: A) -> usize {
        match self.bindings.iter().position(|(bound, _)| *bound == action) {
            Some(index) => index,
            None => {
                self.bindings.push((action, Vec::new()));
                self.bindings.len() - 1
            }
        }
    }
}

impl<A: Copy + PartialEq> Default for Controls<A> {
    fn default() -> Controls<A> {
        Controls::new()
    }
}

fn push_unique(keys: &mut Vec<KeyCode>, key: KeyCode) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, Key};
    use std::time::Duration;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Action {
        Left,
        Right,
        Jump,
    }

    fn press(code: KeyCode) -> Event {
        Event::Key(Key::new(code))
    }

    fn wasd_and_arrows() -> Controls<Action> {
        Controls::new()
            .bind(Action::Left, &[KeyCode::Left])
            .bind(Action::Left, &[KeyCode::Char('a')])
            .bind(Action::Right, &[KeyCode::Right, KeyCode::Char('d')])
    }

    #[test]
    fn several_keys_can_trigger_one_action() {
        let controls = wasd_and_arrows();
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Char('a')), Duration::ZERO);
        assert!(controls.held(&keys, Action::Left));
        assert!(controls.pressed(&keys, Action::Left));
        keys.end_tick();
        keys.event(&press(KeyCode::Left), Duration::ZERO);
        assert!(controls.held(&keys, Action::Left));
        assert_eq!(
            controls.keys_for(Action::Left),
            &[KeyCode::Left, KeyCode::Char('a')]
        );
    }

    #[test]
    fn binding_the_same_action_again_adds_keys() {
        let controls = wasd_and_arrows();
        assert_eq!(
            controls.keys_for(Action::Left),
            &[KeyCode::Left, KeyCode::Char('a')]
        );
        let twice = Controls::new()
            .bind(Action::Jump, &[KeyCode::Char(' ')])
            .bind(Action::Jump, &[KeyCode::Char(' '), KeyCode::Up]);
        assert_eq!(
            twice.keys_for(Action::Jump),
            &[KeyCode::Char(' '), KeyCode::Up]
        );
    }

    #[test]
    fn an_unbound_action_is_never_pressed_held_or_released() {
        let controls: Controls<Action> = Controls::new();
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Enter), Duration::ZERO);
        assert!(!controls.pressed(&keys, Action::Jump));
        assert!(!controls.held(&keys, Action::Jump));
        assert!(!controls.released(&keys, Action::Jump));
        assert_eq!(controls.axis(&keys, Action::Left, Action::Right), 0);
        assert_eq!(controls.keys_for(Action::Jump), &[] as &[KeyCode]);
    }

    #[test]
    fn axis_follows_the_bound_keys() {
        let controls = wasd_and_arrows();
        let mut keys = Keys::new();
        assert_eq!(controls.axis(&keys, Action::Left, Action::Right), 0);
        keys.event(&press(KeyCode::Char('d')), Duration::ZERO);
        assert_eq!(controls.axis(&keys, Action::Left, Action::Right), 1);
        keys.event(&press(KeyCode::Left), Duration::ZERO);
        assert_eq!(controls.axis(&keys, Action::Left, Action::Right), 0);
    }

    #[test]
    fn rebind_replaces_the_keys_of_an_action() {
        let mut controls = wasd_and_arrows();
        controls.rebind(Action::Left, &[KeyCode::Char('j')]);
        assert_eq!(controls.keys_for(Action::Left), &[KeyCode::Char('j')]);
        let mut keys = Keys::new();
        keys.event(&press(KeyCode::Left), Duration::ZERO);
        assert!(!controls.held(&keys, Action::Left), "the old key is gone");
        keys.event(&press(KeyCode::Char('j')), Duration::ZERO);
        assert!(controls.held(&keys, Action::Left));
    }

    #[test]
    fn rebind_of_an_unknown_action_binds_it() {
        let mut controls: Controls<Action> = Controls::new();
        controls.rebind(Action::Jump, &[KeyCode::Enter]);
        assert_eq!(controls.keys_for(Action::Jump), &[KeyCode::Enter]);
    }

    #[test]
    fn released_follows_real_key_releases() {
        let controls = Controls::new().bind(Action::Jump, &[KeyCode::Char(' ')]);
        let mut keys = Keys::new().real_release(true);
        keys.event(&press(KeyCode::Char(' ')), Duration::ZERO);
        keys.event(
            &Event::KeyUp(Key::new(KeyCode::Char(' '))),
            Duration::from_millis(40),
        );
        assert!(controls.released(&keys, Action::Jump));
        assert!(!controls.held(&keys, Action::Jump));
    }
}

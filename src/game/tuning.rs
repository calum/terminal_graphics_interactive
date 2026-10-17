//! [`Tuning`]: numbers a game exposes so they can be changed while it runs
//! (sliders on the website).

use std::cell::RefCell;
use std::rc::Rc;

/// One number a game exposes.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
/// let knob = &tuning.knobs()[0];
/// assert_eq!(knob.name, "speed");
/// assert_eq!(knob.value, 2.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knob {
    /// The name the game looks it up by, e.g. `"fireball_rate"`.
    pub name: &'static str,
    /// The words a person sees beside the slider, e.g. `"Fireball rate"`.
    pub label: &'static str,
    /// The value the knob starts at (clamped into `min..=max`).
    pub default: f32,
    /// The smallest value the knob can be set to.
    pub min: f32,
    /// The largest value the knob can be set to.
    pub max: f32,
    /// The value now.
    pub value: f32,
}

/// A shared set of knobs. Cloning gives another handle to the same knobs, so
/// the code that built the set can move a knob while the game reads it.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let tuning = Tuning::new().knob("fireball_rate", "Fireball rate", 1.0, 0.0, 5.0);
/// let for_the_page = tuning.clone();
/// for_the_page.set("fireball_rate", 3.0);
/// assert_eq!(tuning.get("fireball_rate", 1.0), 3.0);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Tuning {
    knobs: Rc<RefCell<Vec<Knob>>>,
}

impl Tuning {
    /// Declares a knob (builder style, returns the set). Its value starts at
    /// `default`. If `min` is above `max` they are swapped, and `default` is
    /// clamped into the range. If `default`, `min` or `max` is NaN or infinite
    /// the call is ignored and no knob is added. Declaring a name again
    /// replaces the earlier knob in its place, so the count does not change.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new()
    ///     .knob("rain", "Rain", 0.5, 0.0, 1.0)
    ///     .knob("wind", "Wind", 9.0, 10.0, 0.0); // swapped to 0..=10, default 9
    /// assert_eq!(tuning.get("rain", 0.0), 0.5);
    /// assert_eq!(tuning.get("wind", 0.0), 9.0);
    /// tuning.set("wind", 99.0);
    /// assert_eq!(tuning.get("wind", 0.0), 10.0);
    /// ```
    pub fn knob(
        self,
        name: &'static str,
        label: &'static str,
        default: f32,
        min: f32,
        max: f32,
    ) -> Tuning {
        if ![default, min, max].iter().all(|n| n.is_finite()) {
            return self;
        }
        let (min, max) = if min > max { (max, min) } else { (min, max) };
        let default = clamp(default, min, max);
        let knob = Knob {
            name,
            label,
            default,
            min,
            max,
            value: default,
        };
        {
            let mut knobs = self.knobs.borrow_mut();
            match knobs.iter().position(|k| k.name == name) {
                Some(index) => knobs[index] = knob,
                None => knobs.push(knob),
            }
        }
        self
    }

    /// A copy of every knob in declaration order, for building a UI. It may
    /// allocate.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new().knob("a", "A", 1.0, 0.0, 2.0);
    /// assert_eq!(tuning.knobs()[0].name, "a");
    /// ```
    pub fn knobs(&self) -> Vec<Knob> {
        self.knobs.borrow().clone()
    }

    /// The current value of `name`, or `fallback` if no knob has that name.
    /// Call it every update: a game with no tuner gets `fallback` and behaves
    /// as it always did. It does not allocate.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new().knob("fireball_rate", "Fireball rate", 2.0, 0.0, 5.0);
    /// assert_eq!(tuning.get("fireball_rate", 1.0), 2.0);
    /// assert_eq!(tuning.get("no_such_knob", 1.0), 1.0);
    /// ```
    pub fn get(&self, name: &str, fallback: f32) -> f32 {
        self.knobs
            .borrow()
            .iter()
            .find(|k| k.name == name)
            .map_or(fallback, |k| k.value)
    }

    /// Sets `name` to `value`, clamped into the knob's range. An unknown name
    /// is ignored. Call it from the page's code or from a person's slider.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new().knob("volume", "Volume", 0.5, 0.0, 1.0);
    /// tuning.set("volume", 7.0);
    /// assert_eq!(tuning.get("volume", 0.0), 1.0);
    /// ```
    pub fn set(&self, name: &str, value: f32) {
        let position = self.knobs.borrow().iter().position(|k| k.name == name);
        if let Some(index) = position {
            self.set_at(index, value);
        }
    }

    /// Sets the knob at `index` (its place in declaration order) the same way
    /// as [`set`](Tuning::set). An index with no knob is ignored.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new()
    ///     .knob("a", "A", 0.0, 0.0, 1.0)
    ///     .knob("b", "B", 0.0, 0.0, 1.0);
    /// tuning.set_at(1, 0.25);
    /// assert_eq!(tuning.get("b", 9.0), 0.25);
    /// ```
    pub fn set_at(&self, index: usize, value: f32) {
        if value.is_nan() {
            return;
        }
        if let Some(knob) = self.knobs.borrow_mut().get_mut(index) {
            knob.value = clamp(value, knob.min, knob.max);
        }
    }

    /// A set with no knobs.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new();
    /// assert!(tuning.is_empty());
    /// ```
    pub fn new() -> Tuning {
        Tuning::default()
    }

    /// How many knobs there are.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// let tuning = Tuning::new().knob("a", "A", 0.0, 0.0, 1.0);
    /// assert_eq!(tuning.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.knobs.borrow().len()
    }

    /// Whether there are no knobs.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::*;
    ///
    /// assert!(Tuning::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `value` moved into `min..=max`. Assumes `min <= max`.
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tuning_has_no_knobs() {
        let tuning = Tuning::new();
        assert_eq!(tuning.len(), 0);
        assert!(tuning.is_empty());
    }

    #[test]
    fn a_knob_is_added_with_its_value_at_its_default() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.5, 0.0, 10.0);
        assert_eq!(tuning.len(), 1);
        let knob = tuning.knobs()[0];
        assert_eq!(knob.name, "speed");
        assert_eq!(knob.label, "Speed");
        assert_eq!(knob.default, 2.5);
        assert_eq!(knob.value, 2.5);
    }

    #[test]
    fn a_default_outside_the_range_is_clamped_into_it() {
        let tuning = Tuning::new().knob("x", "X", 20.0, 0.0, 10.0);
        let knob = tuning.knobs()[0];
        assert_eq!(knob.default, 10.0);
        assert_eq!(knob.value, 10.0);
    }

    #[test]
    fn a_min_above_max_is_swapped() {
        let tuning = Tuning::new().knob("x", "X", 5.0, 10.0, 0.0);
        let knob = tuning.knobs()[0];
        assert_eq!(knob.min, 0.0);
        assert_eq!(knob.max, 10.0);
        assert_eq!(knob.value, 5.0);
    }

    #[test]
    fn a_knob_with_a_nan_or_infinite_number_is_ignored() {
        let tuning = Tuning::new()
            .knob("nan_default", "N", f32::NAN, 0.0, 1.0)
            .knob("inf_max", "I", 0.5, 0.0, f32::INFINITY)
            .knob("neg_inf_min", "I", 0.5, f32::NEG_INFINITY, 1.0)
            .knob("nan_min", "N", 0.5, f32::NAN, 1.0);
        assert!(tuning.is_empty());
    }

    #[test]
    fn declaring_a_name_again_replaces_it_in_place() {
        let tuning = Tuning::new()
            .knob("a", "A", 1.0, 0.0, 10.0)
            .knob("b", "B", 2.0, 0.0, 10.0)
            .knob("a", "A again", 7.0, 0.0, 20.0);
        assert_eq!(tuning.len(), 2, "the count is unchanged");
        let knobs = tuning.knobs();
        assert_eq!(knobs[0].name, "a");
        assert_eq!(knobs[0].label, "A again");
        assert_eq!(knobs[0].default, 7.0);
        assert_eq!(knobs[0].value, 7.0);
        assert_eq!(knobs[0].max, 20.0);
        assert_eq!(knobs[1].name, "b");
    }

    #[test]
    fn get_returns_the_value_of_a_known_knob() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        assert_eq!(tuning.get("speed", 9.0), 2.0);
    }

    #[test]
    fn get_returns_the_fallback_for_an_unknown_name() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        assert_eq!(tuning.get("gravity", 9.8), 9.8);
    }

    #[test]
    fn set_clamps_the_value_into_the_range_at_both_ends() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        tuning.set("speed", 99.0);
        assert_eq!(tuning.get("speed", -1.0), 10.0);
        tuning.set("speed", -5.0);
        assert_eq!(tuning.get("speed", -1.0), 0.0);
    }

    #[test]
    fn set_ignores_nan() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        tuning.set("speed", f32::NAN);
        assert_eq!(tuning.get("speed", -1.0), 2.0);
    }

    #[test]
    fn set_ignores_an_unknown_name_and_adds_no_knob() {
        let tuning = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        tuning.set("gravity", 5.0);
        assert_eq!(tuning.len(), 1);
        assert_eq!(tuning.get("speed", -1.0), 2.0);
    }

    #[test]
    fn set_at_sets_the_knob_at_that_index_and_ignores_a_bad_index() {
        let tuning = Tuning::new()
            .knob("a", "A", 1.0, 0.0, 10.0)
            .knob("b", "B", 2.0, 0.0, 10.0);
        tuning.set_at(1, 7.0);
        assert_eq!(tuning.get("a", -1.0), 1.0);
        assert_eq!(tuning.get("b", -1.0), 7.0);
        tuning.set_at(5, 3.0);
        assert_eq!(tuning.len(), 2);
    }

    #[test]
    fn a_clone_shares_the_knobs_with_the_original() {
        let game_side = Tuning::new().knob("speed", "Speed", 2.0, 0.0, 10.0);
        let page_side = game_side.clone();
        page_side.set("speed", 6.0);
        assert_eq!(game_side.get("speed", -1.0), 6.0);
    }

    #[test]
    fn a_game_with_no_tuner_gets_its_fallbacks_and_nothing_breaks() {
        let tuning = Tuning::new();
        assert_eq!(tuning.get("fireball_rate", 1.5), 1.5);
        tuning.set("fireball_rate", 3.0);
        tuning.set_at(0, 3.0);
        assert!(tuning.knobs().is_empty());
        assert_eq!(tuning.get("fireball_rate", 1.5), 1.5);
    }
}

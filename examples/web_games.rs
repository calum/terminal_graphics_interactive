#[allow(dead_code)]
#[path = "glow.rs"]
mod glow;
#[allow(dead_code)]
#[path = "quickgame.rs"]
mod quickgame;
#[allow(dead_code)]
#[path = "sail.rs"]
mod sail;

use std::cell::RefCell;
use terminal_graphics_interactive::game::{Headless, Knob, Tuning};
use terminal_graphics_interactive::{Colour, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};

/// The size of the games on the page, in cells.
const COLUMNS: u32 = 80;
const ROWS: u32 = 30;

/// The games run at 60 updates a second, whatever the page's frame rate.
const STEP_MS: f32 = 1000.0 / 60.0;
/// The most updates one tick may run, so a long pause does not fast-forward.
const MAX_STEPS: u32 = 5;
/// The longest gap counted in one tick (a hidden tab counts as this much).
const MAX_GAP_MS: f32 = 250.0;

struct Machine {
    game: Option<Headless>,
    tuning: Tuning,
    frame: Vec<u32>,
    waiting_ms: f32,
}

/// Two `u32`s a cell: a space, then black on black (palette index 0 both ways).
fn blank_frame() -> Vec<u32> {
    let mut frame = Vec::with_capacity((COLUMNS * ROWS * 2) as usize);
    for _ in 0..COLUMNS * ROWS {
        frame.push(' ' as u32);
        frame.push(0);
    }
    frame
}

fn palette_index(colour: Colour) -> u32 {
    Colour::ALL
        .iter()
        .position(|candidate| *candidate == colour)
        .unwrap_or(0) as u32
}

impl Machine {
    /// Runs the game for `ms` milliseconds of waiting at a fixed 60 updates a
    /// second. NaN, zero and a finished or missing game do nothing.
    /// `down` 1 presses the key, 0 releases it; unknown keys and other values
    /// do nothing. The frame is drawn on the next tick, not here.
    fn key_event(&mut self, down: u32, code: u32) {
        let (Some(game), Some(key)) = (self.game.as_mut(), key_code(code)) else {
            return;
        };
        match down {
            1 => game.press(key),
            0 => game.release(key),
            _ => {}
        }
    }

    /// Moves or clicks the pointer. Off the screen is pulled back onto it. The
    /// frame is drawn on the next tick, not here.
    fn mouse_event(&mut self, kind: u32, x: i32, y: i32) {
        let kind = match kind {
            0 => MouseKind::Moved,
            1 => MouseKind::Down(MouseButton::Left),
            2 => MouseKind::Up(MouseButton::Left),
            3 => MouseKind::Down(MouseButton::Right),
            4 => MouseKind::Up(MouseButton::Right),
            5 => MouseKind::Drag(MouseButton::Left),
            6 => MouseKind::Drag(MouseButton::Right),
            _ => return,
        };
        let mouse = Mouse {
            kind,
            x: x.clamp(0, COLUMNS as i32 - 1) as isize,
            y: y.clamp(0, ROWS as i32 - 1) as isize,
            modifiers: Modifiers::NONE,
        };
        if let Some(game) = self.game.as_mut() {
            game.mouse(&mouse);
        }
    }

    fn tuning_count(&self) -> u32 {
        self.tuning.len() as u32
    }

    /// The knob at `index` in declaration order, if there is one.
    fn knob(&self, index: u32) -> Option<Knob> {
        self.tuning.knobs().get(index as usize).copied()
    }

    fn tuning_min(&self, index: u32) -> f32 {
        self.knob(index).map_or(0.0, |knob| knob.min)
    }

    fn tuning_max(&self, index: u32) -> f32 {
        self.knob(index).map_or(0.0, |knob| knob.max)
    }

    fn tuning_value(&self, index: u32) -> f32 {
        self.knob(index).map_or(0.0, |knob| knob.value)
    }

    /// Sets the knob; it is clamped into its range. A bad index or NaN does nothing.
    fn tuning_set(&self, index: u32, value: f32) {
        self.tuning.set_at(index as usize, value);
    }

    fn tuning_label_len(&self, index: u32) -> u32 {
        self.knob(index).map_or(0, |knob| knob.label.len() as u32)
    }

    /// Byte `byte` of the knob's label, or 0 past its end or for no knob.
    fn tuning_label_byte(&self, index: u32, byte: u32) -> u32 {
        self.knob(index)
            .and_then(|knob| knob.label.as_bytes().get(byte as usize).copied())
            .map_or(0, u32::from)
    }

    fn tuning_name_len(&self, index: u32) -> u32 {
        self.knob(index).map_or(0, |knob| knob.name.len() as u32)
    }

    /// Byte `byte` of the knob's name, or 0 past its end or for no knob.
    fn tuning_name_byte(&self, index: u32, byte: u32) -> u32 {
        self.knob(index)
            .and_then(|knob| knob.name.as_bytes().get(byte as usize).copied())
            .map_or(0, u32::from)
    }

    /// True once the game has quit (for example q on quickgame's title).
    fn finished(&self) -> bool {
        self.game.as_ref().is_some_and(|game| game.quit())
    }

    fn tick(&mut self, ms: f32) {
        if ms.is_nan() || ms <= 0.0 {
            return;
        }
        let Some(game) = self.game.as_mut() else {
            return;
        };
        if game.quit() {
            return;
        }
        self.waiting_ms += ms.min(MAX_GAP_MS);
        let steps = ((self.waiting_ms / STEP_MS) as u32).min(MAX_STEPS);
        if steps == 0 {
            return;
        }
        // after a long pause, do not try to catch up on every missed update
        self.waiting_ms = (self.waiting_ms - steps as f32 * STEP_MS).clamp(0.0, STEP_MS);
        game.step(steps);
        self.encode();
    }

    fn ticks(&self) -> u64 {
        self.game.as_ref().map_or(0, Headless::tick)
    }

    /// Replaces the game with a fresh one: 0 glow, 1 sail, 2 quickgame. Any
    /// other index is ignored. Each game gets its own knobs at their defaults.
    fn load(&mut self, game: u32, seed: u32) {
        let game = match game {
            0 => {
                self.tuning = glow::tuning();
                Headless::new(glow::start(self.tuning.clone()))
            }
            1 => {
                self.tuning = sail::tuning();
                Headless::new(sail::start(self.tuning.clone()))
            }
            2 => {
                self.tuning = Tuning::new();
                Headless::new(quickgame::Title { score: None })
            }
            _ => return,
        };
        self.game = Some(game.size(COLUMNS, ROWS).seed(u64::from(seed)));
        self.waiting_ms = 0.0;
        self.encode();
    }

    /// Copies what the game drew into the frame the page reads.
    fn encode(&mut self) {
        let Some(game) = self.game.as_mut() else {
            self.frame = blank_frame();
            return;
        };
        let display = game.screen().display();
        for y in 0..ROWS {
            for x in 0..COLUMNS {
                let index = ((y * COLUMNS + x) * 2) as usize;
                let (character, colours) = match display.pixel(x as isize, y as isize) {
                    Some(pixel) => (
                        pixel.character() as u32,
                        palette_index(pixel.colour()) | palette_index(pixel.background()) << 8,
                    ),
                    None => (' ' as u32, 0),
                };
                self.frame[index] = character;
                self.frame[index + 1] = colours;
            }
        }
    }

    fn new() -> Machine {
        Machine {
            game: None,
            tuning: Tuning::new(),
            frame: blank_frame(),
            waiting_ms: 0.0,
        }
    }
}

/// The first code past every Unicode scalar value: named keys start here.
const NAMED_KEYS: u32 = 0x110000;

/// The named keys, in code order from `NAMED_KEYS`.
const NAMED: [KeyCode; 8] = [
    KeyCode::Enter,
    KeyCode::Esc,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Tab,
    KeyCode::Backspace,
];

fn key_code(code: u32) -> Option<KeyCode> {
    if code < NAMED_KEYS {
        char::from_u32(code).map(KeyCode::Char)
    } else {
        NAMED.get((code - NAMED_KEYS) as usize).copied()
    }
}

thread_local! {
    static MACHINE: RefCell<Machine> = RefCell::new(Machine::new());
}

/// Runs `action` on the one machine the page talks to.
fn with_machine<T>(action: impl FnOnce(&mut Machine) -> T) -> T {
    MACHINE.with(|machine| action(&mut machine.borrow_mut()))
}

#[no_mangle]
pub extern "C" fn load(game: u32, seed: u32) {
    with_machine(|m| m.load(game, seed));
}

#[no_mangle]
pub extern "C" fn tick(ms: u32) {
    with_machine(|m| m.tick(ms as f32));
}

#[no_mangle]
pub extern "C" fn ticks() -> u32 {
    with_machine(|m| m.ticks() as u32)
}

#[no_mangle]
pub extern "C" fn key_event(down: u32, code: u32) {
    with_machine(|m| m.key_event(down, code));
}

#[no_mangle]
pub extern "C" fn mouse_event(kind: u32, x: i32, y: i32) {
    with_machine(|m| m.mouse_event(kind, x, y));
}

#[no_mangle]
pub extern "C" fn finished() -> u32 {
    with_machine(|m| u32::from(m.finished()))
}

#[no_mangle]
pub extern "C" fn width() -> u32 {
    COLUMNS
}

#[no_mangle]
pub extern "C" fn height() -> u32 {
    ROWS
}

#[no_mangle]
pub extern "C" fn frame_ptr() -> usize {
    with_machine(|m| m.frame.as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn palette(index: u32) -> u32 {
    let colour = Colour::ALL[index as usize % Colour::ALL.len()];
    let (r, g, b) = colour.rgb();
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

#[no_mangle]
pub extern "C" fn tuning_count() -> u32 {
    with_machine(|m| m.tuning_count())
}

#[no_mangle]
pub extern "C" fn tuning_name_len(index: u32) -> u32 {
    with_machine(|m| m.tuning_name_len(index))
}

#[no_mangle]
pub extern "C" fn tuning_name_byte(index: u32, byte: u32) -> u32 {
    with_machine(|m| m.tuning_name_byte(index, byte))
}

#[no_mangle]
pub extern "C" fn tuning_label_len(index: u32) -> u32 {
    with_machine(|m| m.tuning_label_len(index))
}

#[no_mangle]
pub extern "C" fn tuning_label_byte(index: u32, byte: u32) -> u32 {
    with_machine(|m| m.tuning_label_byte(index, byte))
}

#[no_mangle]
pub extern "C" fn tuning_min(index: u32) -> f32 {
    with_machine(|m| m.tuning_min(index))
}

#[no_mangle]
pub extern "C" fn tuning_max(index: u32) -> f32 {
    with_machine(|m| m.tuning_max(index))
}

#[no_mangle]
pub extern "C" fn tuning_value(index: u32) -> f32 {
    with_machine(|m| m.tuning_value(index))
}

#[no_mangle]
pub extern "C" fn tuning_set(index: u32, value: f32) {
    with_machine(|m| m.tuning_set(index, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTER: u32 = NAMED_KEYS;
    const KEY_RIGHT: u32 = NAMED_KEYS + 3;

    #[test]
    fn a_character_code_is_that_character() {
        assert_eq!(key_code('a' as u32), Some(KeyCode::Char('a')));
    }

    #[test]
    fn an_invalid_character_code_is_ignored() {
        assert_eq!(key_code(0xD800), None, "a surrogate is not a character");
    }

    #[test]
    fn the_named_keys_follow_the_character_range() {
        assert_eq!(key_code(NAMED_KEYS), Some(KeyCode::Enter));
    }

    #[test]
    fn a_new_machine_with_no_game_shows_a_blank_frame_of_the_right_size() {
        let machine = Machine::new();
        assert_eq!(machine.frame.len(), (COLUMNS * ROWS * 2) as usize);
        assert!(machine
            .frame
            .chunks(2)
            .all(|cell| cell[0] == ' ' as u32 && cell[1] == 0));
    }

    fn distinct_cells(machine: &Machine) -> usize {
        let mut seen = std::collections::HashSet::new();
        for cell in machine.frame.chunks(2) {
            seen.insert((cell[0], cell[1]));
        }
        seen.len()
    }

    #[test]
    fn loading_a_game_draws_its_title_screen_at_once() {
        for game in 0..3 {
            let mut machine = Machine::new();
            machine.load(game, 1);
            assert!(distinct_cells(&machine) > 3, "game {game}");
        }
    }

    /// Runs `ms` milliseconds in 16 ms ticks.
    fn run(machine: &mut Machine, ms: u32) {
        for _ in 0..ms / 16 {
            machine.tick(16.0);
        }
    }

    #[test]
    fn ticking_a_second_in_small_pieces_runs_about_sixty_updates() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        run(&mut machine, 1000);
        let ticks = machine.ticks();
        assert!((59..=61).contains(&ticks), "{ticks}");
    }

    #[test]
    fn glows_sky_and_sails_sea_animate_on_the_title_screen() {
        for game in [0, 1] {
            let mut machine = Machine::new();
            machine.load(game, 1);
            let first = machine.frame.clone();
            run(&mut machine, 500);
            assert_ne!(machine.frame, first, "game {game} should move");
        }
    }

    #[test]
    fn glow_has_two_knobs_sail_three_and_quickgame_none() {
        let mut machine = Machine::new();
        for (game, count) in [(0, 2), (1, 3), (2, 0)] {
            machine.load(game, 1);
            assert_eq!(machine.tuning_count(), count, "game {game}");
        }
    }

    /// Reads a knob's name back one byte at a time, as the page does.
    fn name_of(machine: &Machine, index: u32) -> String {
        let len = machine.tuning_name_len(index);
        let bytes: Vec<u8> = (0..len)
            .map(|j| machine.tuning_name_byte(index, j) as u8)
            .collect();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn glows_knob_names_read_back_byte_by_byte() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        assert_eq!(name_of(&machine, 0), "mote_rate");
        assert_eq!(name_of(&machine, 1), "round_length");
        assert_eq!(machine.tuning_name_byte(0, 99), 0, "past the end is 0");
        assert_eq!(machine.tuning_name_len(7), 0, "no such knob");
    }

    /// Reads a knob's label back one byte at a time, as the page does.
    fn label_of(machine: &Machine, index: u32) -> String {
        let len = machine.tuning_label_len(index);
        let bytes: Vec<u8> = (0..len)
            .map(|j| machine.tuning_label_byte(index, j) as u8)
            .collect();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn sails_knob_labels_read_back_byte_by_byte() {
        let mut machine = Machine::new();
        machine.load(1, 1);
        assert_eq!(label_of(&machine, 0), "fireball rate (x)");
        assert_eq!(label_of(&machine, 2), "boat speed (x)");
        assert_eq!(machine.tuning_label_len(3), 0);
    }

    #[test]
    fn tuning_set_clamps_into_the_range_and_the_range_reads_back() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        assert_eq!(machine.tuning_min(0), 0.25);
        assert_eq!(machine.tuning_max(0), 4.0);
        assert_eq!(machine.tuning_value(0), 1.0, "the default");
        machine.tuning_set(0, 99.0);
        assert_eq!(machine.tuning_value(0), 4.0);
        machine.tuning_set(0, -5.0);
        assert_eq!(machine.tuning_value(0), 0.25);
        machine.tuning_set(0, 2.5);
        assert_eq!(machine.tuning_value(0), 2.5);
    }

    #[test]
    fn tuning_set_ignores_bad_indexes_and_nan_and_reads_zero_for_them() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        machine.tuning_set(0, 2.0);
        machine.tuning_set(0, f32::NAN);
        machine.tuning_set(99, 2.0);
        machine.tuning_set(u32::MAX, f32::NAN);
        assert_eq!(machine.tuning_value(0), 2.0);
        assert_eq!(machine.tuning_value(99), 0.0);
        assert_eq!(machine.tuning_min(99), 0.0);
        assert_eq!(machine.tuning_max(99), 0.0);
    }

    /// Game `game` with knob `knob` set to `value` before the round starts.
    fn round_with_knob(game: u32, knob: u32, value: f32) -> Machine {
        let mut machine = Machine::new();
        machine.load(game, 1);
        machine.tuning_set(knob, value);
        machine.key_event(1, ENTER);
        run(&mut machine, 16);
        machine.key_event(0, ENTER);
        run(&mut machine, 16);
        machine
    }

    #[test]
    fn the_knobs_change_what_the_games_draw() {
        let sail = |rate| {
            let mut machine = round_with_knob(1, 0, rate); // fireball_rate
            run(&mut machine, 5000);
            machine.frame
        };
        assert_ne!(sail(3.0), sail(0.2));

        let glow = |rate| {
            let mut machine = round_with_knob(0, 0, rate); // mote_rate
            run(&mut machine, 5000);
            machine.frame
        };
        assert_ne!(glow(4.0), glow(0.25));
    }

    #[test]
    fn loading_a_game_resets_its_knobs_to_their_defaults() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        machine.tuning_set(0, 3.5);
        machine.load(0, 1);
        assert_eq!(machine.tuning_value(0), 1.0);
    }

    #[test]
    fn an_unknown_game_index_changes_nothing() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        run(&mut machine, 100);
        let before = machine.frame.clone();
        machine.load(9, 1);
        assert_eq!(machine.frame, before);
        run(&mut machine, 500);
        assert_ne!(machine.frame, before, "glow is still running");
    }

    #[test]
    fn every_game_uses_palette_indexes_below_sixteen() {
        for game in 0..3 {
            let mut machine = started(game);
            run(&mut machine, 1000);
            for cell in machine.frame.chunks(2) {
                let (foreground, background) = (cell[1] & 0xff, (cell[1] >> 8) & 0xff);
                assert!(foreground < 16 && background < 16, "game {game}: {cell:?}");
            }
        }
    }

    #[test]
    fn q_on_the_quickgame_title_finishes_it_and_later_ticks_change_nothing() {
        let mut machine = Machine::new();
        machine.load(2, 1);
        machine.key_event(1, u32::from('q'));
        run(&mut machine, 16);
        machine.key_event(0, u32::from('q'));
        run(&mut machine, 16);
        assert!(machine.finished());
        let before = (machine.frame.clone(), machine.ticks());
        run(&mut machine, 500);
        assert_eq!((machine.frame.clone(), machine.ticks()), before);
    }

    #[test]
    fn enter_starts_each_game() {
        for game in 0..3 {
            let mut machine = Machine::new();
            machine.load(game, 1);
            let title = machine.frame.clone();
            machine.key_event(1, ENTER);
            run(&mut machine, 200);
            assert_ne!(machine.frame, title, "game {game}");
            machine.key_event(0, ENTER);
        }
    }

    /// Game `game` (see `load`) with its round started and no key held.
    fn started(game: u32) -> Machine {
        started_with_seed(game, 1)
    }

    fn started_with_seed(game: u32, seed: u32) -> Machine {
        let mut machine = Machine::new();
        machine.load(game, seed);
        machine.key_event(1, ENTER);
        run(&mut machine, 16);
        machine.key_event(0, ENTER);
        run(&mut machine, 16);
        machine
    }

    fn playing_quickgame() -> Machine {
        started(2)
    }

    #[test]
    fn the_same_seed_gives_the_same_frames_and_another_seed_a_different_one() {
        let make = |game, seed| {
            let mut machine = started_with_seed(game, seed);
            run(&mut machine, 2000);
            machine.frame
        };
        assert_eq!(make(1, 3), make(1, 3));
        assert_ne!(make(1, 3), make(1, 4), "sail");
        assert_eq!(make(0, 3), make(0, 3));
        assert_ne!(make(0, 3), make(0, 4), "glow");
    }

    #[test]
    fn holding_the_mouse_button_moves_the_glow_light() {
        let mut held = started(0);
        held.mouse_event(1, 79, 15);
        run(&mut held, 1000);
        let mut idle = started(0);
        run(&mut idle, 1000);
        assert_ne!(held.frame, idle.frame);
    }

    /// The column and row of the player's `@` in the frame.
    fn player_cell(machine: &Machine) -> Option<(u32, u32)> {
        let index = machine
            .frame
            .chunks(2)
            .position(|cell| cell[0] == '@' as u32)?;
        Some((index as u32 % COLUMNS, index as u32 / COLUMNS))
    }

    #[test]
    fn holding_right_draws_the_round_differently_from_holding_nothing() {
        let mut held = playing_quickgame();
        held.key_event(1, KEY_RIGHT);
        run(&mut held, 1000);
        let mut idle = playing_quickgame();
        run(&mut idle, 1000);
        assert_ne!(held.frame, idle.frame);
    }

    #[test]
    fn holding_right_moves_the_player_and_releasing_stops_it() {
        let mut machine = playing_quickgame();
        let start = player_cell(&machine).expect("the player is drawn");
        machine.key_event(1, KEY_RIGHT);
        run(&mut machine, 300);
        machine.key_event(0, KEY_RIGHT);
        run(&mut machine, 16);
        let stopped = player_cell(&machine).expect("the player is drawn");
        assert!(stopped.0 > start.0, "{start:?} to {stopped:?}");
        run(&mut machine, 500);
        assert_eq!(player_cell(&machine), Some(stopped));
    }

    #[test]
    fn quickgame_animates_once_it_is_playing() {
        let mut machine = Machine::new();
        machine.load(2, 1);
        machine.key_event(1, ENTER);
        run(&mut machine, 16);
        machine.key_event(0, ENTER);
        run(&mut machine, 16);
        let first = machine.frame.clone();
        // the first star falls 36 updates in, about 0.6 seconds
        run(&mut machine, 1200);
        assert_ne!(machine.frame, first);
    }

    #[test]
    fn a_huge_gap_runs_at_most_five_updates() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        machine.tick(1_000_000.0);
        assert_eq!(machine.ticks(), 5);
    }

    #[test]
    fn a_zero_or_nan_tick_changes_nothing() {
        let mut machine = Machine::new();
        machine.load(0, 1);
        let before = (machine.frame.clone(), machine.ticks());
        machine.tick(0.0);
        machine.tick(f32::NAN);
        assert_eq!((machine.frame.clone(), machine.ticks()), before);
    }

    #[test]
    fn every_named_key_has_its_code() {
        let expected = [
            (0, KeyCode::Enter),
            (1, KeyCode::Esc),
            (2, KeyCode::Left),
            (3, KeyCode::Right),
            (4, KeyCode::Up),
            (5, KeyCode::Down),
            (6, KeyCode::Tab),
            (7, KeyCode::Backspace),
        ];
        for (offset, key) in expected {
            assert_eq!(key_code(NAMED_KEYS + offset), Some(key), "offset {offset}");
        }
    }

    #[test]
    fn an_unknown_named_code_is_ignored() {
        assert_eq!(key_code(NAMED_KEYS + 8), None);
        assert_eq!(key_code(u32::MAX), None);
    }
}

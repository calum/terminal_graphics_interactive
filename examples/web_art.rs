//! The program that runs on the project website, compiled to WebAssembly
//! (`cargo build --release --example web_art --target wasm32-unknown-unknown`).
//!
//! The page sends it the code from the editor one byte at a time, asks it to
//! compile, then calls `tick` once per animation frame and paints the frame it
//! gets back onto a canvas. Everything that makes art (the formula, the
//! palette, the physics) is the real library; this file only connects it to
//! the page. The page forwards the mouse and shows cells.
//!
//! The exports:
//!
//! - `init(seed)`: start up with a random seed
//! - `source_clear()`, `source_push(byte)`: the program text, a byte at a time
//! - `compile()`: 0 if it did not compile, 1 for a sketch, 2 for a world. A
//!   program that does not compile leaves the previous one running.
//!   `error_line()`, `error_column()`, `error_len()`, `error_byte(i)` describe
//!   the error.
//! - `failure_len()`, `failure_byte(i)`: why a running program stopped, if it did
//! - `tick(ms)`: advance by `ms` milliseconds and redraw
//! - `mouse_event(kind, x, y)`: 0 moved, 1 left down, 2 left up, 3 right down,
//!   4 right up, 5 left drag, 6 right drag; `x` and `y` are cells
//! - `width()`, `height()`, `frame_ptr()`: the frame, two `u32`s per cell: the
//!   character, then `foreground | background << 8` as palette indexes
//! - `palette(index)`: the `0xRRGGBB` value of a palette colour

#[allow(dead_code)]
#[path = "site/presets.rs"]
mod presets;
#[allow(dead_code)]
#[path = "site/script/mod.rs"]
mod script;

use std::cell::RefCell;
use terminal_graphics_interactive::{
    sketch_frame, Colour, Display, Modifiers, Mouse, MouseButton, MouseKind, Pointer, Simulation,
};

/// The size of the terminal on the page, in cells.
const COLUMNS: u32 = 64;
const ROWS: u32 = 24;

/// The animation runs at about 30 frames a second.
const STEP_MS: f32 = 1000.0 / 30.0;

/// The longest program accepted, in bytes.
const MAX_SOURCE: usize = 64 * 1024;

struct Machine {
    source: Vec<u8>,
    program: Option<script::Program>,
    simulation: Option<Simulation>,
    pointer: Pointer,
    display: Display,
    frame: Vec<u32>,
    seed: u32,
    seconds: f32,
    waiting_ms: f32,
    error: Option<script::CompileError>,
    failure: Option<String>,
}

impl Machine {
    fn new(seed: u32) -> Machine {
        let mut machine = Machine {
            source: Vec::new(),
            program: None,
            simulation: None,
            pointer: Pointer::default(),
            display: Display::new(COLUMNS, ROWS),
            frame: vec![0; (COLUMNS * ROWS * 2) as usize],
            seed,
            seconds: 0.0,
            waiting_ms: 0.0,
            error: None,
            failure: None,
        };
        machine.clear_display();
        machine.encode();
        machine
    }

    fn clear_display(&mut self) {
        for y in 0..ROWS as isize {
            for x in 0..COLUMNS as isize {
                self.display
                    .set_pixel(x, y, ' ', Colour::White, Colour::Black);
            }
        }
    }

    fn source_clear(&mut self) {
        self.source.clear();
    }

    fn source_push(&mut self, byte: u8) {
        if self.source.len() < MAX_SOURCE {
            self.source.push(byte);
        }
    }

    /// 0 for an error, 1 for a sketch, 2 for a world. A program that does not
    /// compile leaves the one that is running alone.
    fn compile(&mut self) -> u32 {
        let text = String::from_utf8_lossy(&self.source).into_owned();
        let program = match script::compile(&text) {
            Ok(program) => program,
            Err(error) => {
                self.error = Some(error);
                return 0;
            }
        };

        // a fresh start: time from zero, a blank screen, no old clicks
        self.error = None;
        self.failure = None;
        self.seconds = 0.0;
        self.waiting_ms = 0.0;
        self.pointer.clicks.clear();
        self.clear_display();
        self.encode();

        let kind = match program.scene() {
            script::Scene::World(world) => {
                let mut simulation = world.simulation(self.seed);
                simulation.resize(COLUMNS, ROWS);
                self.simulation = Some(simulation);
                2
            }
            script::Scene::Sketch(_) => {
                self.simulation = None;
                1
            }
        };
        self.program = Some(program);
        kind
    }

    /// `x` and `y` are cells; anything off the screen is pulled back onto it.
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
        self.pointer.update(&mouse);
    }

    fn tick(&mut self, ms: f32) {
        // NaN and zero do nothing; a long pause (a hidden tab) counts as a short one
        if ms.is_nan() || ms <= 0.0 || self.failure.is_some() {
            return;
        }
        let Some(program) = &self.program else {
            return;
        };
        self.waiting_ms += ms.min(250.0);

        match program.scene() {
            script::Scene::Sketch(palette) => {
                if self.waiting_ms < STEP_MS {
                    return;
                }
                self.seconds += self.waiting_ms / 1000.0;
                self.waiting_ms = 0.0;

                let formula = program.formula();
                let before = self.display.clone();
                sketch_frame(
                    &mut self.display,
                    palette,
                    self.seconds,
                    &self.pointer,
                    |cell| formula.call(cell),
                );
                if let Some(why) = formula.failure() {
                    // keep the last good picture on screen
                    self.display = before;
                    self.failure = Some(why);
                    return;
                }
                self.pointer.end_frame();
            }
            script::Scene::World(_) => {
                let steps = ((self.waiting_ms / STEP_MS) as u32).min(5);
                if steps == 0 {
                    return;
                }
                // after a pause do not try to catch up on every missed step
                self.waiting_ms = (self.waiting_ms - steps as f32 * STEP_MS).clamp(0.0, STEP_MS);
                let Some(simulation) = self.simulation.as_mut() else {
                    return;
                };
                for _ in 0..steps {
                    simulation.step(&self.pointer);
                    // a click is for the first step only
                    self.pointer.end_frame();
                }
                simulation.render(&mut self.display);
            }
        }
        self.encode();
    }

    /// Copies the display into the frame the page reads.
    fn encode(&mut self) {
        for y in 0..ROWS {
            for x in 0..COLUMNS {
                let index = ((y * COLUMNS + x) * 2) as usize;
                let pixel = self.display.pixel(x as isize, y as isize);
                let (character, colours) = match pixel {
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
}

fn palette_index(colour: Colour) -> u32 {
    Colour::ALL
        .iter()
        .position(|candidate| *candidate == colour)
        .unwrap_or(0) as u32
}

thread_local! {
    static MACHINE: RefCell<Machine> = RefCell::new(Machine::new(1));
}

#[no_mangle]
pub extern "C" fn init(seed: u32) {
    MACHINE.with(|m| *m.borrow_mut() = Machine::new(seed));
}

#[no_mangle]
pub extern "C" fn source_clear() {
    MACHINE.with(|m| m.borrow_mut().source_clear());
}

#[no_mangle]
pub extern "C" fn source_push(byte: u32) {
    MACHINE.with(|m| m.borrow_mut().source_push(byte as u8));
}

#[no_mangle]
pub extern "C" fn compile() -> u32 {
    MACHINE.with(|m| m.borrow_mut().compile())
}

#[no_mangle]
pub extern "C" fn error_line() -> u32 {
    MACHINE.with(|m| m.borrow().error.as_ref().map_or(0, |e| e.line))
}

#[no_mangle]
pub extern "C" fn error_column() -> u32 {
    MACHINE.with(|m| m.borrow().error.as_ref().map_or(0, |e| e.column))
}

#[no_mangle]
pub extern "C" fn error_len() -> u32 {
    MACHINE.with(|m| {
        m.borrow()
            .error
            .as_ref()
            .map_or(0, |e| e.message.len() as u32)
    })
}

#[no_mangle]
pub extern "C" fn error_byte(index: u32) -> u32 {
    MACHINE.with(|m| {
        m.borrow()
            .error
            .as_ref()
            .and_then(|e| e.message.as_bytes().get(index as usize))
            .map_or(0, |b| *b as u32)
    })
}

#[no_mangle]
pub extern "C" fn failure_len() -> u32 {
    MACHINE.with(|m| m.borrow().failure.as_ref().map_or(0, |f| f.len() as u32))
}

#[no_mangle]
pub extern "C" fn failure_byte(index: u32) -> u32 {
    MACHINE.with(|m| {
        m.borrow()
            .failure
            .as_ref()
            .and_then(|f| f.as_bytes().get(index as usize))
            .map_or(0, |b| *b as u32)
    })
}

#[no_mangle]
pub extern "C" fn tick(ms: u32) {
    MACHINE.with(|m| m.borrow_mut().tick(ms as f32));
}

#[no_mangle]
pub extern "C" fn mouse_event(kind: u32, x: i32, y: i32) {
    MACHINE.with(|m| m.borrow_mut().mouse_event(kind, x, y));
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
    MACHINE.with(|m| m.borrow().frame.as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn palette(index: u32) -> u32 {
    let colour = Colour::ALL[index as usize % Colour::ALL.len()];
    let (r, g, b) = colour.rgb();
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLASMA: &str = include_str!("plasma.rs");
    const BALLS: &str = include_str!("balls.rs");

    fn load(machine: &mut Machine, source: &str) -> u32 {
        machine.source_clear();
        for byte in source.bytes() {
            machine.source_push(byte);
        }
        machine.compile()
    }

    fn running(source: &str) -> Machine {
        let mut machine = Machine::new(7);
        assert_ne!(load(&mut machine, source), 0, "{:?}", machine.error);
        machine
    }

    /// Runs `ms` milliseconds in 40 ms ticks, like a browser at 25 frames a second.
    fn run_for(machine: &mut Machine, ms: u32) {
        for _ in 0..ms / 40 {
            machine.tick(40.0);
        }
    }

    fn distinct_cells(machine: &Machine) -> usize {
        let mut seen = std::collections::HashSet::new();
        for cell in machine.frame.chunks(2) {
            seen.insert((cell[0], cell[1]));
        }
        seen.len()
    }

    // ------------------------------------------------------------ the frame

    #[test]
    fn a_new_machine_shows_a_blank_black_screen_of_the_right_size() {
        let machine = Machine::new(1);
        assert_eq!(machine.frame.len(), (COLUMNS * ROWS * 2) as usize);
        assert_eq!(distinct_cells(&machine), 1);
        let black = palette_index(Colour::Black) << 8 | palette_index(Colour::White);
        assert!(machine
            .frame
            .chunks(2)
            .all(|c| c[0] == ' ' as u32 && c[1] == black));
    }

    #[test]
    fn colours_are_palette_indexes_below_sixteen_and_the_palette_has_real_values() {
        let mut machine = running(PLASMA);
        run_for(&mut machine, 200);
        for cell in machine.frame.chunks(2) {
            assert!((cell[1] & 255) < 16 && ((cell[1] >> 8) & 255) < 16);
        }
        assert_eq!(palette(0), 0x000000);
        assert_eq!(palette(15), 0xFFFFFF);
        assert_eq!(palette(16), palette(0), "indexes wrap rather than panic");
    }

    // -------------------------------------------------------------- compiling

    #[test]
    fn a_sketch_compiles_to_one_and_a_world_to_two() {
        let mut machine = Machine::new(1);
        assert_eq!(load(&mut machine, PLASMA), 1);
        assert_eq!(load(&mut machine, BALLS), 2);
        assert!(machine.error.is_none());
    }

    #[test]
    fn an_error_is_reported_with_its_position_and_message() {
        let mut machine = Machine::new(1);
        let source = "fn main() -> std::io::Result<()> {\n    sketch(|cell| cell.x * 2)\n}";
        assert_eq!(load(&mut machine, source), 0);
        let error = machine.error.as_ref().unwrap();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("mismatched types"));
        assert_eq!(
            error_line(),
            0,
            "the exports read the global machine, not this one"
        );
    }

    #[test]
    fn a_program_that_does_not_compile_leaves_the_previous_one_running() {
        let mut machine = running(PLASMA);
        run_for(&mut machine, 200);
        let before = machine.frame.clone();
        assert_eq!(load(&mut machine, "fn main("), 0);
        run_for(&mut machine, 200);
        assert_ne!(machine.frame, before, "it kept animating");
        assert!(machine.error.is_some());
    }

    #[test]
    fn a_good_program_clears_the_old_error() {
        let mut machine = Machine::new(1);
        assert_eq!(load(&mut machine, "nonsense"), 0);
        assert!(machine.error.is_some());
        assert_ne!(load(&mut machine, PLASMA), 0);
        assert!(machine.error.is_none());
    }

    #[test]
    fn the_source_is_built_a_byte_at_a_time_and_can_be_cleared() {
        let mut machine = Machine::new(1);
        machine.source_push(b'a');
        machine.source_push(b'b');
        assert_eq!(machine.source, b"ab");
        machine.source_clear();
        assert!(machine.source.is_empty());
    }

    #[test]
    fn unicode_and_broken_bytes_in_the_source_are_an_error_not_a_crash() {
        let mut machine = Machine::new(1);
        assert_eq!(load(&mut machine, "// héllo ✨\nfn main() {}"), 0);
        machine.source_clear();
        for byte in [0xff, 0xfe, 0x80, b'f', b'n'] {
            machine.source_push(byte);
        }
        assert_eq!(machine.compile(), 0);
    }

    #[test]
    fn an_enormous_program_is_cut_off_instead_of_eating_memory() {
        let mut machine = Machine::new(1);
        for _ in 0..(MAX_SOURCE * 2) {
            machine.source_push(b' ');
        }
        assert_eq!(machine.source.len(), MAX_SOURCE);
    }

    // ----------------------------------------------------------- sketches

    #[test]
    fn a_sketch_fills_the_screen_and_animates() {
        let mut machine = running(PLASMA);
        machine.tick(40.0);
        let first = machine.frame.clone();
        assert!(distinct_cells(&machine) > 8, "plenty of shades");
        run_for(&mut machine, 400);
        assert_ne!(machine.frame, first);
    }

    #[test]
    fn nothing_is_drawn_before_the_first_tick_and_a_zero_tick_changes_nothing() {
        let mut machine = running(PLASMA);
        let blank = machine.frame.clone();
        machine.tick(0.0);
        assert_eq!(machine.frame, blank);
        machine.tick(40.0);
        assert_ne!(machine.frame, blank);
    }

    #[test]
    fn the_mouse_changes_a_sketch_that_uses_it() {
        let make = |x, y| {
            let mut machine = running(PLASMA);
            machine.mouse_event(0, x, y);
            run_for(&mut machine, 400);
            machine.frame
        };
        assert_ne!(make(10, 5), make(50, 18));
        assert_eq!(make(10, 5), make(10, 5), "and it is deterministic");
    }

    #[test]
    fn holding_the_mouse_button_changes_a_sketch_that_checks_for_it() {
        let rings = include_str!("rings.rs");
        let make = |down: bool| {
            let mut machine = running(rings);
            machine.mouse_event(0, 20, 10);
            if down {
                machine.mouse_event(1, 20, 10);
            }
            run_for(&mut machine, 400);
            machine.frame
        };
        assert_ne!(make(true), make(false));
    }

    #[test]
    fn a_program_that_stops_working_says_why_and_stops_doing_work() {
        let source = "fn main() -> std::io::Result<()> { sketch(|cell| { let mut s = 0.0; for i in 0..2000000000 { s += 1.0; } s }) }";
        let mut machine = running(source);
        machine.tick(40.0);
        let why = machine.failure.clone().expect("it should have failed");
        assert!(why.contains("too much work"), "{why}");
        let stopped = machine.frame.clone();
        run_for(&mut machine, 400);
        assert_eq!(machine.frame, stopped, "it no longer redraws");
    }

    #[test]
    fn running_a_new_program_clears_the_failure() {
        let source = "fn main() -> std::io::Result<()> { sketch(|cell| { let mut s = 0.0; for i in 0..2000000000 { s += 1.0; } s }) }";
        let mut machine = running(source);
        machine.tick(40.0);
        assert!(machine.failure.is_some());
        assert_ne!(load(&mut machine, PLASMA), 0);
        assert!(machine.failure.is_none());
        machine.tick(40.0);
        assert!(machine.failure.is_none());
    }

    #[test]
    fn time_restarts_when_a_program_is_run_again() {
        let mut machine = running(PLASMA);
        run_for(&mut machine, 800);
        assert!(machine.seconds > 0.5);
        assert_ne!(load(&mut machine, PLASMA), 0);
        assert_eq!(machine.seconds, 0.0);
    }

    #[test]
    fn a_huge_gap_between_ticks_is_cut_down_so_nothing_jumps() {
        let mut machine = running(PLASMA);
        machine.tick(1_000_000.0);
        assert!(machine.seconds <= 0.3, "{}", machine.seconds);
    }

    // -------------------------------------------------------------- worlds

    #[test]
    fn a_world_draws_balls_and_they_move() {
        let mut machine = running(BALLS);
        run_for(&mut machine, 200);
        let early = machine.frame.clone();
        assert!(distinct_cells(&machine) > 1);
        run_for(&mut machine, 600);
        assert_ne!(machine.frame, early);
    }

    #[test]
    fn the_mouse_steers_a_world() {
        let make = |x, y| {
            let mut machine = running(BALLS);
            machine.mouse_event(0, x, y);
            run_for(&mut machine, 1200);
            machine.frame
        };
        assert_ne!(make(5, 20), make(58, 3));
    }

    #[test]
    fn a_click_adds_a_ball_to_a_world() {
        let positions = |machine: &Machine| machine.simulation.as_ref().unwrap().positions().len();
        let mut machine = running(BALLS);
        run_for(&mut machine, 200);
        let before = positions(&machine);
        machine.mouse_event(1, 30, 12);
        machine.mouse_event(2, 30, 12);
        run_for(&mut machine, 120);
        assert_eq!(positions(&machine), before + 1);
    }

    #[test]
    fn a_click_adds_exactly_one_ball_however_many_frames_pass() {
        let mut machine = running(BALLS);
        run_for(&mut machine, 80);
        let before = machine.simulation.as_ref().unwrap().positions().len();
        machine.mouse_event(1, 30, 12);
        run_for(&mut machine, 2000);
        assert_eq!(
            machine.simulation.as_ref().unwrap().positions().len(),
            before + 1
        );
    }

    #[test]
    fn the_right_button_pushes_balls_away() {
        let make = |push: bool| {
            let mut machine = running(BALLS);
            machine.mouse_event(0, 32, 12);
            if push {
                machine.mouse_event(3, 32, 12);
            }
            run_for(&mut machine, 1500);
            machine.frame
        };
        assert_ne!(make(true), make(false));
    }

    #[test]
    fn the_same_seed_gives_the_same_world_and_another_seed_a_different_one() {
        let make = |seed| {
            let mut machine = Machine::new(seed);
            load(&mut machine, BALLS);
            run_for(&mut machine, 800);
            machine.frame
        };
        assert_eq!(make(3), make(3));
        assert_ne!(make(3), make(4));
    }

    // ------------------------------------------------------------- the mouse

    #[test]
    fn mouse_positions_off_the_screen_are_pulled_back_onto_it() {
        let mut machine = running(PLASMA);
        for (x, y) in [(-50, -50), (5000, 5000), (i32::MIN, i32::MAX), (0, 0)] {
            machine.mouse_event(0, x, y);
            assert!((0.0..COLUMNS as f32).contains(&machine.pointer.x));
            assert!((0.0..ROWS as f32).contains(&machine.pointer.y));
        }
    }

    #[test]
    fn unknown_mouse_events_are_ignored() {
        let mut machine = running(PLASMA);
        machine.mouse_event(0, 10, 10);
        let before = machine.pointer.clone();
        machine.mouse_event(99, 20, 20);
        machine.mouse_event(u32::MAX, 20, 20);
        assert_eq!(machine.pointer, before);
    }

    #[test]
    fn button_events_set_and_clear_the_buttons() {
        let mut machine = running(PLASMA);
        machine.mouse_event(1, 3, 3);
        assert!(machine.pointer.left);
        machine.mouse_event(2, 3, 3);
        assert!(!machine.pointer.left);
        machine.mouse_event(3, 3, 3);
        assert!(machine.pointer.right);
        machine.mouse_event(4, 3, 3);
        assert!(!machine.pointer.right);
        machine.mouse_event(5, 4, 4);
        assert!(machine.pointer.left);
        machine.mouse_event(6, 4, 4);
        assert!(machine.pointer.right);
    }

    // ------------------------------------------------------------- examples

    #[test]
    fn every_website_example_runs_animates_and_never_fails() {
        for preset in presets::PRESETS {
            let mut machine = Machine::new(5);
            assert_ne!(load(&mut machine, preset.source), 0, "{}", preset.id);
            run_for(&mut machine, 400);
            let early = machine.frame.clone();
            machine.mouse_event(0, 20, 8);
            machine.mouse_event(1, 20, 8);
            run_for(&mut machine, 1200);
            assert!(
                machine.failure.is_none(),
                "{}: {:?}",
                preset.id,
                machine.failure
            );
            assert_ne!(machine.frame, early, "{} should be moving", preset.id);
            assert!(
                distinct_cells(&machine) > 1,
                "{} should not be blank",
                preset.id
            );
        }
    }

    #[test]
    fn the_starter_runs_too() {
        let mut machine = running(presets::STARTER);
        run_for(&mut machine, 400);
        assert!(machine.failure.is_none());
        assert!(distinct_cells(&machine) > 3);
    }
}

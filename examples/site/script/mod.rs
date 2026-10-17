//! A small, strict Rust interpreter for art scripts.
//!
//! A browser cannot compile Rust, so the website editor and the README GIFs run
//! scripts with this instead. It understands the part of Rust that the art API
//! needs and is careful to refuse what real Rust would refuse (for example
//! `cell.x * 2`, because `2` is not an `f32`), so a script that runs here also
//! compiles in your own project.
//!
//! ```text
//! use terminal_graphics_interactive::prelude::*;
//!
//! fn main() -> std::io::Result<()> {
//!     sketch_with(Palette::fire(), |cell| (cell.x * 0.2 + cell.t).sin())
//! }
//! ```
//!
//! A script is `use` lines, optional helper functions (`fn name(a: f32) -> f32`)
//! and a `main` that ends by showing a scene: `sketch(...)`,
//! `sketch_with(palette, ...)` or `World::new()....run()`. Inside a formula you
//! can use `let`, `let mut`, assignment, `if`/`else`, `for i in 0..n`, the
//! `f32` maths methods, the constants `PI` and `TAU`, and your own functions.

mod compile;
mod eval;
pub mod ir;
pub mod lexer;

use std::cell::RefCell;
use std::fmt;
use terminal_graphics_interactive::{Cell, Palette, World};

/// A script that does not compile, with where and why.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub line: u32,
    pub column: u32,
    pub message: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error at {}:{}: {}",
            self.line, self.column, self.message
        )
    }
}

/// What a script shows.
#[derive(Clone, Debug)]
pub enum Scene {
    /// A formula coloured through a palette, every cell, every frame.
    Sketch(Palette),
    /// A physics world.
    World(World),
}

/// A compiled script.
pub struct Program {
    scene: Scene,
    functions: Vec<ir::Function>,
    closure: Option<ir::Closure>,
    /// The values the formula copied from `main`.
    captured: Vec<ir::Value>,
}

impl Program {
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// The formula of a sketch scene, ready to be called for each cell.
    pub fn formula(&self) -> Formula<'_> {
        Formula {
            program: self,
            interp: RefCell::new(eval::Interp::new(&self.functions)),
            failure: RefCell::new(None),
        }
    }
}

/// A sketch's formula. Call it for every cell; if it fails (it ran for too
/// long, say) it returns 0.0, stops doing any more work, and remembers why in
/// [`Formula::failure`].
pub struct Formula<'a> {
    program: &'a Program,
    interp: RefCell<eval::Interp<'a>>,
    failure: RefCell<Option<String>>,
}

impl Formula<'_> {
    pub fn call(&self, cell: &Cell) -> f32 {
        // once it has failed, do not burn the work budget again for every cell
        if self.failure.borrow().is_some() {
            return 0.0;
        }
        let Some(closure) = &self.program.closure else {
            return 0.0;
        };
        let result = self
            .interp
            .borrow_mut()
            .call_closure(closure, &self.program.captured, cell);
        match result {
            Ok(value) => value,
            Err(error) => {
                *self.failure.borrow_mut() = Some(error.0);
                0.0
            }
        }
    }

    /// Why the formula stopped working, if it did.
    pub fn failure(&self) -> Option<String> {
        self.failure.borrow().clone()
    }
}

/// Compiles a script, or says what is wrong with it.
pub fn compile(source: &str) -> Result<Program, CompileError> {
    let compiled = compile::compile_source(source)?;
    let mut interp = eval::Interp::new(&compiled.functions);
    interp
        .run_main(&compiled.main)
        .map_err(|error| CompileError {
            line: 1,
            column: 1,
            message: error.0,
        })?;
    let Some(scene) = interp.scene.take() else {
        return Err(CompileError {
            line: 1,
            column: 1,
            message: "`main` did not show a scene".into(),
        });
    };
    let captured = std::mem::take(&mut interp.captured);
    Ok(Program {
        scene,
        functions: compiled.functions,
        closure: compiled.closure,
        captured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_graphics_interactive::Colour;

    // ------------------------------------------------------------ helpers

    const PLASMA: &str = include_str!("../../plasma.rs");
    const BALLS: &str = include_str!("../../balls.rs");

    fn script(body: &str) -> String {
        format!(
            "use terminal_graphics_interactive::prelude::*;\n\
             fn main() -> std::io::Result<()> {{\n    sketch(|cell| {body})\n}}\n"
        )
    }

    fn the_cell() -> Cell {
        Cell {
            x: 3.0,
            y: 4.0,
            t: 2.0,
            mouse_x: 6.0,
            mouse_y: 8.0,
            mouse_down: false,
            width: 80.0,
            height: 48.0,
        }
    }

    /// The value of the expression `body` for [`the_cell`].
    fn value(body: &str) -> f32 {
        let program = compile(&script(body)).unwrap_or_else(|e| panic!("{e}\n{}", script(body)));
        let formula = program.formula();
        let result = formula.call(&the_cell());
        assert_eq!(formula.failure(), None);
        result
    }

    fn error(source: &str) -> CompileError {
        match compile(source) {
            Ok(_) => panic!("expected an error from:\n{source}"),
            Err(error) => error,
        }
    }

    fn expect_error(body: &str, contains: &str) -> CompileError {
        let e = error(&script(body));
        assert!(
            e.message.contains(contains),
            "expected {contains:?} in the error for `{body}`, got: {}",
            e.message
        );
        e
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    // ----------------------------------------------------------- arithmetic

    #[test]
    fn arithmetic_follows_the_usual_precedence() {
        assert_eq!(value("1.0 + 2.0 * 3.0"), 7.0);
        assert_eq!(value("(1.0 + 2.0) * 3.0"), 9.0);
        assert_eq!(value("10.0 - 4.0 - 3.0"), 3.0);
        assert_eq!(value("12.0 / 4.0 / 3.0"), 1.0);
        assert_eq!(value("7.0 % 4.0"), 3.0);
        assert_eq!(value("2.0 + 7.0 % 4.0 * 2.0"), 8.0);
    }

    #[test]
    fn unary_minus_and_not() {
        assert_eq!(value("-2.0 * 3.0"), -6.0);
        assert_eq!(value("- -2.0"), 2.0);
        assert_eq!(value("if !(1.0 < 2.0) { 1.0 } else { 0.0 }"), 0.0);
    }

    #[test]
    fn comparisons_and_logic_choose_branches() {
        assert_eq!(value("if 1.0 < 2.0 { 5.0 } else { 6.0 }"), 5.0);
        assert_eq!(
            value("if 2.0 <= 2.0 && 3.0 > 1.0 { 5.0 } else { 6.0 }"),
            5.0
        );
        assert_eq!(
            value("if 1.0 == 2.0 || 3.0 != 3.0 { 5.0 } else { 6.0 }"),
            6.0
        );
        assert_eq!(
            value("if 3.0 > 5.0 { 1.0 } else if 3.0 > 2.0 { 2.0 } else { 3.0 }"),
            2.0
        );
    }

    #[test]
    fn division_by_zero_is_infinity_not_a_crash() {
        assert!(value("1.0 / 0.0").is_infinite());
        assert!(value("0.0 / 0.0").is_nan());
    }

    // -------------------------------------------------------- the cell

    #[test]
    fn the_cell_fields_are_available() {
        assert_eq!(value("cell.x"), 3.0);
        assert_eq!(value("cell.y"), 4.0);
        assert_eq!(value("cell.t"), 2.0);
        assert_eq!(value("cell.mouse_x"), 6.0);
        assert_eq!(value("cell.mouse_y"), 8.0);
        assert_eq!(value("cell.width"), 80.0);
        assert_eq!(value("cell.height"), 48.0);
        assert_eq!(value("if cell.mouse_down { 1.0 } else { 0.0 }"), 0.0);
    }

    #[test]
    fn the_distance_to_the_mouse_matches_the_library() {
        let cell = the_cell();
        assert!(near(value("cell.dist_to_mouse()"), cell.dist_to_mouse()));
        assert!(near(value("cell.dist_to_mouse()"), 5.0));
    }

    #[test]
    fn the_cell_may_be_called_anything() {
        let source = "fn main() -> std::io::Result<()> { sketch(|c: &Cell| c.x + 1.0) }";
        let program = compile(source).unwrap();
        assert_eq!(program.formula().call(&the_cell()), 4.0);
    }

    // ----------------------------------------------------------- f32 maths

    #[test]
    fn single_argument_maths_methods() {
        assert!(near(value("cell.t.sin()"), 2.0f32.sin()));
        assert!(near(value("cell.t.cos()"), 2.0f32.cos()));
        assert!(near(value("cell.t.tan()"), 2.0f32.tan()));
        assert!(near(value("cell.x.sqrt()"), 3.0f32.sqrt()));
        assert!(near(value("(-cell.x).abs()"), 3.0));
        assert!(near(value("(cell.x / 2.0).floor()"), 1.0));
        assert!(near(value("(cell.x / 2.0).ceil()"), 2.0));
        assert!(near(value("(cell.x / 2.0).round()"), 2.0));
        assert!(near(value("(cell.x / 2.0).fract()"), 0.5));
        assert!(near(value("cell.t.exp()"), 2.0f32.exp()));
        assert!(near(value("cell.t.ln()"), 2.0f32.ln()));
        assert!(near(value("(-cell.t).signum()"), -1.0));
        assert!(near(value("cell.t.tanh()"), 2.0f32.tanh()));
        assert!(near(value("cell.t.to_radians()"), 2.0f32.to_radians()));
    }

    #[test]
    fn maths_methods_with_arguments() {
        assert!(near(value("cell.x.powf(2.0)"), 9.0));
        assert!(near(value("cell.x.powi(3)"), 27.0));
        assert!(near(value("cell.y.atan2(cell.x)"), 4.0f32.atan2(3.0)));
        assert!(near(value("cell.x.min(cell.y)"), 3.0));
        assert!(near(value("cell.x.max(cell.y)"), 4.0));
        assert!(near(value("cell.x.hypot(cell.y)"), 5.0));
        assert!(near(value("cell.x.clamp(0.0, 1.0)"), 1.0));
        assert!(near(value("(-1.0f32).rem_euclid(3.0)"), 2.0));
        assert!(near(value("cell.x.mul_add(2.0, 1.0)"), 7.0));
    }

    #[test]
    fn constants_by_any_of_their_usual_paths() {
        assert!(near(value("std::f32::consts::PI"), std::f32::consts::PI));
        assert!(near(value("f32::consts::PI"), std::f32::consts::PI));
        assert!(near(value("consts::TAU"), std::f32::consts::TAU));
        assert!(near(value("PI"), std::f32::consts::PI));
        assert!(near(value("std::f32::consts::E"), std::f32::consts::E));
    }

    // -------------------------------------------------- let, loops, helpers

    #[test]
    fn let_bindings_and_blocks() {
        assert_eq!(value("{ let a = 2.0; let b = a * 3.0; a + b }"), 8.0);
        assert_eq!(
            value("{ let a = 2.0; let a = a + 1.0; a }"),
            3.0,
            "shadowing"
        );
        assert_eq!(value("{ let a: f32 = 2.5; a }"), 2.5);
    }

    #[test]
    fn mutable_variables_and_compound_assignment() {
        assert_eq!(
            value("{ let mut a = 1.0; a += 2.0; a *= 3.0; a -= 1.0; a /= 2.0; a }"),
            4.0
        );
        assert_eq!(value("{ let mut a = 1.0; a = a + 5.0; a }"), 6.0);
    }

    #[test]
    fn for_loops_over_ranges() {
        assert_eq!(
            value("{ let mut s = 0.0; for i in 0..4 { s += i as f32; } s }"),
            6.0
        );
        assert_eq!(
            value("{ let mut s = 0.0; for i in 1..=4 { s += i as f32; } s }"),
            10.0
        );
        assert_eq!(
            value("{ let mut s = 0.0; for _ in 0..0 { s += 1.0; } s }"),
            0.0
        );
    }

    #[test]
    fn a_loop_can_add_up_waves() {
        // five sine waves of rising frequency
        let expected: f32 = (1..=5).map(|k| (3.0 * k as f32).sin() / k as f32).sum();
        assert!(near(
            value("{ let mut s = 0.0; for k in 1..=5 { let f = k as f32; s += (cell.x * f).sin() / f; } s }"),
            expected
        ));
    }

    #[test]
    fn helper_functions_with_numbers_and_with_the_cell() {
        let source = "\
fn double(v: f32) -> f32 { v * 2.0 }
fn ring(c: &Cell, speed: f32) -> f32 { (c.x - c.t * speed).abs() }
fn main() -> std::io::Result<()> { sketch(|cell| double(ring(cell, 1.0))) }";
        let program = compile(source).unwrap();
        assert!(near(
            program.formula().call(&the_cell()),
            2.0 * (3.0f32 - 2.0).abs()
        ));
    }

    #[test]
    fn helpers_may_be_defined_after_main() {
        let source = "\
fn main() -> std::io::Result<()> { sketch(|cell| triple(cell.x)) }
fn triple(v: f32) -> f32 { v * 3.0 }";
        assert_eq!(compile(source).unwrap().formula().call(&the_cell()), 9.0);
    }

    #[test]
    fn a_formula_may_use_constants_defined_in_main() {
        let source = "\
fn main() -> std::io::Result<()> {
    let speed = 2.0;
    let scale = 0.5;
    sketch(|cell| cell.t * speed * scale)
}";
        assert_eq!(compile(source).unwrap().formula().call(&the_cell()), 2.0);
    }

    // ------------------------------------------------------------- scenes

    #[test]
    fn sketch_uses_the_ocean_palette_and_sketch_with_picks_one() {
        match compile(&script("0.0")).unwrap().scene() {
            Scene::Sketch(palette) => assert_eq!(palette, &Palette::ocean()),
            other => panic!("{other:?}"),
        }
        for (name, expected) in [
            ("fire", Palette::fire()),
            ("ocean", Palette::ocean()),
            ("rainbow", Palette::rainbow()),
            ("mono", Palette::mono()),
        ] {
            let source = format!(
                "fn main() -> std::io::Result<()> {{ sketch_with(Palette::{name}(), |cell| cell.x) }}"
            );
            match compile(&source).unwrap().scene() {
                Scene::Sketch(palette) => assert_eq!(palette, &expected, "{name}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_palette_of_your_own_colours() {
        let source = "fn main() -> std::io::Result<()> {
            sketch_with(Palette::new(&[Colour::Black, Colour::Blue, Colour::BrightCyan]), |cell| cell.x)
        }";
        match compile(source).unwrap().scene() {
            Scene::Sketch(palette) => assert_eq!(
                palette,
                &Palette::new(&[Colour::Black, Colour::Blue, Colour::BrightCyan])
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_palette_kept_in_a_variable() {
        let source = "fn main() -> std::io::Result<()> {
            let colours = Palette::rainbow();
            sketch_with(colours, |cell| cell.x)
        }";
        match compile(source).unwrap().scene() {
            Scene::Sketch(palette) => assert_eq!(palette, &Palette::rainbow()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_colour_name_works() {
        for name in [
            "Black",
            "Red",
            "Green",
            "Yellow",
            "Blue",
            "Magenta",
            "Cyan",
            "White",
            "BrightBlack",
            "BrightRed",
            "BrightGreen",
            "BrightYellow",
            "BrightBlue",
            "BrightMagenta",
            "BrightCyan",
            "BrightWhite",
        ] {
            let source = format!(
                "fn main() -> std::io::Result<()> {{
                    sketch_with(Palette::new(&[Colour::Black, Colour::{name}]), |cell| cell.x)
                }}"
            );
            assert!(compile(&source).is_ok(), "Colour::{name}");
        }
    }

    #[test]
    fn a_world_chain_builds_the_same_world_as_the_library() {
        let source = "fn main() -> std::io::Result<()> {
            World::new().balls(7).size(2.5).gravity(0.03).mouse_pulls().trails().palette(Palette::ocean()).run()
        }";
        let expected = World::new()
            .balls(7)
            .size(2.5)
            .gravity(0.03)
            .mouse_pulls()
            .trails()
            .palette(Palette::ocean());
        match compile(source).unwrap().scene() {
            Scene::World(world) => assert_eq!(format!("{world:?}"), format!("{expected:?}")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_bare_world_is_the_default_world() {
        let source = "fn main() -> std::io::Result<()> { World::new().run() }";
        match compile(source).unwrap().scene() {
            Scene::World(world) => assert_eq!(format!("{world:?}"), format!("{:?}", World::new())),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn world_settings_may_be_expressions_and_use_constants() {
        let source = "fn main() -> std::io::Result<()> {
            let big = 3.0;
            World::new().balls(2 + 3).size(big + 0.5).gravity(1.0 / 50.0).run()
        }";
        let expected = World::new().balls(5).size(3.5).gravity(0.02);
        match compile(source).unwrap().scene() {
            Scene::World(world) => assert_eq!(format!("{world:?}"), format!("{expected:?}")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_question_mark_and_ok_at_the_end_are_accepted() {
        let source = "fn main() -> std::io::Result<()> {
            World::new().balls(3).run()?;
            Ok(())
        }";
        assert!(compile(source).is_ok());
        let source = "fn main() -> std::io::Result<()> {
            sketch(|cell| cell.x)?;
            Ok(())
        }";
        assert!(compile(source).is_ok());
    }

    #[test]
    fn use_lines_and_comments_are_ignored() {
        let source = "// my art\nuse std::f32::consts::PI;\nuse terminal_graphics_interactive::prelude::*;\n\
             /* main */ fn main() -> std::io::Result<()> { sketch(|cell| cell.x * PI) }";
        assert!(compile(source).is_ok());
    }

    // ------------------------------------------------- the real examples

    #[test]
    fn the_plasma_example_compiles_and_matches_the_native_formula() {
        let program = compile(PLASMA).unwrap();
        match program.scene() {
            Scene::Sketch(palette) => assert_eq!(palette, &Palette::fire()),
            other => panic!("{other:?}"),
        }
        let formula = program.formula();
        for step in 0..200 {
            let cell = Cell {
                x: (step % 17) as f32 * 1.5,
                y: (step % 11) as f32 * 2.0,
                t: step as f32 * 0.033,
                mouse_x: (step % 7) as f32 * 4.0,
                mouse_y: (step % 5) as f32 * 4.0,
                mouse_down: step % 3 == 0,
                width: 80.0,
                height: 48.0,
            };
            let ripple = cell.dist_to_mouse() * 0.25 - cell.t * 3.0;
            let native = ((cell.x * 0.15 + cell.t).sin()
                + (cell.y * 0.2 - cell.t * 0.7).sin()
                + ripple.sin())
                / 3.0;
            assert!(near(formula.call(&cell), native), "step {step}");
        }
        assert_eq!(formula.failure(), None);
    }

    #[test]
    fn the_balls_example_builds_the_same_world_as_written() {
        let expected = World::new()
            .balls(5)
            .size(3.5)
            .gravity(0.02)
            .mouse_pulls()
            .trails();
        match compile(BALLS).unwrap().scene() {
            Scene::World(world) => assert_eq!(format!("{world:?}"), format!("{expected:?}")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_program_that_needs_a_real_terminal_gets_a_helpful_error() {
        let source = include_str!("../../quickstart.rs");
        let e = error(source);
        assert!(
            e.message.contains("cannot find") && e.message.contains("Terminal"),
            "{}",
            e.message
        );
    }

    // -------------------------------------------------------------- errors

    #[test]
    fn text_is_refused_where_it_appears_with_a_clear_message() {
        let e = error(
            "fn main() -> std::io::Result<()> {\n    let a = \"hi\";\n    sketch(|cell| cell.x)\n}",
        );
        assert!(
            e.message.contains("text and characters are not supported"),
            "{}",
            e.message
        );
        assert_eq!((e.line, e.column), (2, 13));
    }

    #[test]
    fn a_script_needs_a_main() {
        assert!(error("").message.contains("fn main"));
        assert!(error("use std::f32::consts::PI;")
            .message
            .contains("fn main"));
    }

    #[test]
    fn main_must_show_something() {
        let e = error("fn main() -> std::io::Result<()> { Ok(()) }");
        assert!(
            e.message.contains("sketch") && e.message.contains("World"),
            "{}",
            e.message
        );
    }

    #[test]
    fn showing_two_things_is_refused() {
        let e = error(
            "fn main() -> std::io::Result<()> {
                sketch(|cell| cell.x)?;
                World::new().run()
            }",
        );
        assert!(e.message.contains("one"), "{}", e.message);
    }

    #[test]
    fn syntax_errors_say_what_was_expected_and_where() {
        let e = error("fn main() -> std::io::Result<()> {\n    sketch(|cell| cell.x\n}");
        assert!(e.message.contains("expected `)`"), "{}", e.message);
        assert_eq!(e.line, 3);

        let e = error("fn main() -> std::io::Result<()> { sketch(|cell| cell.x +) }");
        assert!(e.message.contains("expected"), "{}", e.message);
    }

    #[test]
    fn unknown_names_are_found_at_their_position_with_a_suggestion() {
        let e = error("fn main() -> std::io::Result<()> {\n    sketch(|cell| cel.x)\n}");
        assert!(
            e.message.contains("cannot find value `cel`"),
            "{}",
            e.message
        );
        assert!(e.message.contains("did you mean `cell`"), "{}", e.message);
        assert_eq!((e.line, e.column), (2, 19));
    }

    #[test]
    fn unknown_fields_methods_functions_palettes_and_colours() {
        let e = expect_error("cell.z", "no field `z`");
        assert!(e.message.contains("x"), "{}", e.message);
        expect_error("cell.x.sinn()", "no method named `sinn`");
        expect_error("cell.x.sinn()", "did you mean `sin`");
        expect_error("wobble(cell.x)", "cannot find function `wobble`");
        let e = error(
            "fn main() -> std::io::Result<()> { sketch_with(Palette::lava(), |cell| cell.x) }",
        );
        assert!(
            e.message.contains("no function `lava`") && e.message.contains("fire"),
            "{}",
            e.message
        );
        let e = error(
            "fn main() -> std::io::Result<()> { sketch_with(Palette::new(&[Colour::Pink, Colour::Red]), |cell| cell.x) }",
        );
        assert!(e.message.contains("no colour `Pink`"), "{}", e.message);
    }

    #[test]
    fn mixing_numbers_and_whole_numbers_is_refused_like_rustc_does() {
        let e = expect_error("cell.x * 2", "mismatched types");
        assert!(
            e.message.contains("f32") && e.message.contains("integer"),
            "{}",
            e.message
        );
        expect_error("cell.x + 1", "mismatched types");
        expect_error("{ let n = 3; cell.x * n }", "mismatched types");
    }

    #[test]
    fn a_formula_must_return_a_number() {
        expect_error("cell.x < 1.0", "expected `f32`");
        expect_error("{ }", "expected `f32`");
    }

    #[test]
    fn conditions_must_be_true_or_false_and_branches_must_agree() {
        expect_error("if cell.x { 1.0 } else { 2.0 }", "expected `bool`");
        expect_error(
            "if cell.x > 1.0 { 1.0 } else { cell.x > 2.0 }",
            "mismatched types",
        );
        expect_error("if cell.x > 1.0 { 1.0 }", "`if` may be missing an `else`");
    }

    #[test]
    fn arguments_are_counted_and_typed() {
        expect_error("cell.x.powf()", "takes 1 argument but 0 were supplied");
        expect_error("cell.x.sin(1.0)", "takes 0 arguments but 1 was supplied");
        expect_error("cell.x.powi(2.0)", "mismatched types");
        let e = error("fn main() -> std::io::Result<()> { World::new().balls(5.0).run() }");
        assert!(e.message.contains("mismatched types"), "{}", e.message);
        let e = error("fn main() -> std::io::Result<()> { World::new().gravity().run() }");
        assert!(e.message.contains("takes 1 argument"), "{}", e.message);
    }

    #[test]
    fn variables_must_be_mutable_to_change() {
        let e = expect_error(
            "{ let a = 1.0; a += 1.0; a }",
            "cannot assign twice to immutable variable `a`",
        );
        assert_eq!(e.line, 3);
    }

    #[test]
    fn helper_function_calls_are_checked_against_their_signature() {
        let source = "fn f(v: f32) -> f32 { v }\nfn main() -> std::io::Result<()> { sketch(|cell| f(cell.x, 1.0)) }";
        let e = error(source);
        assert!(
            e.message.contains("takes 1 argument but 2 were supplied"),
            "{}",
            e.message
        );
        let source = "fn f(v: f32) -> f32 { v > 1.0 }\nfn main() -> std::io::Result<()> { sketch(|cell| f(cell.x)) }";
        assert!(error(source).message.contains("mismatched types"));
    }

    #[test]
    fn world_settings_cannot_use_the_cell() {
        let e = error("fn main() -> std::io::Result<()> { World::new().gravity(cell.x).run() }");
        assert!(
            e.message.contains("cannot find value `cell`"),
            "{}",
            e.message
        );
    }

    #[test]
    fn errors_have_one_based_positions_that_point_at_the_problem() {
        let e = error("fn main() -> std::io::Result<()> {\n\n    sketch(|cell| cell.x * 2)\n}");
        assert_eq!(e.line, 3);
        assert!(e.column > 1);
        assert!(e.to_string().starts_with("error at 3:"));
    }

    // ---------------------------------------------------- runtime safety

    #[test]
    fn a_runaway_loop_is_stopped_and_reported_not_hung() {
        let program = compile(&script(
            "{ let mut s = 0.0; for i in 0..2000000000 { s += 1.0; } s }",
        ))
        .unwrap();
        let formula = program.formula();
        assert_eq!(formula.call(&the_cell()), 0.0);
        let why = formula.failure().expect("it should have failed");
        assert!(why.contains("too much work"), "{why}");
    }

    #[test]
    fn runaway_recursion_is_stopped_and_reported() {
        let source = "fn f(v: f32) -> f32 { f(v + 1.0) }\nfn main() -> std::io::Result<()> { sketch(|cell| f(cell.x)) }";
        let program = compile(source).unwrap();
        let formula = program.formula();
        assert_eq!(formula.call(&the_cell()), 0.0);
        assert!(formula.failure().unwrap().contains("too deep"));
    }

    #[test]
    fn the_work_limit_applies_to_each_cell_separately() {
        let program = compile(&script(
            "{ let mut s = 0.0; for i in 0..200 { s += 1.0; } s }",
        ))
        .unwrap();
        let formula = program.formula();
        for _ in 0..5000 {
            assert_eq!(formula.call(&the_cell()), 200.0);
        }
        assert_eq!(formula.failure(), None);
    }

    #[test]
    fn nothing_panics_on_any_prefix_of_the_real_examples() {
        for source in [PLASMA, BALLS] {
            let chars: Vec<char> = source.chars().collect();
            for end in 0..=chars.len() {
                let prefix: String = chars[..end].iter().collect();
                let _ = compile(&prefix);
            }
        }
    }

    #[test]
    fn nothing_panics_on_odd_scripts() {
        for source in [
            "fn",
            "fn main",
            "fn main(",
            "fn main()",
            "fn main() {",
            "fn main() -> {}",
            "fn main() -> std::io::Result<()> { sketch( }",
            "fn main() { sketch(||) }",
            "fn main() { sketch(|a, b| a) }",
            "fn main() { World::new(.run() }",
            "fn main() { for in 0..3 {} }",
            "fn main() { let = 1; }",
            "fn f(a) {}",
            "fn main() { sketch(|cell| cell..x) }",
            "}}}}",
            "((((",
            "|||",
            "&&&",
        ] {
            let _ = compile(source);
        }
    }
}

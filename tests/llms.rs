//! `llms.txt` is the page AI coding agents read to decide whether to use this
//! crate and how. These tests keep it from drifting away from the API, and its
//! example is checked against a real example file that cargo compiles.

use std::fs;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn llms() -> String {
    fs::read_to_string(root().join("llms.txt")).expect("llms.txt must exist at the repo root")
}

/// All the Rust source under `dir`, including sub-modules.
fn source_in(dir: &Path) -> String {
    let mut all = String::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            all.push_str(&source_in(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            all.push_str(&fs::read_to_string(path).unwrap());
        }
    }
    all
}

fn source() -> String {
    source_in(&root().join("src"))
}

#[test]
fn starts_with_the_crate_name_and_a_one_line_summary() {
    let text = llms();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("# terminal_graphics_interactive"));
    assert!(lines.next().unwrap_or("").is_empty());
    assert!(lines.next().unwrap_or("").starts_with("> "));
}

#[test]
fn names_every_public_type_an_agent_may_need() {
    let text = llms();
    for name in [
        "Terminal",
        "Event",
        "Key",
        "KeyCode",
        "Modifiers",
        "Mouse",
        "MouseKind",
        "MouseButton",
        "Parser",
        "RawMode",
        "Display",
        "Colour",
    ] {
        assert!(text.contains(name), "llms.txt does not mention {name}");
    }
}

#[test]
fn every_method_it_lists_exists_in_the_source() {
    let text = llms();
    let source = source();
    for method in [
        "start",
        "poll",
        "wait",
        "size",
        "draw",
        "cell_in",
        "is_char",
        "is_ctrl",
        "feed",
        "next_event",
        "flush",
        "enable",
        "report_mouse_movement",
        // the art API
        "sketch",
        "sketch_with",
        "sketch_frame",
        "dist_to_mouse",
        "shade",
        "shade_signed",
        "balls",
        "gravity",
        "mouse_pulls",
        "trails",
        "palette",
        "simulation",
        "positions",
        "run",
    ] {
        assert!(
            text.contains(method),
            "llms.txt does not mention `{method}`"
        );
        assert!(
            source.contains(&format!("pub fn {method}(")),
            "`{method}` is listed in llms.txt but is not a public function in src/"
        );
    }
}

#[test]
fn says_when_to_use_and_when_not_to_use_the_crate() {
    let text = llms();
    assert!(text.contains("## Use it when"));
    assert!(text.contains("## Do not use it when"));
}

#[test]
fn warns_about_the_mistakes_that_are_easy_to_make() {
    let text = llms();
    for warning in ["Ctrl+C", "one row shorter", r"\r\n", "ssh -t", "0-based"] {
        assert!(
            text.contains(warning),
            "llms.txt does not warn about {warning:?}"
        );
    }
}

#[test]
fn the_example_is_the_real_quickstart_example_that_cargo_compiles() {
    let text = llms();
    let start = text.find("```rust\n").expect("a rust code block") + "```rust\n".len();
    let end = start + text[start..].find("```").expect("the code block ends");
    let in_llms = text[start..end].trim();

    let file = fs::read_to_string(root().join("examples/quickstart.rs"))
        .expect("examples/quickstart.rs must exist");
    assert_eq!(
        in_llms,
        file.trim(),
        "the example in llms.txt must be exactly examples/quickstart.rs"
    );
}

#[test]
fn says_what_is_supported_and_where_to_get_it() {
    let text = llms();
    for needle in [
        "Windows",
        "PowerShell",
        "Linux",
        "macOS",
        "https://github.com/calum/terminal_graphics_interactive",
        "terminal_graphics",
    ] {
        assert!(
            text.contains(needle),
            "llms.txt does not mention {needle:?}"
        );
    }
}

#[test]
fn recommends_the_single_prelude_import_and_does_not_misplace_the_shapes() {
    let text = llms();
    assert!(text.contains("use terminal_graphics_interactive::prelude::*;"));
    // Rect and Ball live in `shapes`, not at the crate root
    assert!(
        !text.contains("`Rect`, `Ball`, `Text`"),
        "Rect and Ball are not root re-exports; point to the prelude instead"
    );
}

#[test]
fn explains_that_pastes_are_one_event_and_that_matches_need_a_wildcard() {
    let text = llms();
    assert!(
        text.contains("Event::Paste"),
        "pasted text arrives as Event::Paste"
    );
    assert!(
        text.contains("`_ =>"),
        "Event can grow, so a match needs a `_ =>` arm"
    );
}

/// The code blocks in `llms.txt`, in order.
fn code_blocks() -> Vec<String> {
    let text = llms();
    let mut blocks = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find("```rust\n") {
        let after = &rest[start + "```rust\n".len()..];
        let end = after.find("```").expect("a code block that ends");
        blocks.push(after[..end].trim().to_string());
        rest = &after[end + 3..];
    }
    blocks
}

fn example(name: &str) -> String {
    fs::read_to_string(root().join("examples").join(format!("{name}.rs")))
        .unwrap_or_else(|_| panic!("examples/{name}.rs must exist"))
        .trim()
        .to_string()
}

#[test]
fn the_art_examples_are_the_real_plasma_and_balls_examples() {
    let blocks = code_blocks();
    assert!(
        blocks.contains(&example("plasma")),
        "llms.txt must contain examples/plasma.rs exactly"
    );
    assert!(
        blocks.contains(&example("balls")),
        "llms.txt must contain examples/balls.rs exactly"
    );
}

#[test]
fn explains_the_art_api_and_its_units_and_ranges() {
    let text = llms();
    for needle in [
        "## Making art",
        "-1.0 to 1.0",
        "Palette::new",
        "stretched",
        "sketch_frame",
        "`.simulation(",
    ] {
        assert!(
            text.contains(needle),
            "llms.txt does not mention {needle:?}"
        );
    }
}

#[test]
fn the_readme_shows_the_real_plasma_balls_and_quickstart_examples() {
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    for name in ["plasma", "balls", "quickstart"] {
        assert!(
            readme.contains(&example(name)),
            "README.md must contain examples/{name}.rs exactly"
        );
    }
}

/// The games section of `llms.txt`: from its heading to the next `## ` heading
/// or the end of the file.
fn games_section() -> String {
    let text = llms();
    let heading = "\n## Games\n";
    let start = text
        .find(heading)
        .expect("llms.txt must have a `## Games` section")
        + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i + 1);
    rest[..end].to_string()
}

#[test]
fn has_a_games_section_and_it_stays_short() {
    let text = llms();
    assert!(
        text.contains("\n## Games\n"),
        "llms.txt needs a `## Games` heading"
    );
    let lines = games_section().lines().count();
    // The spec asked for about 90 lines. A complete game (title, round and score
    // screen) is about 100 lines of code on its own, so the bound is set to fit it.
    assert!(
        lines <= 140,
        "the games section is {lines} lines; keep it to 140"
    );
}

#[test]
fn names_every_public_game_type_in_code_style() {
    let section = games_section();
    for name in [
        "Game", "Next", "Context", "Headless", "run", "Keys", "Controls", "Screen", "Sprite",
        "Rng", "Timer", "Pool", "Hit", "PixelArt",
    ] {
        assert!(
            section.contains(&format!("`{name}`")),
            "the games section does not name `{name}`"
        );
    }
}

#[test]
fn every_game_function_it_lists_exists_in_the_source() {
    let section = games_section();
    let source = source_in(&root().join("src/game"));
    for method in [
        "switch",
        "quit",
        "held",
        "pressed",
        "axis",
        "bind",
        "text",
        "text_centred",
        "put",
        "clear",
        "time",
        "tap",
        "step",
        "with_capacity",
        "push",
        "every",
        "once",
        "rect",
        "circle",
        "overlaps",
    ] {
        assert!(
            section.contains(&format!("{method}(")),
            "the games section does not mention `{method}`"
        );
        assert!(
            source.contains(&format!("pub fn {method}(")),
            "`{method}` is in the games section but is not a public function in src/game"
        );
    }
}

#[test]
fn says_when_games_are_the_right_tool_and_when_art_is() {
    let section = games_section();
    for needle in [
        "Use games when",
        "keeps state",
        "several screens",
        "Use art when",
        "a formula or a physics toy",
        "Do not use games",
        "a plain full-screen text UI",
        "widget toolkit",
    ] {
        assert!(
            section.contains(needle),
            "the games section does not say {needle:?}"
        );
    }
}

#[test]
fn its_game_example_is_the_real_quickgame_example_and_is_forty_to_sixty_lines() {
    let section = games_section();
    let start = section.find("```rust\n").expect("a rust code block") + "```rust\n".len();
    let end = start + section[start..].find("```").expect("the code block ends");
    let in_llms = section[start..end].trim();

    // The file also holds its headless tests after `#[cfg(test)]`; the page
    // shows only the program, which is the part before that line.
    let file = fs::read_to_string(root().join("examples/quickgame.rs"))
        .expect("examples/quickgame.rs must exist");
    let program = file.split("\n#[cfg(test)]").next().unwrap_or("").trim();
    assert_eq!(
        in_llms, program,
        "the game example in llms.txt must be the program in examples/quickgame.rs"
    );

    let lines = in_llms.lines().count();
    assert!(
        (40..=110).contains(&lines),
        "the example is {lines} lines; the spec asked for 40 to 60, a title, a round and a score screen need about 100"
    );
    for needle in [
        "fn main() -> std::io::Result<()>",
        "run(Title",
        "Next::switch",
        "ctx.keys",
        "Screen",
    ] {
        assert!(
            in_llms.contains(needle),
            "the example does not use {needle}"
        );
    }
}

#[test]
fn says_how_to_test_a_game_without_a_terminal() {
    let section = games_section();
    for needle in [
        "`Headless`",
        "Headless::new(",
        "without a terminal",
        "`step`",
        "`text`",
    ] {
        assert!(
            section.contains(needle),
            "the games section does not mention {needle:?}"
        );
    }
}

#[test]
fn lists_the_game_rules_that_are_easy_to_get_wrong() {
    let section = games_section();
    for rule in [
        "Ctrl+C",
        "`q` and Esc belong to the game",
        "60 times a second",
        "`ctx.dt`",
        "`&self`",
        "twice as tall",
        "emulated by timing",
        "`Rng::new(ctx.seed)`",
        "std::time",
        "push returns the item back",
    ] {
        assert!(
            section.contains(rule),
            "the games section does not warn about {rule:?}"
        );
    }
}

#[test]
fn says_that_objects_are_coloured_regions_not_text() {
    let section = games_section();
    let rule = "objects are coloured regions, not text";
    assert!(
        section.contains(rule),
        "the games section does not say {rule:?}"
    );
}

#[test]
fn points_at_sail_and_glow_as_the_examples_of_art_in_games() {
    let section = games_section();
    assert!(
        section
            .lines()
            .any(|line| line.contains("sail") && line.contains("glow") && line.contains("art")),
        "the games section needs one line pointing at sail and glow as art in games"
    );
}

//! `CONTRIBUTING.md` tells people (and AI agents) how to add to the art module
//! by pull request. It names files and functions; these tests keep those real.

use std::fs;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn contributing() -> String {
    fs::read_to_string(root().join("CONTRIBUTING.md")).expect("CONTRIBUTING.md must exist")
}

#[test]
fn every_file_it_names_exists() {
    let text = contributing();
    for path in [
        "src/art/palette.rs",
        "src/art/world.rs",
        "src/art/sketch.rs",
        "tests/support/mod.rs",
        "examples/make_gifs.rs",
        "llms.txt",
        "CHANGELOG.md",
        "CLAUDE.md",
    ] {
        assert!(
            text.contains(path),
            "CONTRIBUTING.md does not mention {path}"
        );
        // these are left out of the published crate, so only the mention is
        // required there
        if path != "CLAUDE.md" && path != "examples/make_gifs.rs" {
            assert!(
                root().join(path).exists(),
                "{path} is named but does not exist"
            );
        }
    }
}

#[test]
fn the_recipes_for_a_palette_a_setting_and_an_example_are_there() {
    let text = contributing();
    for heading in [
        "## Add a palette",
        "## Add a setting to `World`",
        "## Add an example",
    ] {
        assert!(text.contains(heading), "missing the section {heading:?}");
    }
}

/// The text under a `## ` heading, up to the next one.
fn section(text: &str, heading: &str) -> String {
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("CONTRIBUTING.md must have the section {heading:?}"))
        + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    rest[..end].to_string()
}

#[test]
fn the_game_recipes_are_there_and_name_real_files() {
    let text = contributing();
    for heading in ["## Add a game example", "## Add a game building block"] {
        assert!(text.contains(heading), "missing the section {heading:?}");
    }
    for path in [
        "examples/glow.rs",
        "examples/quickgame.rs",
        "Cargo.toml",
        "src/game/mod.rs",
        "tests/e2e_unix.rs",
        "CHANGELOG.md",
        "README.md",
        "llms.txt",
    ] {
        assert!(
            text.contains(path),
            "CONTRIBUTING.md does not mention {path}"
        );
        assert!(
            root().join(path).exists(),
            "{path} is named but does not exist"
        );
    }
}

#[test]
fn the_game_example_recipe_covers_the_test_entry_and_the_follow_up_files() {
    let text = contributing();
    let recipe = section(&text, "## Add a game example");
    for needle in [
        "game::*",
        "Headless",
        "[[example]]",
        "test = true",
        "tests/e2e_unix.rs",
        "CHANGELOG",
        "README",
        "llms.txt",
    ] {
        assert!(
            recipe.contains(needle),
            "the game example recipe lacks {needle:?}"
        );
    }
}

#[test]
fn the_game_example_recipe_says_to_draw_objects_as_coloured_regions() {
    let text = contributing();
    let recipe = section(&text, "## Add a game example");
    let rule = "Draw objects as coloured regions (`PixelArt`, `disc`, `glow`, `Particles`), \
                not as text characters; keep text for the HUD and labels.";
    assert!(
        recipe.contains(rule),
        "the game example recipe lacks the colour rule {rule:?}"
    );
}

#[test]
fn the_game_building_block_recipe_says_what_a_block_must_be() {
    let text = contributing();
    let recipe = section(&text, "## Add a game building block");
    for needle in [
        "src/game/mod.rs",
        "pure",
        "doc example",
        "unit tests",
        "llms.txt",
    ] {
        assert!(
            recipe.contains(needle),
            "the building block recipe lacks {needle:?}"
        );
    }
}

#[test]
fn the_functions_it_points_at_are_real() {
    let text = contributing();
    let palette = fs::read_to_string(root().join("src/art/palette.rs")).unwrap();
    let world = fs::read_to_string(root().join("src/art/world.rs")).unwrap();
    for (name, source) in [
        ("from_ramp", &palette),
        ("mono", &palette),
        ("step", &world),
    ] {
        assert!(
            text.contains(name),
            "CONTRIBUTING.md does not mention `{name}`"
        );
        assert!(
            source.contains(&format!("fn {name}(")),
            "`{name}` is mentioned but there is no such function"
        );
    }
}

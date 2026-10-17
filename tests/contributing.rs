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
        "llms.txt",
        "CHANGELOG.md",
        "CLAUDE.md",
    ] {
        assert!(
            text.contains(path),
            "CONTRIBUTING.md does not mention {path}"
        );
        // CLAUDE.md is left out of the published crate, so only the mention is
        // required there
        if path != "CLAUDE.md" {
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

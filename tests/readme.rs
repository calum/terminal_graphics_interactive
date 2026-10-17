//! The README is the front door, so it is checked: the GIFs it shows exist and
//! are real GIFs of a sensible size, every GIF in `assets/` is used, and it
//! points to the website and to how to install the crate.

use std::fs;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn readme() -> String {
    fs::read_to_string(root().join("README.md")).expect("README.md")
}

/// The GIF files in `assets/`, or none in the published crate, which leaves
/// `assets/` out.
fn gifs() -> Vec<String> {
    let Ok(entries) = fs::read_dir(root().join("assets")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".gif"))
        .collect();
    names.sort();
    names
}

const RAW: &str =
    "https://raw.githubusercontent.com/calum/terminal_graphics_interactive/master/assets/";

#[test]
fn the_readme_shows_the_plasma_and_the_balls_with_the_mouse() {
    let readme = readme();
    for name in ["plasma.gif", "balls.gif"] {
        assert!(
            readme.contains(&format!("{RAW}{name}")),
            "README.md must show {name} using its raw GitHub URL"
        );
    }
}

#[test]
fn every_gif_in_assets_is_used_and_is_a_real_gif_of_a_sensible_size() {
    let readme = readme();
    for name in gifs() {
        assert!(
            readme.contains(&format!("{RAW}{name}")),
            "assets/{name} is not shown in the README"
        );
        let bytes = fs::read(root().join("assets").join(&name)).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a", "{name} is not a GIF");
        assert_eq!(bytes.last(), Some(&0x3B), "{name} is cut short");
        assert!(bytes.len() > 10_000, "{name} is suspiciously small");
        assert!(bytes.len() < 3_000_000, "{name} is too heavy for a README");
        let (width, height) = (
            u16::from_le_bytes([bytes[6], bytes[7]]),
            u16::from_le_bytes([bytes[8], bytes[9]]),
        );
        assert!(
            (200..=700).contains(&width) && (200..=700).contains(&height),
            "{name}: {width}x{height}"
        );
    }
}

#[test]
fn the_readme_points_to_the_website_and_says_how_to_install() {
    let readme = readme();
    assert!(readme.contains("https://calum.github.io/terminal_graphics_interactive/"));
    assert!(readme.contains("cargo add terminal_graphics_interactive"));
}

#[test]
fn the_readme_links_to_the_things_a_visitor_needs() {
    let readme = readme();
    for link in [
        "llms.txt",
        "CONTRIBUTING.md",
        "https://docs.rs/terminal_graphics_interactive",
        "https://crates.io/crates/terminal_graphics_interactive",
    ] {
        assert!(readme.contains(link), "README.md should mention {link}");
    }
}

/// The `## Terminal Games` section: from its heading to the next `## ` heading.
fn games_section() -> String {
    let text = readme();
    let heading = "\n## Terminal Games\n";
    let start = text
        .find(heading)
        .expect("README.md needs a `## Terminal Games` section")
        + heading.len();
    let rest = &text[start..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i + 1);
    rest[..end].to_string()
}

#[test]
fn the_games_section_sits_next_to_the_art_section() {
    let text = readme();
    let art = text
        .find("## Make art in a few lines")
        .expect("the art section");
    let games = text
        .find("\n## Terminal Games\n")
        .expect("the games section");
    let mouse = text
        .find("## Add the mouse and keyboard")
        .expect("the mouse section");
    assert!(
        art < games && games < mouse,
        "the games section belongs between the art section and the mouse section"
    );
}

#[test]
fn the_games_section_says_what_it_is_and_how_to_run_the_two_games() {
    let section = games_section();
    for needle in [
        "cargo run --example glow",
        "cargo run --example quickgame",
        "Make games",
    ] {
        assert!(
            section.contains(needle),
            "the games section lacks {needle:?}"
        );
    }
}

#[test]
fn the_games_excerpt_is_short_and_every_line_is_real_quickgame_code() {
    let section = games_section();
    let start = section.find("```rust\n").expect("a rust excerpt") + "```rust\n".len();
    let excerpt = &section[start..start + section[start..].find("```").expect("it ends")];
    let lines = excerpt.lines().count();
    assert!(
        (10..=15).contains(&lines),
        "the excerpt is {lines} lines; keep it to 10 to 15"
    );
    for needle in ["Next::switch", "run(Title"] {
        assert!(excerpt.contains(needle), "the excerpt lacks {needle}");
    }
    let quickgame = fs::read_to_string(root().join("examples/quickgame.rs"))
        .expect("examples/quickgame.rs must exist");
    let file_lines: Vec<&str> = quickgame.lines().collect();
    for line in excerpt.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            file_lines.contains(&line),
            "the excerpt line {line:?} is not in examples/quickgame.rs"
        );
    }
}

#[test]
fn the_games_section_is_honest_about_what_is_verified() {
    let section = games_section();
    for needle in [
        "kitty keyboard protocol",
        "pseudo terminal",
        "not yet",
        "Windows",
        "macOS",
        "is coming",
        "Headless",
    ] {
        assert!(
            section.contains(needle),
            "the games section lacks {needle:?}"
        );
    }
}

#[test]
fn the_games_section_makes_no_speed_claims() {
    let section = games_section();
    for claim in ["faster", "per second", "fps", " ms"] {
        assert!(
            !section.contains(claim),
            "the games section must not claim speed ({claim:?}); the performance tables are the only numbers"
        );
    }
}

#[test]
fn the_readme_does_not_repeat_numbers_that_go_stale() {
    // counts of tests and the like belong in CI, not in prose that nobody updates
    let readme = readme();
    assert!(
        !readme.contains("end-to-end tests in a ConPTY"),
        "stale test counts"
    );
    assert!(
        !readme.contains("Status: early"),
        "a long status paragraph rots; keep it short"
    );
}

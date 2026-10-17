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

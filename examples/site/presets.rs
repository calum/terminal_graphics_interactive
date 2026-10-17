//! The examples shown on the website: each is a complete program that runs in
//! the editor and also compiles in a real project.

pub struct Preset {
    /// A short name for the button and the share link.
    pub id: &'static str,
    pub title: &'static str,
    /// One line about what to try.
    pub blurb: &'static str,
    pub source: &'static str,
}

/// What the editor starts with when nobody has chosen anything.
pub const STARTER: &str = r#"use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Change a number and press RUN.
    sketch(|cell| (cell.x * 0.2 + cell.t).sin())
}
"#;

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "plasma",
        title: "PLASMA",
        blurb: "A classic demoscene plasma. Move the mouse to make ripples.",
        source: include_str!("../plasma.rs"),
    },
    Preset {
        id: "rings",
        title: "RINGS",
        blurb: "Rainbow rings that follow the mouse. Hold the button for waves.",
        source: include_str!("../rings.rs"),
    },
    Preset {
        id: "tunnel",
        title: "TUNNEL",
        blurb: "Fly into a spinning tunnel. Hold the button to speed up.",
        source: include_str!("../tunnel.rs"),
    },
    Preset {
        id: "waves",
        title: "WAVES",
        blurb: "A rolling ocean with a glow under the mouse.",
        source: include_str!("../waves.rs"),
    },
    Preset {
        id: "balls",
        title: "BALLS",
        blurb: "Five big balls. The mouse pulls, right-click pushes, a click adds one.",
        source: include_str!("../balls.rs"),
    },
    Preset {
        id: "bubbles",
        title: "BUBBLES",
        blurb: "Bubbles drifting up. The mouse pulls them. Click to add one.",
        source: include_str!("../bubbles.rs"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::{compile, Scene};
    use std::collections::HashSet;

    #[test]
    fn there_is_a_good_choice_of_examples() {
        assert!(PRESETS.len() >= 6, "{} presets", PRESETS.len());
    }

    #[test]
    fn ids_are_unique_short_and_url_safe() {
        let mut seen = HashSet::new();
        for preset in PRESETS {
            assert!(seen.insert(preset.id), "duplicate id {}", preset.id);
            assert!(
                !preset.id.is_empty()
                    && preset.id.len() <= 12
                    && preset.id.chars().all(|c| c.is_ascii_lowercase()),
                "{:?}",
                preset.id
            );
        }
    }

    #[test]
    fn every_example_has_a_title_and_a_blurb_to_read() {
        for preset in PRESETS {
            assert!(
                preset.title.len() >= 3 && preset.title.len() <= 20,
                "{}",
                preset.id
            );
            assert!(
                preset.blurb.len() >= 15 && preset.blurb.len() <= 90,
                "{}: {}",
                preset.id,
                preset.blurb
            );
            assert!(preset.blurb.ends_with('.'), "{}", preset.id);
        }
    }

    #[test]
    fn every_example_and_the_starter_compile() {
        for preset in PRESETS {
            if let Err(error) = compile(preset.source) {
                panic!("{} does not compile: {error}\n{}", preset.id, preset.source);
            }
        }
        compile(STARTER).unwrap_or_else(|e| panic!("the starter does not compile: {e}"));
    }

    #[test]
    fn plasma_and_balls_are_exactly_the_repos_example_files() {
        let find = |id: &str| {
            PRESETS
                .iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("no {id} preset"))
        };
        assert_eq!(find("plasma").source, include_str!("../plasma.rs"));
        assert_eq!(find("balls").source, include_str!("../balls.rs"));
    }

    #[test]
    fn there_are_both_formulas_and_worlds_among_the_examples() {
        let mut sketches = 0;
        let mut worlds = 0;
        for preset in PRESETS {
            match compile(preset.source).unwrap().scene() {
                Scene::Sketch(_) => sketches += 1,
                Scene::World(_) => worlds += 1,
            }
        }
        assert!(sketches >= 4, "{sketches} sketches");
        assert!(worlds >= 2, "{worlds} worlds");
    }

    #[test]
    fn the_examples_use_the_import_a_real_project_would() {
        for preset in PRESETS {
            assert!(
                preset
                    .source
                    .contains("use terminal_graphics_interactive::prelude::*;"),
                "{}",
                preset.id
            );
            assert!(
                preset.source.contains("fn main() -> std::io::Result<()>"),
                "{}",
                preset.id
            );
        }
        assert!(STARTER.contains("use terminal_graphics_interactive::prelude::*;"));
    }

    #[test]
    fn the_examples_are_short_enough_to_read_in_a_glance() {
        for preset in PRESETS {
            let lines = preset.source.lines().count();
            assert!(lines <= 22, "{} is {lines} lines", preset.id);
        }
    }

    #[test]
    fn the_examples_have_a_comment_saying_what_to_try() {
        for preset in PRESETS {
            assert!(preset.source.contains("//"), "{} has no comment", preset.id);
        }
    }

    #[test]
    fn the_starter_is_tiny() {
        assert!(STARTER.lines().count() <= 8);
    }
}

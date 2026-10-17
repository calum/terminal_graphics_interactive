//! Builds the project website into a directory (default `target/site`).
//!
//! ```text
//! cargo build --release --example web_art --target wasm32-unknown-unknown
//! cargo build --release --example web_games --target wasm32-unknown-unknown
//! cargo run --example build_site -- [output-directory]
//! ```
//!
//! The site has three pages on one GitHub Pages site, with one look and one top
//! bar: the landing page at `/` (two doors, to the art and to the games), the
//! art studio at `/art/` and the games at `/games/`. The page is generated from
//! Rust so the repository stays 100% Rust: the colours come from `Colour`, the
//! examples are the repository's example files, and the API guide is generated
//! from the interpreter's own tables, so it cannot disagree with what the editor
//! understands. The compiled `web_art` and `web_games` programs are copied next
//! to their pages as `art.wasm` and `games.wasm`. The result is deployed to
//! GitHub Pages by `.github/workflows/pages.yml`.

#[allow(dead_code)]
#[path = "site/art_page.rs"]
mod art_page;
#[allow(dead_code)]
#[path = "site/games_page.rs"]
mod games_page;
#[allow(dead_code)]
#[path = "site/landing.rs"]
mod landing;
#[allow(dead_code)]
#[path = "site/presets.rs"]
mod presets;
#[allow(dead_code)]
#[path = "site/script/mod.rs"]
mod script;
#[allow(dead_code)]
#[path = "site/shell.rs"]
mod shell;

use std::fs;
use std::path::{Path, PathBuf};

const ART_WASM_PATH: &str = "target/wasm32-unknown-unknown/release/examples/web_art.wasm";
const GAMES_WASM_PATH: &str = "target/wasm32-unknown-unknown/release/examples/web_games.wasm";

/// Which compiled programs were found and copied.
#[derive(Debug, PartialEq, Eq)]
struct Built {
    art: bool,
    games: bool,
}

/// Writes the three pages into `out`, and each compiled program that exists
/// next to its page.
fn build_site(out: &Path, art_wasm: &Path, games_wasm: &Path) -> std::io::Result<Built> {
    fs::create_dir_all(out.join("art"))?;
    fs::create_dir_all(out.join("games"))?;
    fs::write(out.join("index.html"), landing::page())?;
    fs::write(out.join("art/index.html"), art_page::page())?;
    fs::write(out.join("games/index.html"), games_page::page())?;
    fs::write(out.join(".nojekyll"), "")?;
    Ok(Built {
        art: copy_if_found(art_wasm, &out.join("art/art.wasm"))?,
        games: copy_if_found(games_wasm, &out.join("games/games.wasm"))?,
    })
}

/// Copies a compiled program next to its page. Returns false if it was not built.
fn copy_if_found(from: &Path, to: &Path) -> std::io::Result<bool> {
    if from.exists() {
        fs::copy(from, to)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/site"), PathBuf::from);
    let built = build_site(&out, Path::new(ART_WASM_PATH), Path::new(GAMES_WASM_PATH))?;
    println!("site written to {}", out.display());
    if !built.art {
        eprintln!(
            "warning: {ART_WASM_PATH} not found, the art studio will not run. Build it with:\n  \
             cargo build --release --example web_art --target wasm32-unknown-unknown"
        );
    }
    if !built.games {
        eprintln!(
            "warning: {GAMES_WASM_PATH} not found, the games will not run yet. Build it with:\n  \
             cargo build --release --example web_games --target wasm32-unknown-unknown"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::shell::{top_bar, Page, CRATES_URL, SITE_URL};
    use super::*;

    /// Every page of the site, with the page it is.
    fn pages() -> [(&'static str, String, Page); 3] {
        [
            ("landing", landing::page(), Page::Landing),
            ("art", art_page::page(), Page::Art),
            ("games", games_page::page(), Page::Games),
        ]
    }

    // ------------------------------------------------------- every page alike

    #[test]
    fn every_page_has_the_same_retro_look() {
        for (name, page, _) in pages() {
            assert!(
                page.contains("repeating-linear-gradient"),
                "{name}: CRT scanlines"
            );
            assert!(
                page.contains("prefers-reduced-motion"),
                "{name}: reduced motion"
            );
            assert!(page.contains("NO CARRIER"), "{name}: the footer");
            assert!(
                page.contains("@keyframes rainbow"),
                "{name}: the rainbow title"
            );
            for i in 0..16 {
                assert!(page.contains(&format!("--c{i}:")), "{name}: colour {i}");
            }
        }
    }

    #[test]
    fn every_page_has_the_shared_top_bar_with_its_own_page_marked() {
        for (name, page, which) in pages() {
            assert!(page.contains(&top_bar(which)), "{name}: wrong top bar");
        }
    }

    #[test]
    fn every_page_has_its_own_canonical_address() {
        assert!(landing::page().contains(&format!("<link rel=\"canonical\" href=\"{SITE_URL}\">")));
        assert!(
            art_page::page().contains(&format!("<link rel=\"canonical\" href=\"{SITE_URL}art/\">"))
        );
        assert!(games_page::page().contains(&format!(
            "<link rel=\"canonical\" href=\"{SITE_URL}games/\">"
        )));
    }

    #[test]
    fn every_page_links_to_the_crate_on_crates_io() {
        for (name, page, _) in pages() {
            assert!(page.contains(CRATES_URL), "{name}: no link to crates.io");
        }
    }

    // ------------------------------------------------------------- writing it out

    #[test]
    fn the_site_is_three_pages_and_copies_each_program_it_finds() {
        let dir = std::env::temp_dir().join(format!("tgi_site_{}", std::process::id()));
        let art_wasm = dir.join("web_art.wasm");
        let games_wasm = dir.join("web_games.wasm");
        fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out");

        let none = Built {
            art: false,
            games: false,
        };
        assert_eq!(build_site(&out, &art_wasm, &games_wasm).unwrap(), none);
        for file in [
            "index.html",
            "art/index.html",
            "games/index.html",
            ".nojekyll",
        ] {
            assert!(out.join(file).exists(), "{file} was not written");
        }
        assert!(!out.join("art/art.wasm").exists());
        assert!(!out.join("games/games.wasm").exists());
        assert_eq!(
            fs::read_to_string(out.join("index.html")).unwrap(),
            landing::page()
        );
        assert_eq!(
            fs::read_to_string(out.join("art/index.html")).unwrap(),
            art_page::page()
        );
        assert_eq!(
            fs::read_to_string(out.join("games/index.html")).unwrap(),
            games_page::page()
        );

        fs::write(&art_wasm, b"\0asm").unwrap();
        let art_only = Built {
            art: true,
            games: false,
        };
        assert_eq!(build_site(&out, &art_wasm, &games_wasm).unwrap(), art_only);
        assert_eq!(fs::read(out.join("art/art.wasm")).unwrap(), b"\0asm");
        assert!(!out.join("games/games.wasm").exists());

        fs::write(&games_wasm, b"\0asm").unwrap();
        let both = Built {
            art: true,
            games: true,
        };
        assert_eq!(build_site(&out, &art_wasm, &games_wasm).unwrap(), both);
        assert_eq!(fs::read(out.join("games/games.wasm")).unwrap(), b"\0asm");

        fs::remove_dir_all(&dir).unwrap();
    }
}

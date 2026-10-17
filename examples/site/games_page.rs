//! The games page of the website (`/games/`). A placeholder for now: the games
//! are added to this page later, and it already shares the look and top bar.

use super::shell::{document, rainbow_title, Document, Page, SITE_URL};

const BODY: &str = r####"<h1 aria-label="TERMINAL GAMES">{{TITLE}}</h1>
<p>Games are coming to this page.</p>
"####;

/// The games page, at `/games/`.
pub fn page() -> String {
    let body = BODY.replace("{{TITLE}}", &rainbow_title("TERMINAL GAMES"));
    document(&Document {
        page: Page::Games,
        title: "Terminal Games - real games for the terminal, in Rust",
        description: "Real terminal games built with terminal_graphics_interactive. Change their numbers while they run.",
        canonical: &format!("{SITE_URL}games/"),
        head_script: "",
        css: "",
        body: &body,
        script: "",
    })
}

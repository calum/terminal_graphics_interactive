//! What every page of the website shares: the retro look, the top bar, the
//! footer, and the helpers that turn data into HTML. Each page module builds
//! its own body and the look is defined only here.

use terminal_graphics_interactive::Colour;

pub const SITE_URL: &str = "https://calum.github.io/terminal_graphics_interactive/";
pub const REPO_URL: &str = "https://github.com/calum/terminal_graphics_interactive";

/// The rules every page needs. `{{VARS}}`, `{{KEYFRAMES}}` and `{{CLASSES}}` are
/// filled in by `base_css`.
const BASE_CSS: &str = r####"
:root { {{VARS}} }
* { box-sizing: border-box; }
html { background: #000; }
body { margin: 0; background: #000; color: var(--c7); font: 16px/1.5 "Courier New", Courier, monospace; }
.crt { position: relative; min-height: 100vh; }
.crt::after { content: ""; position: fixed; inset: 0; pointer-events: none;
  background: repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0, rgba(0,0,0,0) 2px, rgba(0,0,0,.28) 3px);
  box-shadow: inset 0 0 140px rgba(0,0,0,.8); }
main { max-width: 860px; margin: 0 auto; padding: 24px 16px 64px; text-shadow: 0 0 4px currentColor; }
a { color: var(--c14); }
a:hover { background: var(--c14); color: #000; text-shadow: none; }
h1 { margin: 8px 0 0; font-size: clamp(24px, 7vw, 52px); letter-spacing: .12em; line-height: 1.2; }
h1 span { animation: rainbow 1.4s steps(1) infinite; }
@keyframes rainbow { {{KEYFRAMES}} }
h2 { color: var(--c11); font-size: 18px; margin: 40px 0 8px; }
h3 { color: var(--c14); font-size: 15px; margin: 22px 0 4px; }
.subtitle { color: var(--c8); margin: 4px 0 12px; }
.dim { color: var(--c8); font-size: 13px; }
.topbar { display: flex; flex-wrap: wrap; gap: 4px 14px; margin: 0 0 4px; font-size: 14px; }
.topbar a { text-decoration: none; padding: 2px 4px; }
.topbar a.here { background: var(--c11); color: #000; text-shadow: none; }
.prompt { background: #000; border: 1px solid var(--c8); padding: 8px 12px; overflow-x: auto; }
.cursor { animation: blink 1s steps(1) infinite; }
@keyframes blink { 50% { opacity: 0; } }
code { color: var(--c10); }
ul { padding-left: 20px; list-style: none; }
li::before { content: "> "; color: var(--c8); margin-left: -20px; }
button { font: inherit; background: #000; color: var(--c10); border: 1px solid var(--c10); padding: 6px 14px; cursor: pointer; }
button:hover, button:active { background: var(--c10); color: #000; text-shadow: none; }
.box { border: 1px solid var(--c8); background: #050505; padding: 10px 14px; text-shadow: none; }
.box h3 { margin-top: 0; }
.box p { margin: 6px 0; }
.box pre { margin: 6px 0; overflow-x: auto; color: var(--c7); font-size: 14px; }
.names code { display: inline-block; margin: 2px 8px 2px 0; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 14px; margin: 6px 0; }
dt { color: var(--c10); }
dd { margin: 0; }
{{CLASSES}}
@media (prefers-reduced-motion: reduce) { h1 span, .cursor { animation: none; } }
"####;

/// Makes text safe to put in a page or an attribute: it is shown, never run.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The title with every letter in its own span, so each can cycle through the
/// bright colours.
pub fn rainbow_title(text: &str) -> String {
    text.chars()
        .enumerate()
        .map(|(i, letter)| {
            if letter == ' ' {
                " ".to_string()
            } else {
                format!(
                    "<span style=\"animation-delay:-{}s\">{letter}</span>",
                    i as f32 * 0.2
                )
            }
        })
        .collect()
}

pub fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn css_variables() -> String {
    Colour::ALL
        .iter()
        .enumerate()
        .map(|(i, colour)| format!("--c{i}:{};", hex(colour.rgb())))
        .collect()
}

pub fn colour_classes() -> String {
    (0..Colour::ALL.len())
        .map(|i| format!(".c{i}{{color:var(--c{i});}}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cycles `color` through the 7 bright colours (the last 7 in `Colour::ALL`).
pub fn rainbow_keyframes() -> String {
    let bright = &Colour::ALL[9..];
    bright
        .iter()
        .enumerate()
        .map(|(i, colour)| {
            let percent = i as f32 * 100.0 / bright.len() as f32;
            format!("{percent:.1}% {{ color: {}; }}", hex(colour.rgb()))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn palette_swatches() -> String {
    Colour::ALL
        .iter()
        .enumerate()
        .map(|(i, colour)| {
            let (r, g, b) = colour.rgb();
            let light = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000 > 120;
            format!(
                "<div style=\"background:var(--c{i});color:{}\">{}</div>",
                if light { "#000" } else { "#fff" },
                colour.name()
            )
        })
        .collect()
}

/// The look every page shares: colours, CRT scanlines, title, and the common parts.
pub fn base_css() -> String {
    BASE_CSS
        .replace("{{VARS}}", &css_variables())
        .replace("{{KEYFRAMES}}", &rainbow_keyframes())
        .replace("{{CLASSES}}", &colour_classes())
}

pub const CRATES_URL: &str = "https://crates.io/crates/terminal_graphics_interactive";
pub const DOCS_URL: &str = "https://docs.rs/terminal_graphics_interactive";

/// Which page is being built. It decides which top bar link is the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Landing,
    Art,
    Games,
}

/// The top bar, with links that work from the page that shows it. The link to
/// the page itself is marked as the current one.
pub fn top_bar(current: Page) -> String {
    let (art, games) = match current {
        Page::Landing => ("art/", "games/"),
        Page::Art => ("./", "../games/"),
        Page::Games => ("../art/", "./"),
    };
    format!(
        "<nav class=\"topbar\" aria-label=\"Site\">{} {} {} {}</nav>",
        top_link(art, "ART", current == Page::Art),
        top_link(games, "GAMES", current == Page::Games),
        top_link(REPO_URL, "GitHub", false),
        top_link(CRATES_URL, "crates.io", false),
    )
}

fn top_link(href: &str, label: &str, current: bool) -> String {
    if current {
        format!("<a href=\"{href}\" class=\"here\" aria-current=\"page\">[ {label} ]</a>")
    } else {
        format!("<a href=\"{href}\">[ {label} ]</a>")
    }
}

/// Everything one page supplies to `document`.
pub struct Document<'a> {
    pub page: Page,
    pub title: &'a str,
    pub description: &'a str,
    pub canonical: &'a str,
    /// Script that runs in the head, before anything paints (may be empty).
    pub head_script: &'a str,
    /// Styles for this page only, after the shared ones.
    pub css: &'a str,
    pub body: &'a str,
    /// Script at the end of the body.
    pub script: &'a str,
}

/// The whole HTML page: the shared head, top bar and footer around the body.
pub fn document(d: &Document) -> String {
    let head_script = if d.head_script.is_empty() {
        String::new()
    } else {
        format!("<script>{}</script>\n", d.head_script)
    };
    let css = base_css() + d.css;
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{description}">
<link rel="canonical" href="{canonical}">
{head_script}<style>
{css}
</style>
</head>
<body>
<div class="crt">
<main>
{bar}
{body}
<footer>
<p class="dim">terminal_graphics_interactive v{version} - MIT OR Apache-2.0 - <a href="{REPO_URL}">view the source on GitHub</a></p>
<p class="dim">NO CARRIER<span class="cursor">_</span></p>
</footer>

</main>
</div>
<script>
{script}
</script>
</body>
</html>
"#,
        title = escape(d.title),
        description = escape(d.description),
        canonical = d.canonical,
        bar = top_bar(d.page),
        body = d.body,
        version = env!("CARGO_PKG_VERSION"),
        script = d.script,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(head_script: &str) -> String {
        document(&Document {
            page: Page::Art,
            title: "Test",
            description: "A test page",
            canonical: "https://example.test/",
            head_script,
            css: ".x{}",
            body: "<p>BODY</p>",
            script: "",
        })
    }

    #[test]
    fn the_top_bar_marks_art_as_the_current_link_on_the_art_page() {
        let bar = top_bar(Page::Art);
        assert_eq!(bar.matches("aria-current=\"page\"").count(), 1);
        assert!(bar.contains("<a href=\"./\" class=\"here\" aria-current=\"page\">[ ART ]</a>"));
        assert!(bar.contains("<a href=\"../games/\">[ GAMES ]</a>"));
    }

    #[test]
    fn the_top_bar_marks_games_as_the_current_link_on_the_games_page() {
        let bar = top_bar(Page::Games);
        assert_eq!(bar.matches("aria-current=\"page\"").count(), 1);
        assert!(bar.contains("<a href=\"../art/\">[ ART ]</a>"));
        assert!(bar.contains("<a href=\"./\" class=\"here\" aria-current=\"page\">[ GAMES ]</a>"));
    }

    #[test]
    fn the_landing_page_links_to_both_doors_and_marks_no_link_as_current() {
        let bar = top_bar(Page::Landing);
        assert!(!bar.contains("aria-current"));
        assert!(bar.contains("<a href=\"art/\">[ ART ]</a>"));
        assert!(bar.contains("<a href=\"games/\">[ GAMES ]</a>"));
    }

    #[test]
    fn the_top_bar_links_to_the_repo_and_the_crate_by_full_address() {
        let bar = top_bar(Page::Art);
        assert!(bar.contains(&format!("<a href=\"{REPO_URL}\">[ GitHub ]</a>")));
        assert!(bar.contains(&format!("<a href=\"{CRATES_URL}\">[ crates.io ]</a>")));
    }

    #[test]
    fn the_current_page_in_the_top_bar_is_shown_like_an_active_button() {
        let css = base_css();
        assert!(
            css.contains(".topbar a.here"),
            "no style for the current page"
        );
        assert!(css.contains(".topbar a.here { background: var(--c11)"));
    }

    #[test]
    fn the_top_bar_is_a_labelled_navigation_bar() {
        assert!(top_bar(Page::Art).starts_with("<nav class=\"topbar\" aria-label=\"Site\">"));
    }

    #[test]
    fn a_document_is_a_complete_page_with_the_shared_top_bar_body_and_footer() {
        let page = sample("");
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.contains("<title>Test</title>"));
        assert!(page.contains("<meta name=\"description\" content=\"A test page\">"));
        assert!(page.contains("<link rel=\"canonical\" href=\"https://example.test/\">"));
        assert!(page.contains(".x{}"), "the page's own css is missing");
        assert!(page.contains("<div class=\"crt\">\n<main>\n"));
        assert!(page.contains(&top_bar(Page::Art)));
        assert!(page.contains("<p>BODY</p>"));
        assert!(page.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
        assert!(page.contains("NO CARRIER"));
        assert!(page.contains("</main>\n</div>\n"));
        assert!(page.trim_end().ends_with("</html>"));
    }

    #[test]
    fn the_head_script_is_written_only_when_there_is_one() {
        assert_eq!(sample("").matches("<script>").count(), 1);
        assert_eq!(sample("f();").matches("<script>").count(), 2);
    }

    #[test]
    fn the_head_script_comes_before_the_body_so_it_runs_before_anything_paints() {
        let page = sample("window.early = 1;");
        let head = page.find("<script>window.early = 1;</script>");
        assert!(head.is_some(), "the head script is missing");
        assert!(head < page.find("<body>"));
    }
}

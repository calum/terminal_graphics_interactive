//! The front door of the website (`/`): two doors, to the art studio and to the
//! games. Old share links (`/#code=...`) are forwarded to the art studio.

use super::shell::{document, rainbow_title, Document, Page, SITE_URL};

/// Sends an old share link (`/#code=...`) on to the art studio, keeping the hash.
/// It sits in the head, so it runs before the landing page is painted.
const FORWARD: &str = "(function () { if (location.hash.length > 1) { location.replace(\"art/\" + location.hash); } })();";

const BODY: &str = r####"<h1 aria-label="TERMINAL GRAPHICS">{{TITLE}}</h1>
<p class="subtitle">art and games for the terminal, written in Rust | nothing to install</p>
<pre class="prompt"><span class="c10">$</span> cargo add terminal_graphics_interactive<span class="cursor">_</span></pre>

<noscript><p class="dim">JavaScript is off, so this page cannot send you on. Use one of the two doors below.</p></noscript>

<section class="doors">
<a class="door" href="art/">
<h2>&gt; TERMINAL ART</h2>
<p>Write a formula or a physics toy in a few lines of Rust and watch it move, mouse included.</p>
<p class="cta">ENTER THE STUDIO</p>
</a>
<a class="door" href="games/">
<h2>&gt; TERMINAL GAMES</h2>
<p>Play real games built with the same crate, change their numbers while they run, and read their source.</p>
<p class="cta">PLAY</p>
</a>
</section>
"####;

const CSS: &str = r####"
.doors { display: grid; gap: 16px; margin-top: 28px; }
@media (min-width: 640px) { .doors { grid-template-columns: 1fr 1fr; } }
.door { display: block; height: 100%; color: var(--c7); text-decoration: none; border: 2px solid var(--c10); background: #050505; padding: 14px 16px; }
.door:focus-visible { outline: 2px solid var(--c11); outline-offset: 2px; }
.door h2 { color: inherit; margin-top: 0; }
.door p { margin: 6px 0; }
.door .cta { color: var(--c11); }
.door:hover .cta, .door:focus-visible .cta { color: inherit; }
"####;

/// The front door, at the root of the site.
pub fn page() -> String {
    let body = BODY.replace("{{TITLE}}", &rainbow_title("TERMINAL GRAPHICS"));
    document(&Document {
        page: Page::Landing,
        title: "Terminal Graphics - art and games for the terminal, in Rust",
        description: "Terminal art you write in a few lines of Rust, and real terminal games you can change while they run. Nothing to install.",
        canonical: SITE_URL,
        head_script: FORWARD,
        css: CSS,
        body: &body,
        script: "",
    })
}

#[cfg(test)]
mod tests {
    use super::super::shell::SITE_URL;
    use super::*;

    const FORWARD: &str = "(function () { if (location.hash.length > 1) { location.replace(\"art/\" + location.hash); } })();";

    /// The part of the page before `<body>`, where the head script lives.
    fn head(page: &str) -> &str {
        &page[..page.find("<body>").expect("no <body>")]
    }

    #[test]
    fn it_has_one_door_to_the_art_studio_and_one_to_the_games() {
        let page = page();
        assert!(page.contains("<a class=\"door\" href=\"art/\">"));
        assert!(page.contains("<a class=\"door\" href=\"games/\">"));
        assert!(page.contains("<h2>&gt; TERMINAL ART</h2>"));
        assert!(page.contains("<h2>&gt; TERMINAL GAMES</h2>"));
        assert!(page.contains("ENTER THE STUDIO"));
        assert!(page.contains(">PLAY<"));
    }

    #[test]
    fn it_shows_the_title_and_the_way_to_install_the_crate() {
        let page = page();
        assert!(page.contains("<h1 aria-label=\"TERMINAL GRAPHICS\">"));
        assert!(
            page.contains("animation-delay"),
            "the title is rainbow lettered"
        );
        assert!(page.contains("cargo add terminal_graphics_interactive"));
    }

    #[test]
    fn it_forwards_an_old_share_link_to_the_art_studio_in_the_head_before_anything_paints() {
        let page = page();
        assert!(
            head(&page).contains(FORWARD),
            "the forwarding script is missing from the head"
        );
        assert!(!page.contains("location.replace(\"games/\""));
    }

    #[test]
    fn a_reader_with_javascript_off_is_told_to_use_the_doors() {
        assert!(page().contains("<noscript>"));
    }

    #[test]
    fn its_canonical_address_is_the_site_root() {
        assert!(page().contains(&format!("<link rel=\"canonical\" href=\"{SITE_URL}\">")));
    }

    #[test]
    fn every_placeholder_is_filled_in() {
        assert!(!page().contains("{{"), "a placeholder is left in the page");
    }
}

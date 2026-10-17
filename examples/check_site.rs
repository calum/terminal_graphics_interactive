//! Runs the built website in a headless browser and checks that it works.
//!
//! ```text
//! cargo build --release --example web_art --target wasm32-unknown-unknown
//! cargo run --example build_site
//! cargo run --release --example check_site [site-directory] [--require]
//! ```
//!
//! It serves the site from memory and disk on a local port, adds a test page
//! that takes control of the animation clock, drives the page's real buttons,
//! editor and mouse handlers with synthetic events, and reads back what
//! happened. The test script is a Rust string, so the repository stays 100%
//! Rust. Without a browser it says so and exits cleanly, unless `--require`
//! is given (CI does that).

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Replaces `requestAnimationFrame` with a clock the test steps by hand, so
/// every frame is deterministic and does not depend on the browser's timers.
const STUB: &str = "";

/// The checks, run inside the page. It reports one line per check on a
/// `<pre id=\"harness-report\">`: `ok: name` or `FAIL: name (detail)`.
const HARNESS: &str = "";

/// What the browser reported.
#[derive(Debug, PartialEq)]
struct Report {
    passed: Vec<String>,
    failed: Vec<String>,
}

/// Puts the clock stub in before the page's content and the checks after it.
fn with_harness(html: &str) -> String {
    let _ = html;
    String::new()
}

/// Reads the report out of the page the browser printed.
fn read_report(dom: &str) -> Option<Report> {
    let _ = dom;
    None
}

/// What to answer for a request path: status, content type and body.
fn route(site: &Path, path: &str) -> (u16, &'static str, Vec<u8>) {
    let _ = (site, path);
    (500, "text/plain", Vec::new())
}

/// A browser that can run headless, if one is installed.
fn find_browser() -> Option<PathBuf> {
    None
}

fn main() {
    let mut site = PathBuf::from("target/site");
    let mut require = false;
    for argument in std::env::args().skip(1) {
        if argument == "--require" {
            require = true;
        } else {
            site = PathBuf::from(argument);
        }
    }
    let _ = (Stdio::null(), Duration::ZERO, Instant::now(), &site);
    let _ = (fs::read, Command::new("x"), TcpListener::bind("x"));
    let _ = (std::io::stdin().read(&mut []), std::io::stdout().flush());
    if require {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str =
        "<html><body>\n<div class=\"crt\">x</div>\n<script>app()</script>\n</body>\n</html>\n";

    #[test]
    fn the_harness_is_added_around_the_page_without_changing_it() {
        let page = with_harness(PAGE);
        assert!(page.contains("app()"), "the page's own script is kept");
        let stub = page.find("__step").expect("the stub");
        let content = page.find("<div class=\"crt\">").expect("the content");
        let app = page.find("app()").expect("the app");
        let checks = page.find("harness-report").expect("the checks");
        assert!(
            stub < content && content < app && app < checks,
            "stub, page, then checks"
        );
        assert!(page.trim_end().ends_with("</html>"));
    }

    #[test]
    fn a_page_without_the_expected_markers_is_left_alone_rather_than_broken() {
        assert_eq!(with_harness("<p>hello</p>"), "<p>hello</p>");
    }

    #[test]
    fn the_stub_takes_over_animation_frames_and_can_step_them() {
        assert!(STUB.contains("requestAnimationFrame"));
        assert!(STUB.contains("__step"));
    }

    #[test]
    fn the_checks_cover_the_things_that_matter() {
        for needle in [
            "mousemove",
            "mousedown",
            "contextmenu",
            "data-example",
            "problems",
            "reset",
            "blank",
            "share",
            "runaway",
            "harness-report",
            "ok: ",
            "FAIL: ",
        ] {
            assert!(
                HARNESS.contains(needle),
                "the harness never mentions {needle}"
            );
        }
        assert!(!HARNESS.contains("{{"));
    }

    #[test]
    fn a_report_is_read_from_the_dumped_page_and_unescaped() {
        let dom = "<html><body><pre id=\"harness-report\">ok: start\nok: a &lt; b &amp; c\nFAIL: mouse (x &quot;y&quot;)\n</pre></body></html>";
        let report = read_report(dom).unwrap();
        assert_eq!(report.passed, vec!["start", "a < b & c"]);
        assert_eq!(report.failed, vec!["mouse (x \"y\")"]);
    }

    #[test]
    fn no_report_means_the_checks_never_finished() {
        assert_eq!(read_report("<html><body>nothing here</body></html>"), None);
        assert_eq!(read_report(""), None);
    }

    #[test]
    fn an_empty_report_is_not_a_pass() {
        let report = read_report("<pre id=\"harness-report\"></pre>").unwrap();
        assert!(report.passed.is_empty() && report.failed.is_empty());
    }

    fn site_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tgi_check_{name}_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("index.html"), PAGE).unwrap();
        fs::write(dir.join("art.wasm"), b"\0asm").unwrap();
        dir
    }

    #[test]
    fn the_server_serves_the_page_the_wasm_and_the_harness_page() {
        let dir = site_dir("serve");
        let (status, kind, body) = route(&dir, "/");
        assert_eq!((status, kind), (200, "text/html; charset=utf-8"));
        assert_eq!(body, PAGE.as_bytes());

        let (status, kind, body) = route(&dir, "/art.wasm");
        assert_eq!((status, kind), (200, "application/wasm"));
        assert_eq!(body, b"\0asm");

        let (status, _, body) = route(&dir, "/harness.html");
        assert_eq!(status, 200);
        assert_eq!(body, with_harness(PAGE).into_bytes());

        assert_eq!(route(&dir, "/missing.txt").0, 404);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_server_ignores_query_strings_and_never_leaves_the_site_directory() {
        let dir = site_dir("safe");
        assert_eq!(route(&dir, "/?x=1").0, 200);
        assert_eq!(route(&dir, "/index.html#top").0, 200);
        for sneaky in [
            "/../Cargo.toml",
            "/..%2FCargo.toml",
            "/%2e%2e/Cargo.toml",
            "//etc/passwd",
            "/a/../../x",
        ] {
            assert_eq!(route(&dir, sneaky).0, 404, "{sneaky}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finding_a_browser_does_not_panic() {
        let _ = find_browser();
    }
}

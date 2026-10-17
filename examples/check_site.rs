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
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Replaces `requestAnimationFrame` with a clock the test steps by hand, so
/// every frame is deterministic and does not depend on the browser's timers.
const STUB: &str = r####"
(function () {
  var pending = [], now = 0;
  window.requestAnimationFrame = function (callback) { pending.push(callback); return pending.length; };
  window.__step = function (ms) {
    now += ms;
    var run = pending;
    pending = [];
    run.forEach(function (callback) { callback(now); });
  };
})();
"####;

/// The checks, run inside the page. It reports one line per check on a
/// `<pre id="harness-report">`: `ok: name` or `FAIL: name (detail)`.
const HARNESS: &str = r####"
(function () {
  var lines = [];
  function check(name, good, detail) {
    lines.push(good ? "ok: " + name : "FAIL: " + name + (detail ? " (" + detail + ")" : ""));
  }
  function $(id) { return document.getElementById(id); }
  function frames(count) { for (var i = 0; i < count; i++) window.__step(16); }
  function sleep(ms) { return new Promise(function (done) { setTimeout(done, ms); }); }
  var canvas = $("screen"), ctx = canvas.getContext("2d");
  var editor = $("code"), problems = $("problems");

  // a cheap fingerprint of what is on the screen
  function picture() {
    var data = ctx.getImageData(0, 0, canvas.width, canvas.height).data, sum = 0;
    for (var i = 0; i < data.length; i += 13) sum = (sum * 31 + data[i] + 7 * data[i + 1] + 13 * data[i + 2]) | 0;
    return sum;
  }
  function mouse(type, x, y, button) {
    var rect = canvas.getBoundingClientRect();
    var event = new MouseEvent(type, {
      bubbles: true, cancelable: true, button: button || 0,
      clientX: rect.left + rect.width * x, clientY: rect.top + rect.height * y
    });
    (type === "mouseup" ? window : canvas).dispatchEvent(event);
    return event;
  }
  // wraps a formula in the smallest complete program
  function sketch(formula) {
    return "use terminal_graphics_interactive::prelude::*;\n\nfn main() -> std::io::Result<()> {\n    sketch(|cell| " + formula + ")\n}\n";
  }
  function setCode(text) { editor.value = text; $("run").click(); frames(3); }

  async function checks() {
    // the WebAssembly is compiled outside the stepped clock, so wait for it
    for (var tries = 0; tries < 300 && problems.textContent.indexOf("loading") === 0; tries++) await sleep(50);
    frames(3);
    check("start", problems.className.indexOf("good") >= 0, problems.textContent.slice(0, 80));
    check("editor has code", editor.value.length > 20);

    // every example loads, compiles and runs for a while without stopping
    var buttons = document.querySelectorAll("button[data-example]");
    check("data-example buttons exist", buttons.length >= 3, buttons.length + " found");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].click();
      frames(30);
      var name = "example " + buttons[i].dataset.example;
      check(name, editor.value.length > 0 && problems.className.indexOf("good") >= 0, problems.textContent.slice(0, 80));
    }

    // the mouse: moving it changes a program that follows it
    setCode(sketch("if (cell.x - cell.mouse_x).abs() < 3.0 { 1.0 } else { 0.0 }"));
    mouse("mousemove", 0.2, 0.5); frames(8);
    var left = picture();
    mouse("mousemove", 0.2, 0.5); frames(8);
    check("mousemove: same place, same picture", picture() === left);
    mouse("mousemove", 0.8, 0.5); frames(8);
    check("mousemove: elsewhere, different picture", picture() !== left);

    // buttons: left sets mouse_down, right alone does not
    setCode(sketch("if cell.mouse_down { 1.0 } else { 0.0 }"));
    var up = picture();
    mouse("mousedown", 0.5, 0.5, 0); frames(8);
    check("mousedown: the left button changes the picture", picture() !== up);
    mouse("mouseup", 0.5, 0.5, 0); frames(8);
    check("mouseup: releasing restores it", picture() === up);
    mouse("mousedown", 0.5, 0.5, 2); frames(8);
    check("mousedown: the right button alone does not set mouse_down", picture() === up);
    mouse("mouseup", 0.5, 0.5, 2);
    check("contextmenu is suppressed over the screen", mouse("contextmenu", 0.5, 0.5, 2).defaultPrevented);

    // an error is shown, with where it is, and the old program keeps going
    setCode(sketch("cell.x +"));
    check("a mistake is reported with its position",
      problems.className.indexOf("bad") >= 0 && /error at \d+:\d+/.test(problems.textContent), problems.textContent.slice(0, 80));
    check("the old program is said to be still running", problems.textContent.indexOf("still running") >= 0);

    // a runaway program is stopped, not hung
    setCode(sketch("{ let mut s = 0.0; for i in 0..2000000000 { s += 1.0; } s }"));
    frames(5);
    check("a runaway loop is stopped and reported",
      problems.className.indexOf("bad") >= 0 && problems.textContent.indexOf("stopped") >= 0, problems.textContent.slice(0, 80));

    // reset and blank
    buttons[0].click(); frames(2);
    var original = editor.value;
    editor.value = "nonsense";
    $("reset").click(); frames(2);
    check("reset brings the example back", editor.value === original);
    $("blank").click(); frames(2);
    check("blank loads the starter", editor.value !== original && editor.value.length > 0);

    // share puts the code in the address
    $("share").click();
    check("share puts the code in the link", location.hash.indexOf("#code=") === 0, "len=" + location.hash.length);
  }

  function finish() {
    var pre = document.createElement("pre");
    pre.id = "harness-report";
    pre.textContent = lines.join("\n");
    document.body.appendChild(pre);
    document.title = "RESULT";
  }

  checks().catch(function (error) { check("the checks ran to the end", false, String(error)); }).then(finish);
})();
"####;

/// What the browser reported.
#[derive(Debug, PartialEq)]
struct Report {
    passed: Vec<String>,
    failed: Vec<String>,
}

/// Puts the clock stub in before the page's content and the checks after it.
fn with_harness(html: &str) -> String {
    let (Some(content), Some(end)) = (html.find("<div class=\"crt\">"), html.rfind("</body>"))
    else {
        return html.to_string();
    };
    if content > end {
        return html.to_string();
    }
    let mut page = String::with_capacity(html.len() + STUB.len() + HARNESS.len() + 64);
    page.push_str(&html[..content]);
    page.push_str(&format!("<script>{STUB}</script>\n"));
    page.push_str(&html[content..end]);
    page.push_str(&format!("<script>{HARNESS}</script>\n"));
    page.push_str(&html[end..]);
    page
}

/// Reads the report out of the page the browser printed.
fn read_report(dom: &str) -> Option<Report> {
    let start = dom.find("id=\"harness-report\"")?;
    let body = &dom[start..];
    let body = &body[body.find('>')? + 1..];
    let body = &body[..body.find("</pre>")?];
    let text = body
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    let mut report = Report {
        passed: Vec::new(),
        failed: Vec::new(),
    };
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("ok: ") {
            report.passed.push(name.to_string());
        } else if let Some(name) = line.strip_prefix("FAIL: ") {
            report.failed.push(name.to_string());
        }
    }
    Some(report)
}

fn content_type(file: &str) -> &'static str {
    match file.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "wasm" => "application/wasm",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "gif" => "image/gif",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// What to answer for a request path: status, content type and body.
fn route(site: &Path, path: &str) -> (u16, &'static str, Vec<u8>) {
    let not_found = (404, "text/plain", b"not found".to_vec());
    let path = path.split(['?', '#']).next().unwrap_or("");
    let relative = path.trim_start_matches('/');
    // only plain names: no escapes, no climbing out of the site directory
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains('%')
        || path.contains('\\')
        || relative.split('/').any(|part| part == "..")
    {
        return not_found;
    }
    let (relative, harness) = match relative {
        "" => ("index.html", false),
        "harness.html" => ("index.html", true),
        other => (other, false),
    };
    match fs::read(site.join(relative)) {
        Ok(body) if harness => {
            let page = String::from_utf8_lossy(&body);
            (
                200,
                content_type("harness.html"),
                with_harness(&page).into_bytes(),
            )
        }
        Ok(body) => (200, content_type(relative), body),
        Err(_) => not_found,
    }
}

fn serve_one(site: &Path, mut stream: TcpStream) {
    let mut buffer = [0u8; 4096];
    let Ok(length) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..length]);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let (status, kind, body) = route(site, path);
    let reason = if status == 200 { "OK" } else { "Not Found" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// A browser that can run headless, if one is installed.
fn find_browser() -> Option<PathBuf> {
    if let Some(chosen) = std::env::var_os("BROWSER_PATH") {
        return Some(PathBuf::from(chosen));
    }
    let mut candidates: Vec<PathBuf> = [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
        "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    // Playwright keeps its browsers in one folder, one subfolder per version
    if let Ok(folders) = fs::read_dir("/opt/pw-browsers") {
        for folder in folders.flatten() {
            candidates.push(folder.path().join("chrome-linux").join("chrome"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
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
    if !site.join("index.html").is_file() {
        eprintln!("no built site in {}: run build_site first", site.display());
        std::process::exit(1);
    }
    let Some(browser) = find_browser() else {
        eprintln!("no Chrome, Chromium or Edge found; skipping (set BROWSER_PATH to choose one)");
        std::process::exit(if require { 1 } else { 0 });
    };

    let listener = TcpListener::bind("127.0.0.1:0").expect("a free local port");
    let port = listener.local_addr().expect("an address").port();
    let served = site.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let site = served.clone();
            std::thread::spawn(move || serve_one(&site, stream));
        }
    });

    let started = Instant::now();
    let output = Command::new(&browser)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--hide-scrollbars",
            "--window-size=1100,2300",
            "--virtual-time-budget=30000",
            "--dump-dom",
        ])
        .arg(format!("http://127.0.0.1:{port}/harness.html"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let dom = match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(error) => {
            eprintln!("could not run {}: {error}", browser.display());
            std::process::exit(1);
        }
    };

    let Some(report) = read_report(&dom) else {
        eprintln!(
            "the browser never produced a report after {:?}",
            Duration::from_millis(started.elapsed().as_millis() as u64)
        );
        std::process::exit(1);
    };
    for name in &report.passed {
        println!("ok: {name}");
    }
    for name in &report.failed {
        println!("FAIL: {name}");
    }
    println!(
        "{} passed, {} failed",
        report.passed.len(),
        report.failed.len()
    );
    if !report.failed.is_empty() || report.passed.is_empty() {
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

//! Runs the built website in a headless browser and checks that it works.
//!
//! ```text
//! cargo build --release --example web_art --example web_games --target wasm32-unknown-unknown
//! cargo run --example build_site
//! cargo run --release --example check_site [site-directory] [--require] [--screenshots DIR]
//! ```
//!
//! It serves each page of the site from memory and disk on a local port, adds a
//! test script that takes control of the animation clock, drives the page's real
//! buttons, editor, keyboard, touch and mouse handlers with synthetic events, and
//! reads back what happened. The test scripts are Rust strings, so the repository
//! stays 100% Rust. Four browser runs cover the site: the landing page, the art
//! studio, the games, and a forwarded old share link. Without a browser it says
//! so and exits cleanly, unless `--require` is given (CI does that).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The program the forwarding check shares: a valid sketch, as the art page runs it.
const FORWARD_PROGRAM: &str = "use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    sketch(|cell| (cell.x * 0.3 + cell.t).sin())
}";

/// Replaces `requestAnimationFrame` with a clock the test steps by hand, so
/// every frame is deterministic and does not depend on the browser's timers. It
/// also records script errors in `window.__errors`, which every check reports.
const STUB: &str = r####"
(function () {
  window.__errors = [];
  window.addEventListener("error", function (event) { window.__errors.push(String(event.message || event.error)); });
  window.addEventListener("unhandledrejection", function (event) { window.__errors.push("unhandled: " + String(event.reason)); });
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

/// What every check script shares: the report, the helpers, and the end. The
/// report is a `<pre id="harness-report">` with `ok: name` or `FAIL: name (detail)`.
const HELPERS: &str = r####"
  var lines = [];
  function check(name, good, detail) {
    lines.push(good ? "ok: " + name : "FAIL: " + name + (detail ? " (" + detail + ")" : ""));
  }
  function $(id) { return document.getElementById(id); }
  function frames(count) { for (var i = 0; i < count; i++) window.__step(16); }
  function sleep(ms) { return new Promise(function (done) { setTimeout(done, ms); }); }
  // waits until test() is true, polling every 50 ms for at most 15 seconds
  async function waitFor(test) {
    for (var tries = 0; tries < 300 && !test(); tries++) await sleep(50);
    return test();
  }
  // a cheap fingerprint of what is on a canvas
  function picture(canvas) {
    var data = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data, sum = 0;
    for (var i = 0; i < data.length; i += 13) sum = (sum * 31 + data[i] + 7 * data[i + 1] + 13 * data[i + 2]) | 0;
    return sum;
  }
  // how many different colours a canvas shows, sampled
  function colours(canvas) {
    var data = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data, seen = {}, count = 0;
    for (var i = 0; i < data.length; i += 4 * 97) {
      var key = data[i] + "," + data[i + 1] + "," + data[i + 2];
      if (!seen[key]) { seen[key] = true; count++; }
    }
    return count;
  }
  function keyEvent(element, type, name) {
    var event = new KeyboardEvent(type, { key: name, bubbles: true, cancelable: true });
    element.dispatchEvent(event);
    return event;
  }
  function pointer(element, type) {
    element.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, pointerId: 1, isPrimary: true }));
  }
  function finish() {
    var pre = document.createElement("pre");
    pre.id = "harness-report";
    pre.textContent = lines.join("\n");
    document.body.appendChild(pre);
    document.title = "RESULT";
    fetch("/done");
  }
"####;

/// How every check script ends: any error in the checks is reported, the page's
/// script errors are checked, and the report is written.
const RUN: &str = r####"
  checks().catch(function (error) { check("the checks ran to the end", false, String(error)); })
    .then(function () {
      check("no script errors", window.__errors.length === 0, window.__errors.join("; "));
      finish();
    });
"####;

/// The landing page: the top bar, the two doors and the title. No game is started.
const LANDING_CHECKS: &str = r####"
  var links = document.querySelectorAll("nav.topbar a");
  check("the top bar has four links", links.length === 4, links.length + " found");
  var names = Array.prototype.map.call(links, function (link) { return link.textContent; }).join(" ");
  check("the top bar names ART, GAMES, GitHub and crates.io",
    ["ART", "GAMES", "GitHub", "crates.io"].every(function (word) { return names.indexOf(word) >= 0; }), names);
  check("the landing page marks no link as the current page", document.querySelectorAll("[aria-current]").length === 0);
  check("the door to the art studio is there", !!document.querySelector('a.door[href="art/"]'));
  check("the door to the games is there", !!document.querySelector('a.door[href="games/"]'));
  var title = document.querySelector("h1");
  check("the title is there", !!title && title.textContent.indexOf("TERMINAL GRAPHICS") === 0, title ? title.textContent : "no h1");
  check("the page has its title", document.title.indexOf("Terminal Graphics") === 0, document.title);
  check("nothing was forwarded away from the landing page", /\/harness\.html$/.test(location.pathname), location.pathname);
"####;

/// The art studio: the editor, the examples, the mouse, the errors, the runaway
/// loop, reset, blank, share, and the top bar.
const ART_CHECKS: &str = r####"
  var canvas = $("screen"), editor = $("code"), problems = $("problems");
  // a sketch that uses the mouse, the way the studio's own examples do
  function sketch(formula) {
    return "use terminal_graphics_interactive::prelude::*;\n\nfn main() -> std::io::Result<()> {\n    sketch(|cell| " + formula + ")\n}\n";
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
  function setCode(text) { editor.value = text; $("run").click(); frames(3); }

  await waitFor(function () { return problems.textContent.indexOf("loading") !== 0; });
  frames(3);
  check("start", problems.className.indexOf("good") >= 0, problems.textContent.slice(0, 80));
  check("editor has code", editor.value.length > 20);

  var links = document.querySelectorAll("nav.topbar a");
  check("the top bar has four links", links.length === 4, links.length + " found");
  var current = document.querySelector("nav.topbar a[aria-current]");
  check("ART is marked as the current page", !!current && current.textContent.indexOf("ART") >= 0 && current.textContent.indexOf("GAMES") < 0,
    current ? current.textContent : "none");

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
  var left = picture(canvas);
  mouse("mousemove", 0.2, 0.5); frames(8);
  check("mousemove: same place, same picture", picture(canvas) === left);
  mouse("mousemove", 0.8, 0.5); frames(8);
  check("mousemove: elsewhere, different picture", picture(canvas) !== left);

  // buttons: left sets mouse_down, right alone does not
  setCode(sketch("if cell.mouse_down { 1.0 } else { 0.0 }"));
  var up = picture(canvas);
  mouse("mousedown", 0.5, 0.5, 0); frames(8);
  check("mousedown: the left button changes the picture", picture(canvas) !== up);
  mouse("mouseup", 0.5, 0.5, 0); frames(8);
  check("mouseup: releasing restores it", picture(canvas) === up);
  mouse("mousedown", 0.5, 0.5, 2); frames(8);
  check("mousedown: the right button alone does not set mouse_down", picture(canvas) === up);
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
"####;

/// The games: the gallery, the overlay, the keys (keyboard, touch and blur), the
/// mouse, pause, the tuner, the end of a game, and the sections of the page.
const GAMES_CHECKS: &str = r####"
  var canvas = $("screen"), overlay = $("overlay"), status = $("status");
  var sourceBox = $("source");
  var glowButton = document.querySelector('button[data-game="glow"]');
  var sailButton = document.querySelector('button[data-game="sail"]');
  var quickButton = document.querySelector('button[data-game="quickgame"]');
  var enterButton = document.querySelector('button[data-key="Enter"]');
  var pauseButton = document.querySelector('button[data-key="Pause"]');
  var rightButton = document.querySelector('button[data-key="Right"]');
  var expectedSliders = { glow: 2, sail: 3, quickgame: 0 };

  // the page's own restart, then the canvas has the keys again
  function restart() { $("restart").click(); frames(5); canvas.focus(); }
  // a key pressed and let go, with a frame between, as a keyboard does
  function tap(name) { keyEvent(canvas, "keydown", name); frames(2); keyEvent(canvas, "keyup", name); frames(10); }
  function mouse(type, x, y) {
    var rect = canvas.getBoundingClientRect();
    var event = new MouseEvent(type, {
      bubbles: true, cancelable: true, button: 0,
      clientX: rect.left + rect.width * x, clientY: rect.top + rect.height * y
    });
    (type === "mouseup" ? window : canvas).dispatchEvent(event);
  }
  function knob(name) { return document.querySelector('#tuner-sliders input[data-knob="' + name + '"]'); }
  function setKnob(input, value) {
    input.value = String(value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
  // a round of quickgame from its title: ArrowRight held for `hold` frames, or not at all, then `after` frames
  function round(holding, hold, after) {
    restart(); tap("Enter");
    if (holding) keyEvent(canvas, "keydown", "ArrowRight");
    frames(hold);
    if (holding) keyEvent(canvas, "keyup", "ArrowRight");
    frames(after);
    return picture(canvas);
  }
  // a round of glow from its title, with the mouse held at 90% across and 50% down, or not
  function glowRound(pressing) {
    restart(); tap("Enter");
    if (pressing) mouse("mousedown", 0.9, 0.5);
    frames(60);
    if (pressing) mouse("mouseup", 0.9, 0.5);
    frames(5);
    return picture(canvas);
  }

  await waitFor(function () { return status.textContent.indexOf("loading") !== 0; });
  frames(3);
  var started = status.textContent === "playing glow";
  check("the games load and glow is playing", started, status.textContent);
  if (!started) return;

  var buttons = document.querySelectorAll("button[data-game]");
  check("the gallery has three games", buttons.length === 3, buttons.length + " found");

  // the overlay asks for a click until the screen has the focus
  canvas.blur(); frames(2);
  check("click to play shows while the screen is not focused",
    overlay.textContent === "click to play" && !overlay.hidden, overlay.textContent + (overlay.hidden ? " (hidden)" : ""));
  canvas.focus(); frames(2);
  check("focusing the screen hides the overlay", overlay.hidden, overlay.textContent);
  check("the screen is not blank: at least four colours", colours(canvas) >= 4, colours(canvas) + " colours");

  // every game in the gallery: its button, its status, its picture, its source and its sliders
  var previous = null;
  for (var i = 0; i < buttons.length; i++) {
    var id = buttons[i].dataset.game;
    buttons[i].click();
    frames(30);
    check(id + ": its button is the active one", buttons[i].className === "active", buttons[i].className);
    check(id + ": the status names it", status.textContent === "playing " + id, status.textContent);
    var fingerprint = picture(canvas);
    check(id + ": the picture differs from the game before", previous === null || fingerprint !== previous);
    previous = fingerprint;
    check(id + ": its source is shown and has fn main",
      sourceBox.textContent.length > 1000 && sourceBox.textContent.indexOf("fn main") >= 0, "length " + sourceBox.textContent.length);
    var sliders = document.querySelectorAll("#tuner-sliders input[type=range]").length;
    check(id + ": " + expectedSliders[id] + " tuning sliders", sliders === expectedSliders[id], sliders + " found");
  }

  // keyboard on quickgame: Enter starts it, ArrowRight moves the player, release stops it
  quickButton.click(); frames(3); canvas.focus(); frames(2);
  restart();
  var titleBack = picture(canvas);
  tap("Enter");
  check("Enter starts the quickgame round", picture(canvas) !== titleBack);
  var held = round(true, 60, 5);
  var idle = round(false, 60, 5);
  check("holding ArrowRight moves the player: differs from holding nothing", held !== idle);
  check("the same keys give the same picture twice (deterministic)", round(true, 60, 5) === held);
  var early = round(true, 20, 45);
  check("releasing ArrowRight early stops the player: differs from holding it", early !== held);
  restart(); tap("Enter");
  var down = keyEvent(canvas, "keydown", "ArrowRight");
  check("ArrowRight is handled by the game, not by the page", down.defaultPrevented);
  keyEvent(canvas, "keyup", "ArrowRight"); frames(2);
  restart(); tap("Enter");
  keyEvent(canvas, "keydown", "ArrowRight"); frames(20);
  canvas.blur(); frames(45);
  check("leaving the screen lets go of the keys: same as releasing early", picture(canvas) === early);

  // the touch buttons send the same keys as the keyboard
  restart();
  var beforeEnter = picture(canvas);
  pointer(enterButton, "pointerdown"); frames(2); pointer(enterButton, "pointerup"); frames(10);
  check("the on-screen ENTER starts a game", picture(canvas) !== beforeEnter);
  restart(); tap("Enter");
  pointer(rightButton, "pointerdown"); frames(60); pointer(rightButton, "pointerup"); frames(5);
  check("holding the on-screen right button matches holding ArrowRight", picture(canvas) === held);

  // the mouse on glow pulls it toward the pointer
  glowButton.click(); frames(3); canvas.focus(); frames(2);
  var pulled = glowRound(true);
  var unpulled = glowRound(false);
  check("holding the mouse on glow changes the picture", pulled !== unpulled);

  // PAUSE on sail: a paused round stands still
  sailButton.click(); frames(3); canvas.focus(); frames(2);
  tap("Enter");
  pointer(pauseButton, "pointerdown"); frames(2); pointer(pauseButton, "pointerup"); frames(10);
  var pausedA = picture(canvas);
  frames(40);
  check("PAUSE on sail freezes the picture", picture(canvas) === pausedA);

  // the tuner: the fireball rate changes what sail draws, and its number shows
  sailButton.click(); frames(3); canvas.focus(); frames(2);
  tap("Enter");
  var fire = knob("fireball_rate");
  check("sail has a fireball rate slider", !!fire);
  setKnob(fire, fire.max); frames(600);
  var high = picture(canvas);
  var shown = fire.parentNode.querySelector("output").textContent;
  check("the slider's number shows its value", shown === Number(fire.value).toFixed(2), shown);
  restart(); tap("Enter");
  var fireLow = knob("fireball_rate");
  setKnob(fireLow, fireLow.min); frames(600);
  check("the fireball rate changes what sail draws", picture(canvas) !== high);
  check("sail's other two sliders exist", !!knob("wave_height") && !!knob("boat_speed"));

  // quickgame's title: q ends it, and RESTART brings the title back
  quickButton.click(); frames(3); canvas.focus(); frames(2);
  restart();
  var titleAgain = picture(canvas);
  keyEvent(canvas, "keydown", "q"); frames(2); keyEvent(canvas, "keyup", "q"); frames(3);
  check("q ends quickgame and the overlay says so",
    overlay.textContent === "the game ended - press RESTART" && !overlay.hidden, overlay.textContent);
  restart();
  check("RESTART brings the title back", overlay.hidden && picture(canvas) === titleAgain);

  // the other sections of the page
  check("the multiplayer section is there", !!$("multiplayer"));
  var make = $("make") ? $("make").textContent : "";
  check("make your own shows the cargo command", make.indexOf("cargo add terminal_graphics_interactive") >= 0);
  check("make your own has a complete program with fn main", make.indexOf("fn main") >= 0);
"####;

/// The forwarding check: an old share link on the landing page lands in the
/// studio with its code in the editor, compiled. `expected` is set before this.
const FORWARD_CHECKS: &str = r####"
  var editor = $("code"), problems = $("problems");
  await waitFor(function () { return problems.textContent.indexOf("loading") !== 0; });
  frames(3);
  check("the landing page forwarded to the art studio", /\/art\/$/.test(location.pathname), location.pathname);
  check("the address keeps the code", location.hash.indexOf("#code=") === 0, location.hash.slice(0, 12));
  check("the studio shows the forwarded program exactly", editor.value === expected, editor.value.slice(0, 60));
  check("the forwarded program compiled", problems.className.indexOf("good") >= 0, problems.textContent.slice(0, 80));
"####;

/// A check script from its checks, with the shared helpers around them.
fn harness_script(checks: &str) -> String {
    format!("(function () {{\n{HELPERS}\nasync function checks() {{\n{checks}\n}}\n{RUN}\n}})();")
}

fn landing_harness() -> String {
    harness_script(LANDING_CHECKS)
}

fn art_harness() -> String {
    harness_script(ART_CHECKS)
}

fn games_harness() -> String {
    harness_script(GAMES_CHECKS)
}

/// The forwarding check, which also knows the program it expects.
fn forward_harness() -> String {
    harness_script(&format!(
        "var expected = {};\n{FORWARD_CHECKS}",
        js_string(FORWARD_PROGRAM)
    ))
}

/// What the browser reported.
#[derive(Debug, PartialEq)]
struct Report {
    passed: Vec<String>,
    failed: Vec<String>,
}

/// Puts the clock stub in before the page's content and the checks after it.
fn with_harness(html: &str, harness: &str) -> String {
    let (Some(content), Some(end)) = (html.find("<div class=\"crt\">"), html.rfind("</body>"))
    else {
        return html.to_string();
    };
    if content > end {
        return html.to_string();
    }
    let mut page = String::with_capacity(html.len() + STUB.len() + harness.len() + 64);
    page.push_str(&html[..content]);
    page.push_str(&format!("<script>{STUB}</script>\n"));
    page.push_str(&html[content..end]);
    page.push_str(&format!("<script>{harness}</script>\n"));
    // the browser dumps the page once it has loaded, so keep it loading until the checks are done
    page.push_str("<script src=\"/wait.js\"></script>\n");
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

/// What the server does: `Normal` serves every page and its harness page
/// (`/harness.html`, `/art/harness.html`, `/games/harness.html`). `Forward` serves
/// the landing page as it is and the studio at `/art/` with the forwarding check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Normal,
    Forward,
}

/// The file a request is for (a directory means its index page), and the check
/// script to put in it. `None` when the request is not a page of this mode.
fn plan(relative: &str, mode: Mode) -> Option<(String, Option<String>)> {
    match (mode, relative) {
        (Mode::Normal, "harness.html") => Some(("index.html".into(), Some(landing_harness()))),
        (Mode::Normal, "art/harness.html") => Some(("art/index.html".into(), Some(art_harness()))),
        (Mode::Normal, "games/harness.html") => {
            Some(("games/index.html".into(), Some(games_harness())))
        }
        (Mode::Forward, "art/") => Some(("art/index.html".into(), Some(forward_harness()))),
        (Mode::Forward, "harness.html" | "art/harness.html" | "games/harness.html") => None,
        _ => {
            let file = if relative.is_empty() || relative.ends_with('/') {
                format!("{relative}index.html")
            } else {
                relative.to_string()
            };
            Some((file, None))
        }
    }
}

/// What to answer for a request path: status, content type and body.
fn route(site: &Path, path: &str, mode: Mode) -> (u16, &'static str, Vec<u8>) {
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
    let Some((file, harness)) = plan(relative, mode) else {
        return not_found;
    };
    match fs::read(site.join(&file)) {
        Ok(body) => match harness {
            Some(script) => {
                let page = String::from_utf8_lossy(&body);
                (
                    200,
                    content_type(&file),
                    with_harness(&page, &script).into_bytes(),
                )
            }
            None => (200, content_type(&file), body),
        },
        Err(_) => not_found,
    }
}

/// Set when the checks have finished, so the held-open page can finish loading.
struct Done {
    finished: Mutex<bool>,
    changed: Condvar,
}

impl Done {
    fn new() -> Self {
        Done {
            finished: Mutex::new(false),
            changed: Condvar::new(),
        }
    }

    fn finish(&self) {
        *self.finished.lock().unwrap() = true;
        self.changed.notify_all();
    }

    /// Waits until `finish` is called, or gives up after `limit`.
    fn wait(&self, limit: Duration) {
        let finished = self.finished.lock().unwrap();
        let _ = self
            .changed
            .wait_timeout_while(finished, limit, |finished| !*finished);
    }
}

/// The longest the page is held open, so a broken check cannot hang a build.
const HOLD_LIMIT: Duration = Duration::from_secs(120);

fn serve_one(site: &Path, mut stream: TcpStream, done: &Done, mode: Mode) {
    // a request can arrive in several segments: read up to the blank line
    let mut buffer = [0u8; 4096];
    let mut length = 0;
    while length < buffer.len() && !buffer[..length].windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buffer[length..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => length += read,
        }
    }
    let request = String::from_utf8_lossy(&buffer[..length]);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let (status, kind, body) = match path {
        "/wait.js" => {
            done.wait(HOLD_LIMIT);
            (200, "text/javascript; charset=utf-8", Vec::new())
        }
        "/done" => {
            done.finish();
            (200, "text/plain", b"done".to_vec())
        }
        _ => route(site, path, mode),
    };
    let reason = if status == 200 { "OK" } else { "Not Found" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// Starts a test server on a free local port, with its own fresh `/done`, and returns the port.
fn start_server(site: &Path, mode: Mode) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free local port");
    let port = listener.local_addr().expect("an address").port();
    let site = site.to_path_buf();
    let done = Arc::new(Done::new());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (site, done) = (site.clone(), Arc::clone(&done));
            std::thread::spawn(move || serve_one(&site, stream, &done, mode));
        }
    });
    port
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

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The text as URL-safe base64 with no padding: `-` and `_` for `+` and `/`.
/// The art page reads it back with `atob`, which accepts the missing padding.
fn base64url(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let bits = (first << 16) | (second << 8) | third;
        // one byte makes 2 symbols, two bytes 3, three bytes 4
        for symbol in 0..chunk.len() + 1 {
            let index = (bits >> (18 - 6 * symbol)) & 63;
            out.push(char::from(BASE64_ALPHABET[index as usize]));
        }
    }
    out
}

/// The text as a JavaScript string literal, safe inside a `<script>` element.
fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// What the command line asks for.
#[derive(Debug, PartialEq)]
struct Options {
    site: PathBuf,
    require: bool,
    screenshots: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        site: PathBuf::from("target/site"),
        require: false,
        screenshots: None,
    };
    let mut rest = args.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--require" => options.require = true,
            "--screenshots" => {
                let Some(directory) = rest.next() else {
                    return Err("--screenshots needs a directory, for example --screenshots %TEMP%\\tgi_shots".to_string());
                };
                options.screenshots = Some(PathBuf::from(directory));
            }
            other if other.starts_with("--") => {
                return Err(format!(
                    "unknown option {other}; use [site-directory] [--require] [--screenshots DIR]"
                ));
            }
            other => options.site = PathBuf::from(other),
        }
    }
    Ok(options)
}

/// One browser visit: the page it checks, how the server runs for it, and the path.
struct Visit {
    name: &'static str,
    mode: Mode,
    path: String,
}

fn visits() -> Vec<Visit> {
    vec![
        Visit {
            name: "landing",
            mode: Mode::Normal,
            path: "/harness.html".to_string(),
        },
        Visit {
            name: "art",
            mode: Mode::Normal,
            path: "/art/harness.html".to_string(),
        },
        Visit {
            name: "games",
            mode: Mode::Normal,
            path: "/games/harness.html?seed=7".to_string(),
        },
        Visit {
            name: "forward",
            mode: Mode::Forward,
            path: format!("/#code={}", base64url(FORWARD_PROGRAM)),
        },
    ]
}

/// The pages photographed for people to look at, with no checks in them.
const SCREENSHOTS: [(&str, &str); 3] = [
    ("landing", "/"),
    ("art", "/art/"),
    ("games", "/games/?seed=7"),
];

/// Runs the browser on one address and returns what it printed.
fn browse(browser: &Path, url: &str, extra: &[&str]) -> std::io::Result<Output> {
    Command::new(browser)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--hide-scrollbars",
        ])
        .args(extra)
        .arg(url)
        .stdin(Stdio::null())
        .output()
}

/// Photographs each page with no checks in it. Returns false if one failed.
fn take_screenshots(browser: &Path, site: &Path, directory: &Path) -> bool {
    if let Err(error) = fs::create_dir_all(directory) {
        eprintln!("cannot create {}: {error}", directory.display());
        return false;
    }
    let mut all_taken = true;
    for (name, path) in SCREENSHOTS {
        let file = directory.join(format!("{name}.png"));
        let port = start_server(site, Mode::Normal);
        let url = format!("http://127.0.0.1:{port}{path}");
        let shot = format!("--screenshot={}", file.display());
        let ran = browse(
            browser,
            &url,
            &[
                "--window-size=1100,1400",
                "--virtual-time-budget=5000",
                shot.as_str(),
            ],
        );
        match ran {
            Ok(_) if file.is_file() => println!("screenshot: {}", file.display()),
            _ => {
                eprintln!("screenshot failed: {name} ({url})");
                all_taken = false;
            }
        }
    }
    all_taken
}

fn tail(text: &str) -> String {
    text.chars()
        .rev()
        .take(1500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    if !options.site.join("index.html").is_file() {
        eprintln!(
            "no built site in {}: run build_site first",
            options.site.display()
        );
        std::process::exit(1);
    }
    let Some(browser) = find_browser() else {
        eprintln!("no Chrome, Chromium or Edge found; skipping (set BROWSER_PATH to choose one)");
        std::process::exit(if options.require { 1 } else { 0 });
    };

    let mut problems = 0;
    if let Some(directory) = &options.screenshots {
        if !take_screenshots(&browser, &options.site, directory) {
            problems += 1;
        }
    }

    let (mut passed, mut failed) = (0, 0);
    for visit in visits() {
        let port = start_server(&options.site, visit.mode);
        let url = format!("http://127.0.0.1:{port}{}", visit.path);
        let started = Instant::now();
        let output = match browse(&browser, &url, &["--window-size=1100,2300", "--dump-dom"]) {
            Ok(output) => output,
            Err(error) => {
                eprintln!(
                    "{}: could not run {}: {error}",
                    visit.name,
                    browser.display()
                );
                problems += 1;
                continue;
            }
        };
        let dom = String::from_utf8_lossy(&output.stdout);
        let Some(report) = read_report(&dom) else {
            eprintln!(
                "{}: the browser never produced a report after {:?}",
                visit.name,
                started.elapsed()
            );
            eprintln!(
                "--- browser messages (last part):\n{}",
                tail(&String::from_utf8_lossy(&output.stderr))
            );
            eprintln!("--- page (last part):\n{}", tail(&dom));
            problems += 1;
            continue;
        };
        if report.passed.is_empty() && report.failed.is_empty() {
            println!("{}: the report is empty", visit.name);
            problems += 1;
            continue;
        }
        for name in &report.passed {
            println!("{}: ok: {name}", visit.name);
        }
        for name in &report.failed {
            println!("{}: FAIL: {name}", visit.name);
        }
        println!(
            "{}: {} passed, {} failed",
            visit.name,
            report.passed.len(),
            report.failed.len()
        );
        passed += report.passed.len();
        failed += report.failed.len();
        if !report.failed.is_empty() {
            problems += 1;
        }
    }
    println!("total: {passed} passed, {failed} failed");
    if problems > 0 {
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
        let page = with_harness(PAGE, "checks_here()");
        assert!(page.contains("app()"), "the page's own script is kept");
        let stub = page.find("__step").expect("the stub");
        let content = page.find("<div class=\"crt\">").expect("the content");
        let app = page.find("app()").expect("the app");
        let checks = page.find("checks_here()").expect("the checks");
        let wait = page.find("/wait.js").expect("the wait");
        assert!(
            stub < content && content < app && app < checks && checks < wait,
            "stub, page, checks, then wait.js"
        );
        assert!(page.trim_end().ends_with("</html>"));
    }

    #[test]
    fn a_page_without_the_expected_markers_is_left_alone_rather_than_broken() {
        assert_eq!(with_harness("<p>hello</p>", "x"), "<p>hello</p>");
    }

    #[test]
    fn the_stub_takes_over_animation_frames_and_can_step_them() {
        assert!(STUB.contains("requestAnimationFrame"));
        assert!(STUB.contains("__step"));
    }

    #[test]
    fn the_stub_records_script_errors_and_every_check_reports_them() {
        assert!(STUB.contains("__errors"));
        assert!(STUB.contains("unhandledrejection"));
        assert!(RUN.contains("no script errors"));
        for harness in [
            landing_harness(),
            art_harness(),
            games_harness(),
            forward_harness(),
        ] {
            assert!(harness.contains("no script errors"));
        }
    }

    #[test]
    fn the_art_checks_cover_the_studio_and_its_top_bar() {
        let harness = art_harness();
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
            "nav.topbar",
            "aria-current",
            "harness-report",
            "ok: ",
            "FAIL: ",
        ] {
            assert!(
                harness.contains(needle),
                "the art checks never mention {needle}"
            );
        }
    }

    #[test]
    fn the_landing_checks_cover_the_top_bar_doors_and_title() {
        let harness = landing_harness();
        for needle in [
            "nav.topbar",
            "a.door",
            "art/",
            "games/",
            "h1",
            "TERMINAL GRAPHICS",
        ] {
            assert!(
                harness.contains(needle),
                "the landing checks never mention {needle}"
            );
        }
    }

    #[test]
    fn the_games_checks_cover_the_page_and_its_keys() {
        let harness = games_harness();
        for needle in [
            "data-game",
            "data-key",
            "pointerdown",
            "tuner",
            "fireball_rate",
            "restart",
            "overlay",
            "defaultPrevented",
            "harness-report",
            "/done",
            "multiplayer",
            "\"make\"",
        ] {
            assert!(
                harness.contains(needle),
                "the games checks never mention {needle}"
            );
        }
    }

    #[test]
    fn the_forward_check_knows_the_program_it_expects() {
        let harness = forward_harness();
        assert!(harness.contains("#code="));
        assert!(harness.contains(&js_string(FORWARD_PROGRAM)));
        assert!(harness.contains("location.pathname"));
    }

    #[test]
    fn no_check_script_has_template_markers_left_in_it() {
        for harness in [
            landing_harness(),
            art_harness(),
            games_harness(),
            forward_harness(),
        ] {
            assert!(!harness.contains("{{"));
        }
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
        fs::create_dir_all(dir.join("art")).unwrap();
        fs::create_dir_all(dir.join("games")).unwrap();
        fs::write(dir.join("index.html"), PAGE).unwrap();
        fs::write(dir.join("art/index.html"), PAGE).unwrap();
        fs::write(dir.join("games/index.html"), PAGE).unwrap();
        fs::write(dir.join("art/art.wasm"), b"\0asm").unwrap();
        dir
    }

    #[test]
    fn the_server_serves_the_page_the_wasm_and_the_harness_page() {
        let dir = site_dir("serve");
        let (status, kind, body) = route(&dir, "/", Mode::Normal);
        assert_eq!((status, kind), (200, "text/html; charset=utf-8"));
        assert_eq!(body, PAGE.as_bytes());

        let (status, kind, body) = route(&dir, "/art/art.wasm", Mode::Normal);
        assert_eq!((status, kind), (200, "application/wasm"));
        assert_eq!(body, b"\0asm");

        let (status, _, body) = route(&dir, "/harness.html", Mode::Normal);
        assert_eq!(status, 200);
        assert_eq!(body, with_harness(PAGE, &landing_harness()).into_bytes());

        assert_eq!(route(&dir, "/missing.txt", Mode::Normal).0, 404);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_site_has_a_directory_index_for_each_page() {
        let dir = site_dir("index");
        assert_eq!(route(&dir, "/", Mode::Normal).0, 200);
        assert_eq!(
            route(&dir, "/art/", Mode::Normal).1,
            "text/html; charset=utf-8"
        );
        assert_eq!(
            route(&dir, "/games/?seed=7", Mode::Normal).2,
            PAGE.as_bytes()
        );
        assert_eq!(route(&dir, "/art", Mode::Normal).0, 404);
        assert_eq!(route(&dir, "/nowhere/", Mode::Normal).0, 404);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_harness_page_gets_its_own_harness() {
        let dir = site_dir("harnesses");
        let landing = with_harness(PAGE, &landing_harness()).into_bytes();
        let art = with_harness(PAGE, &art_harness()).into_bytes();
        let games = with_harness(PAGE, &games_harness()).into_bytes();
        assert_eq!(route(&dir, "/harness.html", Mode::Normal).2, landing);
        assert_eq!(route(&dir, "/art/harness.html", Mode::Normal).2, art);
        assert_eq!(
            route(&dir, "/games/harness.html?seed=7", Mode::Normal).2,
            games
        );
        assert_ne!(landing, art);
        assert_ne!(art, games);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn forward_mode_leaves_the_landing_page_alone_and_harnesses_the_studio() {
        let dir = site_dir("forward");
        assert_eq!(route(&dir, "/", Mode::Forward).2, PAGE.as_bytes());
        let studio = route(&dir, "/art/", Mode::Forward).2;
        assert_eq!(studio, with_harness(PAGE, &forward_harness()).into_bytes());
        assert_eq!(route(&dir, "/art/harness.html", Mode::Forward).0, 404);
        assert_eq!(route(&dir, "/harness.html", Mode::Forward).0, 404);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_server_ignores_query_strings_and_never_leaves_the_site_directory() {
        let dir = site_dir("safe");
        assert_eq!(route(&dir, "/?x=1", Mode::Normal).0, 200);
        assert_eq!(route(&dir, "/index.html#top", Mode::Normal).0, 200);
        for sneaky in [
            "/../Cargo.toml",
            "/..%2FCargo.toml",
            "/%2e%2e/Cargo.toml",
            "//etc/passwd",
            "/a/../../x",
            "/art/../index.html",
            "/games/%2e%2e/x",
            "/art\\index.html",
        ] {
            assert_eq!(route(&dir, sneaky, Mode::Normal).0, 404, "{sneaky}");
            assert_eq!(route(&dir, sneaky, Mode::Forward).0, 404, "{sneaky}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_page_is_held_open_until_the_checks_say_they_are_done() {
        let page = with_harness(PAGE, &landing_harness());
        assert!(page.contains("/wait.js"), "the page asks to be held");
        assert!(
            harness_script("").contains("/done"),
            "the checks say when they finish"
        );
        let wait = page.find("/wait.js").unwrap();
        let checks = page.find("harness-report").unwrap();
        assert!(checks < wait, "the checks start before the page waits");
    }

    #[test]
    fn the_server_holds_wait_js_until_done_is_requested() {
        let dir = site_dir("hold");
        let port = start_server(&dir, Mode::Normal);
        let (sender, received) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(get(port, "/wait.js"));
        });
        assert!(
            received.recv_timeout(Duration::from_millis(300)).is_err(),
            "wait.js must not answer before /done"
        );
        assert!(get(port, "/done").starts_with("HTTP/1.1 200"));
        let answer = received
            .recv_timeout(Duration::from_secs(5))
            .expect("wait.js answers once done");
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Connects to the test server, trying again for a moment if the port is busy.
    fn connect_with_retries(port: u16) -> TcpStream {
        let mut last_error = String::from("no attempt");
        for attempt in 0..5 {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => return stream,
                Err(error) => {
                    last_error = error.to_string();
                    if attempt < 4 {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
        panic!("could not connect to the test server on port {port} after 5 tries: {last_error}");
    }

    #[test]
    fn a_request_that_arrives_in_pieces_is_still_understood() {
        // TCP may deliver a request in several segments; the server must read
        // up to the blank line instead of trusting its first read
        let dir = site_dir("pieces");
        let port = start_server(&dir, Mode::Normal);
        let mut stream = connect_with_retries(port);
        stream.write_all(b"GET /art/art.wasm HT").unwrap();
        stream.flush().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        stream.write_all(b"TP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.contains("application/wasm"), "{answer}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// One plain HTTP GET against the test server, answered as text.
    fn get(port: u16, path: &str) -> String {
        let mut stream = connect_with_retries(port);
        // one write, so the request is one segment
        let request = format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer
    }

    #[test]
    fn finding_a_browser_does_not_panic() {
        let _ = find_browser();
    }

    #[test]
    fn base64url_matches_the_known_vectors_without_padding() {
        assert_eq!(base64url(""), "");
        assert_eq!(base64url("f"), "Zg");
        assert_eq!(base64url("fo"), "Zm8");
        assert_eq!(base64url("foo"), "Zm9v");
        assert_eq!(base64url("foob"), "Zm9vYg");
        assert_eq!(base64url("fooba"), "Zm9vYmE");
    }

    #[test]
    fn base64url_uses_dash_and_underscore_where_base64_uses_plus_and_slash() {
        assert_eq!(base64url("~~~"), "fn5-");
        assert_eq!(base64url("\u{7f}\u{7f}\u{7f}"), "f39_");
        assert!(!base64url(FORWARD_PROGRAM).contains(['+', '/', '=']));
    }

    #[test]
    fn a_program_is_put_in_a_script_as_a_safe_string() {
        assert_eq!(js_string("a\"b\n"), "\"a\\\"b\\n\"");
        assert_eq!(js_string("x\\y"), "\"x\\\\y\"");
        assert!(!js_string("</script>").contains("</"));
    }

    #[test]
    fn the_options_are_read_from_the_command_line() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let plain = parse_args(&args(&[])).unwrap();
        assert_eq!(plain.site, PathBuf::from("target/site"));
        assert!(!plain.require && plain.screenshots.is_none());
        let full = parse_args(&args(&["out", "--require", "--screenshots", "shots"])).unwrap();
        assert_eq!(full.site, PathBuf::from("out"));
        assert!(full.require);
        assert_eq!(full.screenshots, Some(PathBuf::from("shots")));
        assert!(parse_args(&args(&["--screenshots"])).is_err());
        assert!(parse_args(&args(&["--nonsense"])).is_err());
    }

    #[test]
    fn every_visit_is_a_page_with_its_own_server_mode() {
        let visits = visits();
        let names: Vec<&str> = visits.iter().map(|v| v.name).collect();
        assert_eq!(names, ["landing", "art", "games", "forward"]);
        assert_eq!(visits[3].mode, Mode::Forward);
        assert!(visits[3].path.starts_with("/#code="));
        assert!(visits[2].path.contains("seed=7"));
    }
}

//! Builds the project website into a directory (default `target/site`).
//!
//! ```text
//! cargo build --release --example web_art --target wasm32-unknown-unknown
//! cargo run --example build_site -- [output-directory]
//! ```
//!
//! The page is generated from Rust so the repository stays 100% Rust: the
//! colours come from `Colour`, the examples are the repository's example files,
//! and the API guide is generated from the interpreter's own tables, so it
//! cannot disagree with what the editor understands. The compiled `web_art`
//! example is copied alongside as `art.wasm`. The result is deployed to GitHub
//! Pages by `.github/workflows/pages.yml`.

#[allow(dead_code)]
#[path = "site/presets.rs"]
mod presets;
#[allow(dead_code)]
#[path = "site/script/mod.rs"]
mod script;

use std::fs;
use std::path::{Path, PathBuf};

use script::ir::{Field, Math1, Math2, PaletteKind, WorldSetting, COLOURS, CONSTANTS};
use terminal_graphics_interactive::Colour;

const SITE_URL: &str = "https://calum.github.io/terminal_graphics_interactive/";
const REPO_URL: &str = "https://github.com/calum/terminal_graphics_interactive";
const WASM_PATH: &str = "target/wasm32-unknown-unknown/release/examples/web_art.wasm";

/// The page, with `{{NAME}}` where generated parts go.
const PAGE: &str = r####"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terminal Art - make something beautiful in a few lines of Rust</title>
<meta name="description" content="Write a few lines of Rust, press run, and watch art move in a terminal. Formulas, physics and your mouse. Nothing to install.">
<link rel="canonical" href="{{SITE_URL}}">
<style>
{{CSS}}
</style>
</head>
<body>
<div class="crt">
<main>

<p class="dim">TERMINAL ART BBS v{{VERSION}} - CONNECTED 64000 BAUD</p>
<h1 aria-label="TERMINAL ART">{{TITLE}}</h1>
<p class="subtitle">a few lines of Rust | your mouse | 16 colours | nothing to install</p>
<p class="nav">
<a href="{{REPO_URL}}">[ GitHub ]</a>
<a href="https://crates.io/crates/terminal_graphics_interactive">[ crates.io ]</a>
<a href="https://docs.rs/terminal_graphics_interactive">[ docs.rs ]</a>
</p>

<pre class="prompt"><span class="c10">$</span> cargo add terminal_graphics_interactive<span class="cursor">_</span></pre>

<section id="studio" data-default="{{DEFAULT}}">
<h2>&gt; THE STUDIO</h2>
<p>Pick something to start from, change a number, press <b class="c11">RUN</b>.
Everything you see is drawn into a grid of characters, just like in a real
terminal. Your mouse works in the screen: move it, click, drag, right-click.</p>
<p class="examples" role="group" aria-label="Examples">
{{EXAMPLE_BUTTONS}}
</p>
<p id="blurb" class="blurb">&nbsp;</p>

<div class="editor">
<pre id="gutter" aria-hidden="true">1</pre>
<textarea id="code" spellcheck="false" autocapitalize="off" autocomplete="off" autocorrect="off" wrap="off" rows="16" aria-label="Rust code editor"></textarea>
</div>
<p class="actions">
<button id="run" type="button">RUN</button>
<button id="reset" type="button">RESET</button>
<button id="blank" type="button">BLANK</button>
<button id="share" type="button">SHARE</button>
<span class="dim">Ctrl+Enter runs it. Esc then Tab leaves the editor.</span>
</p>
<pre id="problems" class="problems" role="status" aria-live="polite">loading the studio ...</pre>

<div class="screen">
<canvas id="screen" tabindex="0" width="768" height="528" aria-label="Terminal showing your art. Move the mouse over it, click, drag and right-click to play with it."></canvas>
</div>
<p class="dim">Move, click, drag and right-click inside the screen. On a phone, touch and drag.</p>
{{EXAMPLE_SOURCES}}
</section>

<section>
<h2>&gt; THE GUIDE</h2>
{{GUIDE}}
</section>

<section>
<h2>&gt; THE PALETTE</h2>
<p>A terminal has 16 colours. Between two of them a number is drawn with a shade block, so a palette is smoother than it sounds.</p>
<div class="palette">{{PALETTE}}</div>
</section>

<section>
<h2>&gt; LINKS</h2>
<ul class="links">
<li><a href="{{REPO_URL}}">GitHub repository</a> - source, examples, issues</li>
<li><a href="https://crates.io/crates/terminal_graphics_interactive">crates.io/crates/terminal_graphics_interactive</a></li>
<li><a href="https://docs.rs/terminal_graphics_interactive">docs.rs/terminal_graphics_interactive</a> - API documentation</li>
</ul>
</section>

<footer>
<p class="dim">terminal_graphics_interactive v{{VERSION}} - MIT OR Apache-2.0 - <a href="{{REPO_URL}}">view the source on GitHub</a></p>
<p class="dim">NO CARRIER<span class="cursor">_</span></p>
</footer>

</main>
</div>
<script>
{{JS}}
</script>
</body>
</html>
"####;

const CSS: &str = r####"
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
.nav a { margin-right: 12px; }
.prompt { background: #000; border: 1px solid var(--c8); padding: 8px 12px; overflow-x: auto; }
.cursor { animation: blink 1s steps(1) infinite; }
@keyframes blink { 50% { opacity: 0; } }
code { color: var(--c10); }
ul { padding-left: 20px; list-style: none; }
li::before { content: "> "; color: var(--c8); margin-left: -20px; }
.examples { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0 4px; }
.examples button.active { background: var(--c11); color: #000; border-color: var(--c11); text-shadow: none; }
.blurb { color: var(--c14); min-height: 1.5em; margin: 4px 0 10px; }
.editor { display: flex; border: 2px solid var(--c8); background: #080808; }
.editor:focus-within { border-color: var(--c11); box-shadow: 0 0 18px rgba(255,255,85,.2); }
#gutter { margin: 0; padding: 10px 8px 10px 10px; color: var(--c8); text-align: right; font: 14px/1.5 "Courier New", Courier, monospace;
  overflow: hidden; user-select: none; min-width: 3.4ch; text-shadow: none; border-right: 1px solid #222; }
#code { flex: 1; min-width: 0; background: transparent; color: var(--c15); caret-color: var(--c10); border: 0; outline: 0; resize: vertical;
  padding: 10px; font: 14px/1.5 "Courier New", Courier, monospace; tab-size: 4; white-space: pre; overflow: auto; text-shadow: none; }
.actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 10px 0; }
.problems { margin: 8px 0 14px; padding: 8px 12px; border: 1px solid var(--c8); background: #050505; white-space: pre-wrap; overflow-x: auto; min-height: 2.6em; font-size: 14px; }
.problems.good { border-color: var(--c10); color: var(--c10); }
.problems.bad { border-color: var(--c9); color: var(--c9); }
.screen { border: 2px solid var(--c10); padding: 8px; background: #000; box-shadow: 0 0 24px rgba(85,255,85,.25); }
canvas { display: block; width: 100%; height: auto; image-rendering: pixelated; outline: none; cursor: crosshair; touch-action: none; }
canvas:focus { box-shadow: 0 0 0 2px var(--c11); }
button { font: inherit; background: #000; color: var(--c10); border: 1px solid var(--c10); padding: 6px 14px; cursor: pointer; }
button:hover, button:active { background: var(--c10); color: #000; text-shadow: none; }
#run { color: var(--c11); border-color: var(--c11); }
#run:hover, #run:active { background: var(--c11); }
.guide { display: grid; gap: 14px; }
.box { border: 1px solid var(--c8); background: #050505; padding: 10px 14px; text-shadow: none; }
.box h3 { margin-top: 0; }
.box p { margin: 6px 0; }
.box pre { margin: 6px 0; overflow-x: auto; color: var(--c7); font-size: 14px; }
.names code { display: inline-block; margin: 2px 8px 2px 0; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 14px; margin: 6px 0; }
dt { color: var(--c10); }
dd { margin: 0; }
.palette { display: grid; grid-template-columns: repeat(8, 1fr); gap: 4px; }
.palette div { padding: 14px 2px 4px; font-size: 11px; text-align: center; border: 1px solid var(--c8); text-shadow: none; }
{{CLASSES}}
@media (max-width: 560px) { .palette { grid-template-columns: repeat(4, 1fr); } dl { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { h1 span, .cursor { animation: none; } }
"####;

const JS: &str = r####"
(function () {
  "use strict";
  var $ = function (id) { return document.getElementById(id); };
  var canvas = $("screen"), ctx = canvas.getContext("2d");
  var editor = $("code"), gutter = $("gutter"), problems = $("problems"), blurb = $("blurb");
  var studio = $("studio");
  var CELL_W = 12, CELL_H = 22;
  var art = null, colours = [], last = 0, failed = false;
  var ran = false;             // has a program compiled and started yet?
  var current = null;          // the id of the example loaded, if any
  var held = 0;                // which mouse button is down: 0 none, 1 left, 2 right
  var lastCell = [0, 0];
  var encoder = new TextEncoder(), decoder = new TextDecoder();

  function css(rgb) { return "#" + ("000000" + rgb.toString(16)).slice(-6); }

  function say(kind, text) {
    problems.className = "problems " + kind;
    problems.textContent = text;
  }

  // ---------------------------------------------------------------- the screen

  function draw() {
    var w = art.width(), h = art.height();
    var cells = new Uint32Array(art.memory.buffer, art.frame_ptr(), w * h * 2);
    ctx.font = "bold 18px 'Courier New', monospace";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    for (var y = 0; y < h; y++) {
      for (var x = 0; x < w; x++) {
        var i = (y * w + x) * 2, ch = cells[i], pair = cells[i + 1];
        var foreground = colours[pair & 255], background = colours[(pair >> 8) & 255];
        ctx.globalAlpha = 1;
        ctx.fillStyle = background;
        ctx.fillRect(x * CELL_W, y * CELL_H, CELL_W, CELL_H);
        // the shade blocks are the foreground laid over the background
        var cover = ch === 0x2591 ? 0.25 : ch === 0x2592 ? 0.5 : ch === 0x2593 ? 0.75 : ch === 0x2588 ? 1 : 0;
        if (cover > 0) {
          ctx.globalAlpha = cover;
          ctx.fillStyle = foreground;
          ctx.fillRect(x * CELL_W, y * CELL_H, CELL_W, CELL_H);
          ctx.globalAlpha = 1;
        } else if (ch !== 32) {
          ctx.fillStyle = foreground;
          ctx.fillText(String.fromCodePoint(ch), x * CELL_W + CELL_W / 2, y * CELL_H + CELL_H / 2 + 1);
        }
      }
    }
    ctx.globalAlpha = 1;
  }

  function readError() {
    var length = art.error_len(), bytes = new Uint8Array(length);
    for (var i = 0; i < length; i++) bytes[i] = art.error_byte(i);
    return decoder.decode(bytes);
  }

  function readFailure() {
    var length = art.failure_len(), bytes = new Uint8Array(length);
    for (var i = 0; i < length; i++) bytes[i] = art.failure_byte(i);
    return decoder.decode(bytes);
  }

  function frame(now) {
    if (art) {
      art.tick(Math.min(Math.round(now - last), 250));
      last = now;
      if (!failed && art.failure_len() > 0) {
        failed = true;
        say("bad", "the program stopped: " + readFailure());
      }
      draw();
    }
    requestAnimationFrame(frame);
  }

  // ------------------------------------------------------------------- running

  /** Shows the line the compiler is unhappy about, and where, like rustc does. */
  function showError() {
    var line = art.error_line(), column = art.error_column();
    var lines = editor.value.split("\n");
    var text = "error at " + line + ":" + column + ": " + readError();
    if (line >= 1 && line <= lines.length) {
      var number = String(line), pad = new Array(number.length + 1).join(" ");
      text += "\n\n " + number + " | " + lines[line - 1] + "\n " + pad + " | " + new Array(Math.max(column, 1)).join(" ") + "^";
      var start = 0;
      for (var i = 0; i < line - 1; i++) start += lines[i].length + 1;
      editor.focus();
      editor.setSelectionRange(start, start + lines[line - 1].length);
    }
    say("bad", text + (ran ? "\n\n(the program that was running is still running)" : ""));
  }

  function run() {
    if (!art) return;
    var bytes = encoder.encode(editor.value);
    art.source_clear();
    for (var i = 0; i < bytes.length; i++) art.source_push(bytes[i]);
    var kind = art.compile();
    if (kind === 0) { showError(); return; }
    failed = false;
    ran = true;
    say("good", "compiled: running a " + (kind === 1 ? "sketch" : "world") + ". Try the mouse in the screen.");
  }

  // ----------------------------------------------------------------- the editor

  function updateGutter() {
    var count = editor.value.split("\n").length, text = "";
    for (var i = 1; i <= count; i++) text += i + "\n";
    gutter.textContent = text;
  }

  editor.addEventListener("input", updateGutter);
  editor.addEventListener("scroll", function () { gutter.scrollTop = editor.scrollTop; });
  editor.addEventListener("keydown", function (event) {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
      event.preventDefault();
      run();
    } else if (event.key === "Escape") {
      editor.blur();
    } else if (event.key === "Tab" && !event.shiftKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      var start = editor.selectionStart, end = editor.selectionEnd;
      editor.value = editor.value.slice(0, start) + "    " + editor.value.slice(end);
      editor.selectionStart = editor.selectionEnd = start + 4;
      updateGutter();
    }
  });

  function setActive(id) {
    current = id;
    var buttons = document.querySelectorAll("button[data-example]");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].className = buttons[i].dataset.example === id ? "active" : "";
    }
  }

  function loadExample(id) {
    var source = $("example-" + id);
    if (!source) return;
    editor.value = source.value;
    var button = document.querySelector("button[data-example=" + id + "]");
    blurb.textContent = button ? button.dataset.blurb : "";
    setActive(id);
    updateGutter();
    if (history.replaceState) history.replaceState(null, "", location.pathname);
    run();
  }

  function loadBlank() {
    editor.value = $("example-starter").value;
    blurb.textContent = "A fresh start. Change a number and press RUN.";
    setActive(null);
    updateGutter();
    run();
  }

  Array.prototype.forEach.call(document.querySelectorAll("button[data-example]"), function (button) {
    button.addEventListener("click", function () { loadExample(button.dataset.example); });
  });
  $("run").addEventListener("click", run);
  $("reset").addEventListener("click", function () { if (current) loadExample(current); else loadBlank(); });
  $("blank").addEventListener("click", loadBlank);

  // -------------------------------------------------------------------- sharing

  function toBase64Url(text) {
    var bytes = encoder.encode(text), binary = "";
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  function fromBase64Url(text) {
    var binary = atob(text.replace(/-/g, "+").replace(/_/g, "/")), bytes = new Uint8Array(binary.length);
    for (var i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return decoder.decode(bytes);
  }

  $("share").addEventListener("click", function () {
    location.hash = "code=" + toBase64Url(editor.value);
    var link = location.href;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(link).then(function () {
        say("good", "link copied. Anyone who opens it gets your code, running.");
      }, function () {
        say("good", "copy this link to share your art:\n" + link);
      });
    } else {
      say("good", "copy this link to share your art:\n" + link);
    }
  });

  // ------------------------------------------------------------------ the mouse

  function cellOf(event) {
    var rect = canvas.getBoundingClientRect(), w = art.width(), h = art.height();
    var x = Math.floor((event.clientX - rect.left) / rect.width * w);
    var y = Math.floor((event.clientY - rect.top) / rect.height * h);
    lastCell = [Math.max(0, Math.min(w - 1, x)), Math.max(0, Math.min(h - 1, y))];
    return lastCell;
  }

  canvas.addEventListener("mousemove", function (event) {
    if (!art) return;
    var cell = cellOf(event);
    art.mouse_event(held === 1 ? 5 : held === 2 ? 6 : 0, cell[0], cell[1]);
  });
  canvas.addEventListener("mousedown", function (event) {
    if (!art) return;
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    var cell = cellOf(event);
    if (event.button === 0) { held = 1; art.mouse_event(1, cell[0], cell[1]); }
    else if (event.button === 2) { held = 2; art.mouse_event(3, cell[0], cell[1]); }
  });
  window.addEventListener("mouseup", function () {
    if (!art || held === 0) return;
    art.mouse_event(held === 1 ? 2 : 4, lastCell[0], lastCell[1]);
    held = 0;
  });
  canvas.addEventListener("contextmenu", function (event) { event.preventDefault(); });

  function touch(event, kind) {
    if (!art) return;
    event.preventDefault();
    var point = event.touches[0] || event.changedTouches[0];
    var cell = cellOf(point);
    art.mouse_event(kind, cell[0], cell[1]);
  }
  canvas.addEventListener("touchstart", function (event) {
    if (!art) return;
    held = 1;
    touch(event, 0);
    art.mouse_event(1, lastCell[0], lastCell[1]);
  }, { passive: false });
  canvas.addEventListener("touchmove", function (event) { touch(event, 5); }, { passive: false });
  canvas.addEventListener("touchend", function (event) {
    event.preventDefault();
    if (art) art.mouse_event(2, lastCell[0], lastCell[1]);
    held = 0;
  }, { passive: false });

  // ---------------------------------------------------------------------- start

  updateGutter();
  fetch("art.wasm")
    .then(function (response) { return response.arrayBuffer(); })
    .then(function (bytes) { return WebAssembly.instantiate(bytes, {}); })
    .then(function (result) {
      art = result.instance.exports;
      for (var i = 0; i < 16; i++) colours.push(css(art.palette(i)));
      art.init((Math.random() * 4294967295) >>> 0);
      canvas.width = art.width() * CELL_W;
      canvas.height = art.height() * CELL_H;

      // a shared link brings its own code; otherwise start with the first example
      var shared = null;
      if (location.hash.indexOf("#code=") === 0) {
        try { shared = fromBase64Url(location.hash.slice(6)); } catch (error) { shared = null; }
      }
      if (shared !== null) {
        editor.value = shared;
        blurb.textContent = "Someone shared this with you. Press RUN after you change it.";
        updateGutter();
        run();
      } else {
        loadExample(studio.dataset.default);
      }
      requestAnimationFrame(function (now) { last = now; frame(now); });
    })
    .catch(function (error) {
      say("bad", "could not start the studio: " + error);
    });
})();
"####;

/// Makes text safe to put in a page or an attribute: it is shown, never run.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The title with every letter in its own span, so each can cycle through the
/// bright colours.
fn rainbow_title(text: &str) -> String {
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

fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn css_variables() -> String {
    Colour::ALL
        .iter()
        .enumerate()
        .map(|(i, colour)| format!("--c{i}:{};", hex(colour.rgb())))
        .collect()
}

fn colour_classes() -> String {
    (0..Colour::ALL.len())
        .map(|i| format!(".c{i}{{color:var(--c{i});}}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cycles `color` through the 7 bright colours (the last 7 in `Colour::ALL`).
fn rainbow_keyframes() -> String {
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

fn palette_swatches() -> String {
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

/// One button per example.
fn example_buttons() -> String {
    presets::PRESETS
        .iter()
        .map(|preset| {
            format!(
                "<button type=\"button\" data-example=\"{}\" data-blurb=\"{}\">{}</button>",
                preset.id,
                escape(preset.blurb),
                escape(preset.title)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The code of every example, in hidden text areas the script copies from
/// (a text area decodes the escapes back into the original code).
fn example_sources() -> String {
    let mut out: Vec<String> = presets::PRESETS
        .iter()
        .map(|preset| {
            format!(
                "<textarea id=\"example-{}\" hidden readonly>{}</textarea>",
                preset.id,
                escape(preset.source)
            )
        })
        .collect();
    out.push(format!(
        "<textarea id=\"example-starter\" hidden readonly>{}</textarea>",
        escape(presets::STARTER)
    ));
    out.join("\n")
}

fn names<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    names
        .into_iter()
        .map(|name| format!("<code>{}</code>", escape(name)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn field_meaning(name: &str) -> &'static str {
    match name {
        "x" => "which column this cell is in",
        "y" => "which row, stretched so one step looks as long as one step of x",
        "t" => "seconds since you pressed RUN",
        "mouse_x" => "the column the mouse is over",
        "mouse_y" => "the row the mouse is over, stretched like y",
        "mouse_down" => "true while the left button is held",
        "width" => "how wide the screen is",
        "height" => "how tall the screen is, stretched like y",
        _ => "",
    }
}

fn setting_meaning(name: &str) -> &'static str {
    match name {
        "balls" => "how many balls to start with (a whole number, up to 64)",
        "size" => "how big each ball is, as a radius: try 3.5",
        "gravity" => "a pull downwards each frame: try 0.02, or a negative number to float up",
        "mouse_pulls" => "the mouse pulls the balls in; hold the right button to push them away",
        "trails" => "balls leave glowing tails",
        "palette" => "the colours the balls are drawn in",
        "run" => "end the chain with this",
        _ => "",
    }
}

/// The API guide, built from the interpreter's own tables so that it lists
/// exactly what the editor understands.
fn guide() -> String {
    let fields: String = Field::ALL
        .iter()
        .map(|(name, _, _)| {
            format!(
                "<dt>cell.{name}</dt><dd>{}</dd>",
                escape(field_meaning(name))
            )
        })
        .collect();
    let settings: String = WorldSetting::ALL
        .iter()
        .map(|(name, _, _)| {
            let call = if *name == "run" {
                "run()".to_string()
            } else {
                format!("{name}(...)")
            };
            let call = if matches!(*name, "mouse_pulls" | "trails") {
                format!("{name}()")
            } else {
                call
            };
            format!(
                "<dt>.{}</dt><dd>{}</dd>",
                escape(&call),
                escape(setting_meaning(name))
            )
        })
        .collect();
    let colour_names = names(COLOURS.iter().map(|(n, _)| *n));
    let palettes = names(PaletteKind::ALL.iter().map(|(n, _)| *n));
    let one = names(Math1::ALL.iter().map(|(n, _)| *n));
    let two = names(Math2::ALL.iter().map(|(n, _)| *n));
    let constants = names(CONSTANTS.iter().map(|(n, _)| *n));

    format!(
        r#"<div class="guide">

<div class="box">
<h3>1. THE SHAPE OF A PROGRAM</h3>
<p>Every program is a <code>main</code> that ends by showing one thing: a <b>sketch</b> (a formula for every cell) or a <b>world</b> (balls that move).</p>
<pre>{shape}</pre>
</div>

<div class="box">
<h3>2. A SKETCH: ONE FORMULA FOR EVERY CELL</h3>
<p>The formula runs for every cell, every frame. It gets a <code>cell</code> and returns a number from -1.0 to 1.0 (what <code>sin</code> and <code>cos</code> give), which the palette turns into a colour.</p>
<dl>{fields}<dt>cell.dist_to_mouse()</dt><dd>how far this cell is from the mouse</dd></dl>
<p>Use <code>sketch(|cell| ...)</code> for the ocean palette, or <code>sketch_with(palette, |cell| ...)</code> to choose.</p>
</div>

<div class="box">
<h3>3. PALETTES AND COLOURS</h3>
<p>Presets: <span class="names">{palettes}</span> used as <code>Palette::fire()</code>.</p>
<p>Or make your own, dark to bright: <code>Palette::new(&amp;[Colour::Black, Colour::Red, Colour::BrightYellow])</code>.</p>
<p>Colours: <span class="names">{colour_names}</span></p>
</div>

<div class="box">
<h3>4. MATHS</h3>
<p>On any decimal (<code>f32</code>), like <code>cell.x.sin()</code>:</p>
<p class="names">{one}</p>
<p>With one argument, like <code>a.min(b)</code> or <code>a.powf(2.0)</code>:</p>
<p class="names">{two}</p>
<p>Also <code>a.powi(3)</code> (a whole-number power), <code>a.clamp(low, high)</code> and <code>a.mul_add(b, c)</code>.</p>
<p>Constants: <span class="names">{constants}</span> (as <code>PI</code>, or <code>std::f32::consts::PI</code>).</p>
</div>

<div class="box">
<h3>5. A WORLD: BALLS THAT MOVE</h3>
<p>Chain settings and finish with <code>.run()</code>. Balls bounce off the walls and each other, and a click adds one.</p>
<pre>{world}</pre>
<dl>{settings}</dl>
</div>

<div class="box">
<h3>6. THE RUST RULES (THIS EDITOR IS AS STRICT AS THE REAL COMPILER)</h3>
<p>Decimals need a dot: <code>cell.x * 2.0</code> works, <code>cell.x * 2</code> does not. Whole numbers are for counts and loops.</p>
<pre>{rules}</pre>
<p>You can use <code>let</code>, <code>let mut</code>, <code>if</code> / <code>else</code>, <code>for i in 0..5</code>, and your own functions like <code>fn ring(d: f32) -&gt; f32</code>. Press RUN and the compiler tells you the line and column of any mistake.</p>
</div>

<div class="box">
<h3>7. TAKE IT HOME</h3>
<p>This is real Rust. Anything that runs here also compiles in your own project:</p>
<pre>$ cargo add terminal_graphics_interactive
$ cargo run</pre>
<p>Paste your code into <code>src/main.rs</code> and it runs in your own terminal (Windows PowerShell, Linux, macOS), with your real mouse. Use SHARE to send your art to a friend as a link.</p>
</div>

</div>"#,
        shape = escape(SHAPE),
        world = escape(WORLD),
        rules = escape(RULES),
    )
}

const SHAPE: &str = "use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    sketch(|cell| (cell.x * 0.2 + cell.t).sin())
}";

const WORLD: &str = "World::new()
    .balls(5)
    .size(3.5)
    .gravity(0.02)
    .mouse_pulls()
    .trails()
    .run()";

const RULES: &str = "let speed = 2.0;              // a decimal
let mut total = 0.0;
for i in 0..5 {               // i is a whole number
    total += (i as f32) * 0.1;  // turn it into a decimal with `as f32`
}
if cell.mouse_down { total } else { -total }";

fn build_page() -> String {
    let css = CSS
        .replace("{{VARS}}", &css_variables())
        .replace("{{KEYFRAMES}}", &rainbow_keyframes())
        .replace("{{CLASSES}}", &colour_classes());
    PAGE.replace("{{CSS}}", &css)
        .replace("{{JS}}", JS)
        .replace("{{TITLE}}", &rainbow_title("TERMINAL ART"))
        .replace("{{EXAMPLE_BUTTONS}}", &example_buttons())
        .replace("{{EXAMPLE_SOURCES}}", &example_sources())
        .replace("{{GUIDE}}", &guide())
        .replace("{{PALETTE}}", &palette_swatches())
        .replace("{{DEFAULT}}", presets::PRESETS[0].id)
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
        .replace("{{SITE_URL}}", SITE_URL)
        .replace("{{REPO_URL}}", REPO_URL)
}

/// Writes the site into `out`. Returns false if the compiled program was missing.
fn build_site(out: &Path, wasm: &Path) -> std::io::Result<bool> {
    fs::create_dir_all(out)?;
    fs::write(out.join("index.html"), build_page())?;
    fs::write(out.join(".nojekyll"), "")?;
    if wasm.exists() {
        fs::copy(wasm, out.join("art.wasm"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/site"), PathBuf::from);
    let found = build_site(&out, Path::new(WASM_PATH))?;
    println!("site written to {}", out.display());
    if !found {
        eprintln!(
            "warning: {WASM_PATH} not found, the page will not run. Build it with:\n  \
             cargo build --release --example web_art --target wasm32-unknown-unknown"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEB_ART: &str = include_str!("web_art.rs");

    fn page() -> String {
        build_page()
    }

    // ----------------------------------------------------------- the basics

    #[test]
    fn every_placeholder_is_filled_in() {
        let page = page();
        assert!(!page.contains("{{"), "a placeholder is left in the page");
        assert!(page.len() > 10_000, "the page is only {} bytes", page.len());
    }

    #[test]
    fn it_is_a_complete_page_that_search_engines_and_phones_understand() {
        let page = page();
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.contains("<html lang=\"en\">"));
        assert!(page.contains("<meta name=\"viewport\""));
        assert!(page.contains("<meta name=\"description\""));
        assert!(page.contains(&format!("<link rel=\"canonical\" href=\"{SITE_URL}\">")));
        assert!(page.contains("<title>"));
        assert!(page.trim_end().ends_with("</html>"));
    }

    #[test]
    fn it_links_to_the_repo_the_crate_and_the_docs() {
        let page = page();
        assert!(page.contains(&format!("href=\"{REPO_URL}\"")));
        assert!(page.contains("https://crates.io/crates/terminal_graphics_interactive"));
        assert!(page.contains("https://docs.rs/terminal_graphics_interactive"));
        assert!(page.contains("cargo add terminal_graphics_interactive"));
    }

    #[test]
    fn it_is_for_people_and_does_not_talk_to_ai_agents() {
        let page = page().to_lowercase();
        assert!(
            !page.contains("llms"),
            "llms.txt belongs in the README, not here"
        );
        assert!(!page.contains("ai agent") && !page.contains("ai coding"));
    }

    // -------------------------------------------------------------- the studio

    #[test]
    fn the_studio_has_an_editor_a_run_button_a_screen_and_a_place_for_errors() {
        let page = page();
        for needle in [
            "<textarea id=\"code\"",
            "<button id=\"run\"",
            "<canvas id=\"screen\"",
            "id=\"problems\"",
            "id=\"gutter\"",
            "id=\"share\"",
            "id=\"reset\"",
        ] {
            assert!(page.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn the_editor_is_not_fiddly_on_a_phone_or_with_a_spell_checker() {
        let page = page();
        assert!(page.contains("spellcheck=\"false\""));
        assert!(page.contains("autocapitalize=\"off\""));
        assert!(page.contains("autocomplete=\"off\""));
        assert!(page.contains("aria-label=\"Rust code editor\""));
        assert!(page.contains("aria-label=\"Terminal showing your art"));
    }

    #[test]
    fn there_is_a_button_for_every_example_and_its_code_is_in_the_page() {
        let page = page();
        for preset in presets::PRESETS {
            assert!(
                page.contains(&format!("data-example=\"{}\"", preset.id)),
                "no button for {}",
                preset.id
            );
            assert!(
                page.contains(&format!("id=\"example-{}\"", preset.id)),
                "no source for {}",
                preset.id
            );
            assert!(
                page.contains(&escape(preset.blurb)),
                "no blurb for {}",
                preset.id
            );
            // the code is in the page, escaped so that it is text and not markup
            let first_code_line = preset
                .source
                .lines()
                .find(|l| l.contains("sketch") || l.contains("World::new"))
                .unwrap();
            assert!(page.contains(&escape(first_code_line)), "{}", preset.id);
        }
    }

    #[test]
    fn the_page_starts_by_running_the_first_example() {
        let page = page();
        assert!(page.contains("data-default=\"plasma\""));
        assert_eq!(presets::PRESETS[0].id, "plasma");
    }

    #[test]
    fn the_editor_says_how_to_run_with_the_keyboard_and_how_to_use_the_mouse() {
        let page = page();
        assert!(page.contains("Ctrl+Enter"));
        assert!(page.contains("right-click"));
    }

    // ------------------------------------------------------------ the guide

    #[test]
    fn the_guide_lists_everything_the_editor_understands_from_its_own_tables() {
        let page = page();
        let mut names: Vec<&str> = Vec::new();
        names.extend(Field::ALL.iter().map(|(n, _, _)| *n));
        names.extend(Math1::ALL.iter().map(|(n, _)| *n));
        names.extend(Math2::ALL.iter().map(|(n, _)| *n));
        names.extend(PaletteKind::ALL.iter().map(|(n, _)| *n));
        names.extend(WorldSetting::ALL.iter().map(|(n, _, _)| *n));
        names.extend(COLOURS.iter().map(|(n, _)| *n));
        names.extend(CONSTANTS.iter().map(|(n, _)| *n));
        names.extend([
            "powi",
            "clamp",
            "mul_add",
            "dist_to_mouse",
            "sketch_with",
            "Palette::new",
        ]);
        for name in names {
            assert!(page.contains(name), "the guide does not mention `{name}`");
        }
    }

    #[test]
    fn the_guide_explains_the_rules_that_trip_people_up() {
        let page = page();
        assert!(page.contains("-1 to 1") || page.contains("-1.0 to 1.0"));
        assert!(page.contains("2.0"), "decimals need a dot");
        assert!(page.contains("stretched"), "why y is stretched");
        assert!(page.contains("for i in 0.."));
        assert!(page.contains("as f32"));
    }

    #[test]
    fn the_guide_says_it_is_real_rust_you_can_take_home() {
        let page = page();
        assert!(page.contains("cargo add terminal_graphics_interactive"));
        assert!(
            page.contains("compiles"),
            "the code works in a real project too"
        );
    }

    // ------------------------------------------------------------- the style

    #[test]
    fn it_has_the_retro_look_with_all_sixteen_colours() {
        let page = page();
        assert_eq!(css_variables().matches("--c").count(), 16);
        for colour in Colour::ALL {
            assert!(
                page.contains(&format!(">{}</div>", colour.name())),
                "{}",
                colour.name()
            );
        }
        assert!(page.contains("BBS"));
        assert!(page.contains("NO CARRIER"));
        assert!(page.contains("repeating-linear-gradient"), "CRT scanlines");
    }

    #[test]
    fn the_title_is_rainbow_lettered_and_still_readable_by_screen_readers() {
        let page = page();
        assert!(page.contains("<h1 aria-label=\"TERMINAL ART\">"));
        assert!(page.contains("animation-delay"));
    }

    #[test]
    fn animation_can_be_turned_off_for_people_who_ask_for_less_motion() {
        assert!(page().contains("prefers-reduced-motion"));
    }

    // -------------------------------------------------------- the javascript

    /// The `art.something(` calls in the page's script.
    fn exports_the_script_calls() -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = JS;
        while let Some(at) = rest.find("art.") {
            let after = &rest[at + 4..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if after[name.len()..].starts_with('(') && !names.contains(&name) {
                names.push(name);
            }
            rest = after;
        }
        names
    }

    #[test]
    fn every_function_the_script_calls_is_really_exported_by_the_wasm_program() {
        let called = exports_the_script_calls();
        assert!(called.len() >= 12, "only found {called:?}");
        for name in called {
            assert!(
                WEB_ART.contains(&format!("pub extern \"C\" fn {name}(")),
                "the page calls art.{name}() but web_art.rs does not export it"
            );
        }
    }

    #[test]
    fn the_script_calls_all_the_exports_it_needs() {
        let called = exports_the_script_calls();
        for needed in [
            "init",
            "source_clear",
            "source_push",
            "compile",
            "tick",
            "mouse_event",
            "frame_ptr",
            "width",
            "height",
            "palette",
            "error_line",
            "error_column",
            "error_len",
            "error_byte",
            "failure_len",
            "failure_byte",
        ] {
            assert!(
                called.contains(&needed.to_string()),
                "the script never calls art.{needed}()"
            );
        }
    }

    #[test]
    fn the_page_loads_the_wasm_and_handles_the_mouse_the_keyboard_and_sharing() {
        let page = page();
        assert!(page.contains("fetch(\"art.wasm\")"));
        for needle in [
            "mousemove",
            "mousedown",
            "mouseup",
            "contextmenu",
            "touchstart",
            "touchend",
            "keydown",
            "location.hash",
            "navigator.clipboard",
            "requestAnimationFrame",
        ] {
            assert!(page.contains(needle), "the script does not use {needle}");
        }
    }

    #[test]
    fn the_script_does_not_use_template_markers_or_unsafe_html() {
        assert!(
            !JS.contains("{{"),
            "the script must not contain template markers"
        );
        assert!(
            !JS.contains("innerHTML"),
            "build error text with textContent"
        );
        assert!(!JS.contains("eval("));
    }

    // --------------------------------------------------------------- escaping

    #[test]
    fn code_is_escaped_so_it_cannot_become_markup_or_end_a_textarea() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
        assert_eq!(
            escape("</textarea><script>"),
            "&lt;/textarea&gt;&lt;script&gt;"
        );
        let page = page();
        assert!(
            !page.contains("-> std::io::Result"),
            "unescaped code in the page"
        );
        assert!(page.contains("-&gt; std::io::Result"));
    }

    // ----------------------------------------------------------- writing it out

    #[test]
    fn the_site_is_written_with_and_without_the_wasm_program() {
        let dir = std::env::temp_dir().join(format!("tgi_site_{}", std::process::id()));
        let wasm = dir.join("fake.wasm");
        fs::create_dir_all(&dir).unwrap();

        let out = dir.join("out");
        assert!(!build_site(&out, &wasm).unwrap());
        assert!(out.join("index.html").exists());
        assert!(out.join(".nojekyll").exists());
        assert!(!out.join("art.wasm").exists());

        fs::write(&wasm, b"\0asm").unwrap();
        assert!(build_site(&out, &wasm).unwrap());
        assert_eq!(fs::read(out.join("art.wasm")).unwrap(), b"\0asm");
        assert_eq!(fs::read_to_string(out.join("index.html")).unwrap(), page());

        fs::remove_dir_all(&dir).unwrap();
    }
}

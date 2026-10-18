//! The games page of the website (`/games/`): real games from `examples/`,
//! compiled to WebAssembly (`games.wasm`, built from `examples/web_games.rs`)
//! and played in the browser, with a tuner for their numbers and their source.

use super::shell::{document, escape, rainbow_title, Document, Page, REPO_URL, SITE_URL};

/// The body of the games page, with `{{NAME}}` where generated parts go.
const BODY: &str = r####"<h1 aria-label="TERMINAL GAMES">{{TITLE}}</h1>
<p class="subtitle">real games, written in Rust | played in your browser | nothing to install</p>
<p>These are the real games from this repository, compiled to WebAssembly and played right here. The pixel art is made of half-block characters, exactly as it is drawn in a real terminal.</p>
<p id="status" role="status" aria-live="polite">loading the games ...</p>

<div class="columns">
<div>
<section id="play" data-default="glow">
<h2>&gt; THE GAMES</h2>
<p class="examples" role="group" aria-label="Games">
{{GAME_BUTTONS}}
</p>
<p id="blurb" class="blurb">&nbsp;</p>
<p id="keys" class="dim">&nbsp;</p>

<div class="screen">
<canvas id="screen" tabindex="0" width="800" height="600" aria-label="The game screen. Click it, then play with the keys, or with the buttons below on a phone."></canvas>
<div id="overlay" class="overlay">click to play</div>
</div>
<p class="actions">
<button id="restart" type="button">RESTART</button>
<span class="dim">Keys work while the screen has focus, so click it first.</span>
</p>
<div id="touch" class="touch">
<button type="button" class="key" data-key="Left" aria-label="left">&#9664;</button>
<button type="button" class="key" data-key="Up" aria-label="up">&#9650;</button>
<button type="button" class="key" data-key="Right" aria-label="right">&#9654;</button>
<button type="button" class="key" data-key="Down" aria-label="down">&#9660;</button>
<button type="button" class="key" data-key="Enter">ENTER</button>
<button type="button" class="key" data-key="Pause">PAUSE</button>
</div>
</section>

<section id="tuner">
<h2>&gt; THE TUNER</h2>
<p>Drag a slider and the running game changes. Games choose which numbers to expose: see <code>Tuning</code> in the crate docs.</p>
<div id="tuner-sliders"></div>
</section>
</div>

<section id="source-section">
<h2>&gt; THE SOURCE</h2>
{{SOURCE_LINES}}
<pre id="source" tabindex="0" class="source"></pre>
{{SOURCE_TEXTAREAS}}
</section>
</div>

<section id="make">
<h2>&gt; MAKE YOUR OWN</h2>
<p>Copy a game into <code>src/main.rs</code> and run it in your own terminal, with your own keyboard and mouse. A game is plain Rust: a <code>Game</code> trait with <code>update</code> and <code>draw</code>. To test one without a terminal, use <code>Headless</code>, which plays it for you.</p>
<pre class="prompt">$ cargo add terminal_graphics_interactive
$ cargo run</pre>
<pre class="code">{{MAKE_EXAMPLE}}</pre>
</section>

<section id="multiplayer">
<h2>&gt; MULTIPLAYER</h2>
<p>Multiplayer works in the terminal, and is not yet in the browser. <code>sail_together</code> is <code>sail</code> for two to four players: everyone sails the same sea in a boat of their own, and only the key presses cross the network (lockstep over TCP), so every machine runs the whole game. One player hosts and the others join:</p>
<pre class="code">$ cargo run --example sail_together -- host 2
$ cargo run --example sail_together -- join HOST:7777</pre>
<p>It has been played between two Windows PowerShell windows on one laptop. It has not yet been tried across a real network, or in macOS and Linux terminals. The host has no password or encryption, so play on a network you trust, such as your own LAN. Playing in the browser needs a WebSocket transport, which is planned and not built. In your own game, put the rules in a <code>Sim</code> and <code>Lockstep</code> shares the inputs.</p>
</section>

<section>
<h2>&gt; LINKS</h2>
<ul class="links">
<li><a href="{{REPO_URL}}">GitHub repository</a> - source, examples, issues</li>
<li><a href="https://crates.io/crates/terminal_graphics_interactive">crates.io/crates/terminal_graphics_interactive</a></li>
<li><a href="https://docs.rs/terminal_graphics_interactive">docs.rs/terminal_graphics_interactive</a> - API documentation</li>
</ul>
</section>
"####;

/// The rules only the games page needs: two columns on wide screens, the
/// screen with its overlay, the touch buttons and the source.
const GAMES_CSS: &str = r####"
main { max-width: 1280px; }
.columns { display: grid; gap: 0 32px; }
@media (min-width: 1000px) { .columns { grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr); align-items: start; } }
.examples { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0 4px; }
.examples button.active { background: var(--c11); color: #000; border-color: var(--c11); text-shadow: none; }
.blurb { color: var(--c14); min-height: 1.5em; margin: 4px 0 2px; }
.screen { position: relative; border: 2px solid var(--c10); padding: 8px; background: #000; box-shadow: 0 0 24px rgba(85,255,85,.25); margin-top: 12px; }
canvas { display: block; width: 100%; height: auto; image-rendering: pixelated; touch-action: none; outline: none; cursor: crosshair; }
canvas:focus { box-shadow: 0 0 0 2px var(--c11); }
.overlay { position: absolute; inset: 8px; display: flex; align-items: center; justify-content: center; padding: 12px; text-align: center;
  background: rgba(0,0,0,.6); color: var(--c14); font-size: clamp(14px, 3vw, 22px); pointer-events: none; }
.overlay[hidden] { display: none; }
.actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 10px 0; }
#restart { color: var(--c11); border-color: var(--c11); }
#restart:hover, #restart:active { background: var(--c11); }
.touch { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; max-width: 420px; margin: 14px 0; touch-action: manipulation; user-select: none; -webkit-user-select: none; }
.key { min-height: 56px; font-size: 20px; touch-action: manipulation; user-select: none; -webkit-user-select: none; }
#tuner-sliders label { display: grid; grid-template-columns: 1fr 3.5em; gap: 2px 8px; margin: 12px 0; }
#tuner-sliders label span { grid-column: 1 / -1; color: var(--c14); }
#tuner-sliders input { width: 100%; accent-color: var(--c10); }
#tuner-sliders output { text-align: right; }
.source { max-height: 70vh; overflow: auto; background: #050505; border: 1px solid var(--c8); padding: 10px; margin: 10px 0;
  font: 13px/1.45 "Courier New", Courier, monospace; white-space: pre; tab-size: 4; color: var(--c7); }
.code { overflow-x: auto; background: #050505; border: 1px solid var(--c8); padding: 10px; font: 13px/1.45 "Courier New", Courier, monospace; }
"####;

/// One game on the page. This table is the only place the gallery is written.
struct Game {
    id: &'static str,
    index: u32,
    title: &'static str,
    blurb: &'static str,
    keys: &'static str,
    /// The game's file, as the repository has it.
    source: &'static str,
}

const GAMES: [Game; 3] = [
    Game {
        id: "glow",
        index: 0,
        title: "GLOW",
        blurb: "A small light drifts through a living plasma sky. Touch a glowing mote and it bursts, and a ring of light rolls out across the sky. You have 45 seconds.",
        keys: "arrows or WASD to steer, hold the mouse to be pulled, Enter starts, p pauses",
        source: include_str!("../glow.rs"),
    },
    Game {
        id: "sail",
        index: 1,
        title: "SAIL",
        blurb: "A sailboat on the open sea, under a sky that turns from dawn to night. Steer it and dodge the fireballs that fall toward it.",
        keys: "A or D (or the arrows) steer, Enter starts, p pauses",
        source: include_str!("../sail.rs"),
    },
    Game {
        id: "quickgame",
        index: 2,
        title: "QUICKGAME",
        blurb: "Steer a @ and collect the * stars before the 20 seconds run out. The smallest complete game in the repository.",
        keys: "arrows or WASD to steer, Enter starts",
        source: include_str!("../quickgame.rs"),
    },
];

/// The page, at `/games/`.
pub fn page() -> String {
    let body = BODY
        .replace("{{TITLE}}", &rainbow_title("TERMINAL GAMES"))
        .replace("{{GAME_BUTTONS}}", &game_buttons())
        .replace("{{SOURCE_LINES}}", &source_lines())
        .replace("{{SOURCE_TEXTAREAS}}", &source_textareas())
        .replace("{{MAKE_EXAMPLE}}", &escape(llms_game_example()))
        .replace("{{REPO_URL}}", REPO_URL);
    document(&Document {
        page: Page::Games,
        title: "Terminal Games - real games written in Rust, played in your browser",
        description: "Real terminal games built with terminal_graphics_interactive, compiled to WebAssembly. Play them here, change their numbers while they run, and read their source.",
        canonical: &format!("{SITE_URL}games/"),
        head_script: "",
        css: GAMES_CSS,
        body: &body,
        script: JS,
    })
}

/// The first ```rust block of the Games section of llms.txt: the complete
/// two-scene game that the page shows under MAKE YOUR OWN.
pub fn llms_game_example() -> &'static str {
    const LLMS: &str = include_str!("../../llms.txt");
    const FENCE: &str = "```rust\n";
    let games = LLMS.split("\n## Games\n").nth(1).unwrap_or("");
    let games = games.split("\n## ").next().unwrap_or("");
    let code = match games.find(FENCE) {
        Some(start) => &games[start + FENCE.len()..],
        None => return "",
    };
    match code.find("\n```") {
        Some(end) => &code[..end],
        None => "",
    }
}

/// One button per game, carrying the blurb and the keys the script shows.
fn game_buttons() -> String {
    GAMES
        .iter()
        .map(|game| {
            format!(
                "<button type=\"button\" data-game=\"{}\" data-index=\"{}\" data-blurb=\"{}\" data-keys=\"{}\">{}</button>",
                game.id,
                game.index,
                escape(game.blurb),
                escape(game.keys),
                game.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A line per game saying where its file is. Only the first (the default) shows.
fn source_lines() -> String {
    GAMES
        .iter()
        .enumerate()
        .map(|(i, game)| {
            let hidden = if i == 0 { "" } else { " hidden" };
            format!(
                "<p class=\"dim\" data-source-line=\"{id}\"{hidden}>This is <a href=\"{REPO_URL}/blob/master/examples/{id}.rs\">examples/{id}.rs</a> in the repository. Run it with <code>cargo run --example {id}</code>.</p>",
                id = game.id,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each game's file, in a hidden text area the script copies from (a text area
/// decodes the escapes back into the original text).
fn source_textareas() -> String {
    GAMES
        .iter()
        .map(|game| {
            format!(
                "<textarea id=\"source-{}\" hidden readonly>{}</textarea>",
                game.id,
                escape(game.source)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The script. It is ES5 in the same style as the art page: the wasm is only
/// ever called through the exports listed in the tests.
const JS: &str = r####"
(function () {
  "use strict";
  var $ = function (id) { return document.getElementById(id); };
  var canvas = $("screen"), ctx = canvas.getContext("2d");
  var statusLine = $("status"), blurb = $("blurb"), keysLine = $("keys"), overlay = $("overlay");
  var sliders = $("tuner-sliders"), sourceBox = $("source");
  var buttons = document.querySelectorAll("button[data-game]");
  var CELL_W = 10, CELL_H = 20;
  var NAMED_BASE = 0x110000;
  var NAMED = { Enter: 0, Escape: 1, ArrowLeft: 2, ArrowRight: 3, ArrowUp: 4, ArrowDown: 5, Tab: 6, Backspace: 7 };
  var TOUCH_KEYS = { Left: "ArrowLeft", Right: "ArrowRight", Up: "ArrowUp", Down: "ArrowDown", Enter: "Enter", Pause: "p" };
  var decoder = new TextDecoder();
  var games = null, selected = null, colours = [], cols = 80, rows = 30, last = 0;
  var seed = 0, overlayText = null;
  var held = {};          // keyboard keys the game has been told are down
  var touchHeld = {};     // touch buttons whose pointer is down
  var mouseButton = 0;    // 0 none, 1 left, 2 right
  var lastCell = [0, 0];

  var match = /[?&]seed=(\d+)/.exec(location.search);
  seed = match ? Number(match[1]) >>> 0 : Math.floor(Math.random() * 4294967296);

  function css(rgb) { return "#" + ("000000" + rgb.toString(16)).slice(-6); }

  /** The contract code for a key, or -1 when the games do not use it. */
  function keyCode(key) {
    if (Object.prototype.hasOwnProperty.call(NAMED, key)) return NAMED_BASE + NAMED[key];
    if (key === " ") return 32;
    if (key.length === 1) return key.toLowerCase().charCodeAt(0);
    return -1;
  }

  function press(heldKeys, code) {
    if (!games || heldKeys[code]) return;
    heldKeys[code] = true;
    games.key_event(1, code);
  }

  function release(heldKeys, code) {
    if (!games || !heldKeys[code]) return;
    delete heldKeys[code];
    games.key_event(0, code);
  }

  function readText(length, byteAt) {
    var bytes = new Uint8Array(length);
    for (var j = 0; j < length; j++) bytes[j] = byteAt(j);
    return decoder.decode(bytes);
  }

  // ---------------------------------------------------------------- the screen

  function draw() {
    var cells = new Uint32Array(games.memory.buffer, games.frame_ptr(), cols * rows * 2);
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.font = "bold 18px 'Courier New', monospace";
    for (var y = 0; y < rows; y++) {
      for (var x = 0; x < cols; x++) {
        var i = (y * cols + x) * 2, ch = cells[i], pair = cells[i + 1];
        var foreground = colours[pair & 255], background = colours[(pair >> 8) & 255];
        var left = x * CELL_W, top = y * CELL_H;
        ctx.globalAlpha = 1;
        ctx.fillStyle = background;
        ctx.fillRect(left, top, CELL_W, CELL_H);
        ctx.fillStyle = foreground;
        if (ch === 0x2580) {                     // upper half block
          ctx.fillRect(left, top, CELL_W, CELL_H / 2);
        } else if (ch === 0x2584) {              // lower half block
          ctx.fillRect(left, top + CELL_H / 2, CELL_W, CELL_H / 2);
        } else if (ch === 0x2588) {              // full block
          ctx.fillRect(left, top, CELL_W, CELL_H);
        } else if (ch === 0x2591 || ch === 0x2592 || ch === 0x2593) {  // light, medium, dark shade
          ctx.globalAlpha = ch === 0x2591 ? 0.25 : ch === 0x2592 ? 0.5 : 0.75;
          ctx.fillRect(left, top, CELL_W, CELL_H);
          ctx.globalAlpha = 1;
        } else if (ch !== 32 && ch !== 0) {
          ctx.fillText(String.fromCodePoint(ch), left + CELL_W / 2, top + CELL_H / 2 + 1);
        }
      }
    }
    ctx.globalAlpha = 1;
  }

  function updateOverlay() {
    var text = "";
    if (games && games.finished() === 1) text = "the game ended - press RESTART";
    else if (document.activeElement !== canvas) text = "click to play";
    if (text === overlayText) return;
    overlayText = text;
    overlay.textContent = text;
    overlay.hidden = text === "";
  }

  function frame(now) {
    if (games) {
      games.tick(Math.min(Math.round(now - last), 250));
      last = now;
      draw();
      updateOverlay();
    }
    requestAnimationFrame(frame);
  }

  // ------------------------------------------------------------ the keyboard

  canvas.addEventListener("keydown", function (event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    var code = keyCode(event.key);
    if (code < 0) return;
    event.preventDefault();
    if (!event.repeat) press(held, code);
  });

  canvas.addEventListener("keyup", function (event) {
    var code = keyCode(event.key);
    if (code < 0 || !held[code]) return;
    event.preventDefault();
    release(held, code);
  });

  canvas.addEventListener("blur", function () {
    Object.keys(held).forEach(function (code) { release(held, Number(code)); });
    updateOverlay();
  });

  canvas.addEventListener("focus", updateOverlay);

  // ------------------------------------------------------------ the buttons

  Array.prototype.forEach.call(document.querySelectorAll("button[data-key]"), function (button) {
    var code = keyCode(TOUCH_KEYS[button.dataset.key]);
    button.addEventListener("pointerdown", function () { press(touchHeld, code); });
    ["pointerup", "pointercancel", "pointerleave"].forEach(function (name) {
      button.addEventListener(name, function () { release(touchHeld, code); });
    });
    button.addEventListener("click", function (event) {
      // a keyboard press on the button clicks it with detail 0: send a whole press
      if (event.detail === 0) { press(touchHeld, code); release(touchHeld, code); }
    });
  });

  // ------------------------------------------------------------ the mouse

  function cellOf(point) {
    var rect = canvas.getBoundingClientRect();
    var x = Math.floor((point.clientX - rect.left) / rect.width * cols);
    var y = Math.floor((point.clientY - rect.top) / rect.height * rows);
    lastCell = [Math.max(0, Math.min(cols - 1, x)), Math.max(0, Math.min(rows - 1, y))];
    return lastCell;
  }

  canvas.addEventListener("mousemove", function (event) {
    if (!games) return;
    var cell = cellOf(event);
    games.mouse_event(mouseButton === 1 ? 5 : mouseButton === 2 ? 6 : 0, cell[0], cell[1]);
  });

  canvas.addEventListener("mousedown", function (event) {
    if (!games) return;
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    var cell = cellOf(event);
    if (event.button === 0) { mouseButton = 1; games.mouse_event(1, cell[0], cell[1]); }
    else if (event.button === 2) { mouseButton = 2; games.mouse_event(3, cell[0], cell[1]); }
  });

  window.addEventListener("mouseup", function () {
    if (!games || mouseButton === 0) return;
    games.mouse_event(mouseButton === 1 ? 2 : 4, lastCell[0], lastCell[1]);
    mouseButton = 0;
  });

  canvas.addEventListener("contextmenu", function (event) { event.preventDefault(); });

  function touch(event, kind) {
    if (!games) return;
    event.preventDefault();
    var point = event.touches[0] || event.changedTouches[0];
    var cell = cellOf(point);
    games.mouse_event(kind, cell[0], cell[1]);
  }

  canvas.addEventListener("touchstart", function (event) {
    if (!games) return;
    canvas.focus({ preventScroll: true });
    mouseButton = 1;
    touch(event, 0);
    games.mouse_event(1, lastCell[0], lastCell[1]);
  }, { passive: false });

  canvas.addEventListener("touchmove", function (event) { touch(event, 5); }, { passive: false });

  canvas.addEventListener("touchend", function (event) {
    event.preventDefault();
    if (games && mouseButton !== 0) games.mouse_event(2, lastCell[0], lastCell[1]);
    mouseButton = 0;
  }, { passive: false });

  // ------------------------------------------------------------ the games

  function buildSlider(i) {
    var name = readText(games.tuning_name_len(i), function (j) { return games.tuning_name_byte(i, j); });
    var label = readText(games.tuning_label_len(i), function (j) { return games.tuning_label_byte(i, j); });
    var min = games.tuning_min(i), max = games.tuning_max(i), value = games.tuning_value(i);
    var row = document.createElement("label");
    var text = document.createElement("span");
    text.textContent = label;
    var input = document.createElement("input");
    input.type = "range";
    input.setAttribute("data-knob", name);
    input.min = String(min);
    input.max = String(max);
    input.step = String((max - min) / 100);
    input.value = String(value);
    var output = document.createElement("output");
    output.textContent = value.toFixed(2);
    input.addEventListener("input", function () {
      var number = Number(input.value);
      games.tuning_set(i, number);
      output.textContent = number.toFixed(2);
    });
    row.appendChild(text);
    row.appendChild(input);
    row.appendChild(output);
    return row;
  }

  function buildSliders() {
    sliders.textContent = "";
    var count = games.tuning_count();
    if (count === 0) {
      var none = document.createElement("p");
      none.className = "dim";
      none.textContent = "This game exposes no numbers to tune.";
      sliders.appendChild(none);
      return;
    }
    for (var i = 0; i < count; i++) sliders.appendChild(buildSlider(i));
  }

  function showSource(id) {
    var source = $("source-" + id);
    sourceBox.textContent = source ? source.value : "";
    Array.prototype.forEach.call(document.querySelectorAll("[data-source-line]"), function (line) {
      line.hidden = line.getAttribute("data-source-line") !== id;
    });
  }

  function start(button) {
    if (!games) return;
    selected = button;
    games.load(Number(button.dataset.index), seed);
    Array.prototype.forEach.call(buttons, function (other) {
      var active = other === button;
      other.className = active ? "active" : "";
      other.setAttribute("aria-pressed", active ? "true" : "false");
    });
    blurb.textContent = button.dataset.blurb;
    keysLine.textContent = button.dataset.keys;
    statusLine.textContent = "playing " + button.dataset.game;
    showSource(button.dataset.game);
    buildSliders();
  }

  Array.prototype.forEach.call(buttons, function (button) {
    button.addEventListener("click", function () { start(button); });
  });

  $("restart").addEventListener("click", function () {
    if (!selected) return;
    start(selected);
    canvas.focus({ preventScroll: true });
  });

  // ---------------------------------------------------------------- start

  fetch("games.wasm")
    .then(function (response) { return response.arrayBuffer(); })
    .then(function (bytes) { return WebAssembly.instantiate(bytes, {}); })
    .then(function (result) {
      games = result.instance.exports;
      cols = games.width();
      rows = games.height();
      for (var i = 0; i < 16; i++) colours.push(css(games.palette(i)));
      canvas.width = cols * CELL_W;
      canvas.height = rows * CELL_H;
      var defaultId = $("play").getAttribute("data-default");
      start(document.querySelector("button[data-game=\"" + defaultId + "\"]"));
      requestAnimationFrame(function (now) { last = now; frame(now); });
    })
    .catch(function (error) {
      statusLine.textContent = "could not start the games: " + error;
    });
})();
"####;

#[cfg(test)]
mod tests {
    use super::super::shell::top_bar;
    use super::*;
    use std::path::Path;

    fn repo_file(path: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
    }

    /// The code block under `## Games` in llms.txt, found without the code under test.
    fn llms_games_block() -> String {
        let text = repo_file("llms.txt");
        let games = text
            .split("\n## Games\n")
            .nth(1)
            .expect("llms.txt has no ## Games section");
        let games = games.split("\n## ").next().unwrap();
        let start = games.find("```rust\n").expect("no rust block in ## Games") + 8;
        let end = start + games[start..].find("\n```").expect("unclosed rust block");
        games[start..end].to_string()
    }

    /// The `games.something(` calls in the page's script.
    fn exports_the_script_calls() -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = JS;
        while let Some(at) = rest.find("games.") {
            let after = &rest[at + 6..];
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
        assert!(page.contains(&format!(
            "<link rel=\"canonical\" href=\"{SITE_URL}games/\">"
        )));
        assert!(page.contains("<title>"));
        assert!(page.trim_end().ends_with("</html>"));
    }

    #[test]
    fn the_top_bar_marks_games_as_the_current_page() {
        assert!(page().contains(&top_bar(Page::Games)));
    }

    #[test]
    fn the_title_is_rainbow_lettered_and_still_readable_by_screen_readers() {
        let page = page();
        assert!(page.contains("<h1 aria-label=\"TERMINAL GAMES\">"));
        assert!(page.contains("animation-delay"));
        assert!(page.contains("prefers-reduced-motion"));
    }

    #[test]
    fn it_says_honestly_what_the_games_are() {
        let page = page();
        assert!(page.contains("WebAssembly"));
        assert!(page.contains("half-block"));
        assert!(page.contains("compiled"));
    }

    #[test]
    fn it_links_to_the_repo_the_crate_and_the_docs() {
        let page = page();
        assert!(page.contains(&format!("href=\"{REPO_URL}\"")));
        assert!(page.contains("https://crates.io/crates/terminal_graphics_interactive"));
        assert!(page.contains("https://docs.rs/terminal_graphics_interactive"));
    }

    // ------------------------------------------------------------ the gallery

    #[test]
    fn there_are_three_games_in_the_order_the_wasm_numbers_them() {
        let ids: Vec<&str> = GAMES.iter().map(|g| g.id).collect();
        assert_eq!(ids, ["glow", "sail", "quickgame"]);
        let indexes: Vec<u32> = GAMES.iter().map(|g| g.index).collect();
        assert_eq!(indexes, [0, 1, 2]);
    }

    #[test]
    fn there_is_a_button_for_every_game_with_its_blurb_and_keys() {
        let page = page();
        assert!(page.contains("<p class=\"examples\" role=\"group\" aria-label=\"Games\">"));
        for game in GAMES.iter() {
            let button = format!(
                "<button type=\"button\" data-game=\"{}\" data-index=\"{}\" data-blurb=\"{}\" data-keys=\"{}\">{}</button>",
                game.id,
                game.index,
                escape(game.blurb),
                escape(game.keys),
                game.title
            );
            assert!(page.contains(&button), "no button for {}", game.id);
        }
    }

    #[test]
    fn the_default_game_is_glow_and_the_page_shows_its_blurb_and_keys() {
        let page = page();
        assert!(page.contains("<section id=\"play\" data-default=\"glow\">"));
        assert!(page.contains("<p id=\"blurb\" class=\"blurb\">"));
        assert!(page.contains("<p id=\"keys\" class=\"dim\">"));
    }

    #[test]
    fn the_status_line_says_when_the_games_are_loaded() {
        let page = page();
        assert!(page.contains(
            "<p id=\"status\" role=\"status\" aria-live=\"polite\">loading the games ...</p>"
        ));
    }

    // ------------------------------------------------------------- the stage

    #[test]
    fn the_screen_is_a_focusable_canvas_of_800_by_600_with_an_overlay() {
        let page = page();
        assert!(page.contains(
            "<canvas id=\"screen\" tabindex=\"0\" width=\"800\" height=\"600\" aria-label=\""
        ));
        assert!(page.contains("<div class=\"screen\">"));
        assert!(page.contains("<div id=\"overlay\" class=\"overlay\">click to play</div>"));
        assert!(page.contains("<button id=\"restart\" type=\"button\">RESTART</button>"));
        assert!(page.contains("the game ended - press RESTART"));
    }

    #[test]
    fn the_touch_buttons_cover_the_arrows_enter_and_pause() {
        let page = page();
        assert!(page.contains("<div id=\"touch\" class=\"touch\">"));
        for key in ["Left", "Right", "Up", "Down", "Enter", "Pause"] {
            assert!(
                page.contains(&format!(
                    "<button type=\"button\" class=\"key\" data-key=\"{key}\""
                )),
                "no touch button for {key}"
            );
        }
        assert!(page.contains("touch-action: manipulation"));
        assert!(page.contains("user-select: none"));
    }

    #[test]
    fn the_script_drives_the_touch_buttons_with_pointer_events() {
        for needle in [
            "pointerdown",
            "pointerup",
            "pointercancel",
            "pointerleave",
            "event.detail",
        ] {
            assert!(JS.contains(needle), "the script does not use {needle}");
        }
    }

    #[test]
    fn the_script_releases_keys_when_the_canvas_loses_focus() {
        for needle in [
            "keydown",
            "keyup",
            "blur",
            "event.repeat",
            "ctrlKey",
            "metaKey",
            "altKey",
        ] {
            assert!(JS.contains(needle), "the script does not use {needle}");
        }
    }

    // ----------------------------------------------------------------- tuner

    #[test]
    fn the_tuner_explains_itself_and_has_a_place_for_the_sliders() {
        let page = page();
        assert!(page.contains("<section id=\"tuner\">"));
        assert!(page.contains("&gt; THE TUNER"));
        assert!(page.contains("Drag a slider and the running game changes."));
        assert!(page.contains("<code>Tuning</code> in the crate docs"));
        assert!(page.contains("<div id=\"tuner-sliders\">"));
    }

    #[test]
    fn the_sliders_are_built_without_innerhtml_and_say_when_a_game_has_none() {
        assert!(JS.contains("createElement"));
        assert!(JS.contains("data-knob"));
        assert!(JS.contains("This game exposes no numbers to tune."));
        assert!(JS.contains("tuning_set("));
    }

    // ---------------------------------------------------------------- source

    #[test]
    fn the_source_section_shows_each_game_file_from_the_repository() {
        let page = page();
        assert!(page.contains("<section id=\"source-section\">"));
        assert!(page.contains("&gt; THE SOURCE"));
        assert!(page.contains("<pre id=\"source\" tabindex=\"0\" class=\"source\"></pre>"));
        for game in GAMES.iter() {
            let file = repo_file(&format!("examples/{}.rs", game.id));
            let textarea = format!(
                "<textarea id=\"source-{}\" hidden readonly>{}</textarea>",
                game.id,
                escape(&file)
            );
            assert!(
                page.contains(&textarea),
                "the source of {} is not in the page",
                game.id
            );
        }
    }

    #[test]
    fn each_game_source_links_to_its_file_and_says_how_to_run_it() {
        let page = page();
        for game in GAMES.iter() {
            let link = format!("href=\"{REPO_URL}/blob/master/examples/{}.rs\"", game.id);
            assert!(page.contains(&link), "no link for {}", game.id);
            assert!(page.contains(&format!("cargo run --example {}", game.id)));
        }
    }

    // ------------------------------------------------------------ make your own

    #[test]
    fn make_your_own_shows_the_cargo_commands_and_mentions_headless() {
        let page = page();
        assert!(page.contains("<section id=\"make\">"));
        assert!(page.contains("&gt; MAKE YOUR OWN"));
        assert!(page.contains("$ cargo add terminal_graphics_interactive"));
        assert!(page.contains("$ cargo run"));
        assert!(page.contains("Headless"));
    }

    #[test]
    fn the_example_is_the_complete_game_from_the_games_section_of_llms_txt() {
        let block = llms_games_block();
        assert_eq!(llms_game_example(), block);
        assert!(page().contains(&format!("<pre class=\"code\">{}</pre>", escape(&block))));
    }

    #[test]
    fn the_example_is_the_games_block_not_the_first_rust_block_in_the_file() {
        let text = repo_file("llms.txt");
        let first = text
            .split("```rust\n")
            .nth(1)
            .and_then(|rest| rest.split("\n```").next())
            .unwrap();
        assert_ne!(llms_game_example(), first);
        assert!(llms_game_example().contains("impl Game for Title"));
        assert!(!llms_game_example().contains("```"));
    }

    // ------------------------------------------------------------ multiplayer

    #[test]
    fn multiplayer_explains_sail_together_and_says_plainly_it_is_not_in_the_browser_yet() {
        let page = page();
        assert!(page.contains("<section id=\"multiplayer\">"));
        assert!(page.contains("&gt; MULTIPLAYER"));
        // what exists, and how to run it
        assert!(page.contains("sail_together"));
        assert!(page.contains("cargo run --example sail_together -- host 2"));
        assert!(page.contains("cargo run --example sail_together -- join HOST:7777"));
        assert!(page.contains("lockstep over TCP"));
        // what is and is not verified, and what is not here yet
        assert!(page.contains("not yet in the browser"));
        assert!(page.contains("two Windows PowerShell windows on one laptop"));
        assert!(page.contains("not yet been tried across a real network"));
        assert!(page.contains("no password or encryption"));
        assert!(page.contains("WebSocket"));
        assert!(
            !page.contains("is planned (lockstep over TCP first"),
            "the old sentence says network play is not built"
        );
    }

    // ----------------------------------------------------------------- script

    #[test]
    fn the_page_loads_the_wasm_and_draws_each_frame() {
        let page = page();
        assert!(page.contains("fetch(\"games.wasm\")"));
        assert!(page.contains("WebAssembly.instantiate(bytes, {})"));
        assert!(JS.contains("requestAnimationFrame"));
        assert!(JS.contains("games.tick(Math.min(Math.round(now - last), 250))"));
    }

    #[test]
    fn the_seed_comes_from_the_query_for_tests_and_is_random_otherwise() {
        assert!(JS.contains("location.search"));
        assert!(JS.contains("seed="));
        assert!(JS.contains("Math.random"));
    }

    #[test]
    fn the_mouse_and_the_keyboard_are_handled_like_the_art_page() {
        for needle in [
            "mousemove",
            "mousedown",
            "mouseup",
            "contextmenu",
            "touchstart",
            "touchmove",
            "touchend",
            "keydown",
            "keyup",
            "blur",
            "preventDefault",
        ] {
            assert!(JS.contains(needle), "the script does not use {needle}");
        }
    }

    #[test]
    fn the_shade_blocks_are_drawn_as_rectangles_and_text_is_drawn_with_fill_text() {
        for needle in [
            "0x2580", "0x2584", "0x2588", "0x2591", "0x2592", "0x2593", "fillRect", "fillText",
        ] {
            assert!(JS.contains(needle), "the script does not mention {needle}");
        }
    }

    #[test]
    fn the_script_calls_all_the_exports_it_needs() {
        let called = exports_the_script_calls();
        for needed in [
            "load",
            "tick",
            "key_event",
            "mouse_event",
            "finished",
            "width",
            "height",
            "frame_ptr",
            "palette",
            "tuning_count",
            "tuning_name_len",
            "tuning_name_byte",
            "tuning_label_len",
            "tuning_label_byte",
            "tuning_min",
            "tuning_max",
            "tuning_value",
            "tuning_set",
        ] {
            assert!(
                called.contains(&needed.to_string()),
                "the script never calls games.{needed}()"
            );
        }
    }

    #[test]
    fn the_script_does_not_use_template_markers_or_unsafe_html() {
        assert!(
            !JS.contains("{{"),
            "the script must not contain template markers"
        );
        assert!(!JS.contains("innerHTML"), "build text with textContent");
        assert!(!JS.contains("eval("));
    }

    #[test]
    fn every_function_the_script_calls_is_really_exported_by_web_games() {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/web_games.rs"),
        )
        .unwrap_or_else(|_| {
            panic!("examples/web_games.rs does not exist yet (another agent is writing it)")
        });
        for name in exports_the_script_calls() {
            assert!(
                source.contains(&format!("pub extern \"C\" fn {name}(")),
                "the page calls games.{name}() but web_games.rs does not export it"
            );
        }
    }
}

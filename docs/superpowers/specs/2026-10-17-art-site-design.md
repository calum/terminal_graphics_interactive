# The art website

## Why

The crate, README, `llms.txt` and docs are for people and AI agents together.
The website is different: it is for **humans**, to make **human art**. Someone
who has never installed Rust should be able to open the page, change a number,
press a button and watch something beautiful move, with their mouse in it. The
site is also the best advertisement the crate can have: it lets you feel the
whole idea in thirty seconds.

## What it is

A retro "BBS" page (the same look as the `terminal_graphics` site: CRT
scanlines, a rainbow title, the 16 ANSI colours) with four parts:

1. **A gallery of examples** you can click: plasma, rings, tunnel, waves, balls,
   bubbles. Each loads its code into the editor and runs it.
2. **The studio:** a code editor with line numbers, a RUN button, and a terminal
   below it showing the result live. The mouse works in the terminal exactly as
   it does in a real one. Errors appear in a "compiler" panel written like
   rustc's, with the line and column, and the editor jumps to the line.
3. **A short API guide:** one screen of reference, written for someone who just
   wants to make a cool piece of art, generated from the same tables the editor
   uses so it can never disagree with it.
4. **Share:** the code is stored in the link, so "here is my art" is one URL.

## The hard part, and the decision

A browser cannot compile Rust, so the editor cannot run arbitrary Rust. The
options were to call a server (the Rust Playground), which cannot show an
interactive terminal and needs a network round trip per run; or to **interpret a
small, strict subset of Rust in the browser**. I chose the second.

The interpreter (`examples/site/script/`, already merged for the README GIFs) is
written in Rust, compiled to WebAssembly together with the real library, so the
page runs the real `sketch_frame`, `Palette` and `Simulation`. It understands
what art needs (`let`, `if`, `for`, helper functions, the `f32` maths methods,
`sketch`, `World`) and is deliberately **as strict as rustc**, so code that runs
on the page also compiles in a real project. The guide says exactly what is
supported, and a program that needs a real terminal gets a clear message.

## Goals

- Fun first: it should load fast, start running something pretty immediately,
  and make "change a number, run it" feel good.
- Human art, not a docs page: the voice is playful; the guide is short.
- Everything on the page is real: the examples are the repo's example files
  (a test checks that), and the guide's lists come from the interpreter's tables.
- The mouse works, including right-click, drag and touch.
- 100% Rust in the repo: the page, CSS and JS are generated from Rust by
  `examples/build_site.rs`, as in the other repo. No other source language.

## Non-goals

- Running arbitrary Rust, other crates, or programs that need a terminal.
- Saving to a server, accounts, or a gallery of other people's work. (Sharing
  is a link, nothing more.)
- AI-generated art or an AI interface. The page is for humans; the README,
  `llms.txt` and crate docs are where AI agents are served.

## Decisions (mine unless marked)

- **One WebAssembly module, no wasm-bindgen:** plain `extern "C"` exports and a
  frame buffer the JS reads, as in the other repo. Source code goes in byte by
  byte (`source_push`) so the Rust side needs no `unsafe`.
- **Shade blocks are drawn as rectangles with transparency**, not as font
  glyphs, so they look the same in every browser and match the README GIFs.
- **A step budget per frame** stops a runaway loop in someone's art, and the
  page says so instead of freezing the tab.
- **The examples are tested:** each compiles, animates, and reacts to the mouse
  (or the world moves), in the same code path the page uses.
- **Share links** put the code in the URL hash, so nothing is stored anywhere.

## How we will know it works

- Unit tests for the machine behind the exports (compile, errors, ticking, the
  mouse, failures) and for the page generator.
- Every example is compiled, animated and checked for differing output.
- The page is built, served, and loaded in a headless browser to check it
  starts, runs the default example, and shows errors.

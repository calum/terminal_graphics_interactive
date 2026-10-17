//! Render benchmarks. Run with `cargo bench --bench render`.
//!
//! Uses only `std` (no benchmark crate). Each scenario's frames are built up
//! front, so the timing covers only turning a `Display` into bytes and not
//! animating it. The frames cycle, so the time is an average over the cycle.
//! Bytes are the average over one whole cycle.
//!
//! Two ways of writing a frame are measured on the same frames:
//! - `full redraw`: `Display::render`, which builds the whole frame every time;
//! - `renderer`: `Renderer::draw_to`, which writes only the cells that changed
//!   since the previous frame. Its bytes are measured after one warm-up cycle,
//!   so the first full frame is not counted.

use std::hint::black_box;
use std::time::{Duration, Instant};
use terminal_graphics::{Colour, Display};
use terminal_graphics_interactive::Renderer;

const SAMPLES: usize = 15;
const SAMPLE_TIME: Duration = Duration::from_millis(100);
/// Frames in one cycle of each scenario.
const FRAMES: u64 = 120;
const RAMP: &str = " .:-=+*#%@";
const COLOURS: [Colour; 6] = [
    Colour::Red,
    Colour::Green,
    Colour::Yellow,
    Colour::Blue,
    Colour::Magenta,
    Colour::Cyan,
];
const SIZES: [(u32, u32); 2] = [(80, 24), (200, 60)];

#[derive(Clone, Copy)]
enum Scenario {
    /// The same frame every time.
    Still,
    /// A 3 by 3 block that moves one cell to the right per frame, on a blank background.
    Moving,
    /// Every cell changes every frame.
    Plasma,
}

impl Scenario {
    const ALL: [(&'static str, Scenario); 3] = [
        ("still", Scenario::Still),
        ("moving", Scenario::Moving),
        ("plasma", Scenario::Plasma),
    ];
}

/// One plasma-like frame: the character and colour depend on `x + y + frame`.
fn plasma_frame(width: u32, height: u32, frame: u64) -> Display {
    let ramp: Vec<char> = RAMP.chars().collect();
    let mut display = Display::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let k = x as u64 + y as u64 + frame;
            let character = ramp[(k % ramp.len() as u64) as usize];
            let colour = COLOURS[(k % COLOURS.len() as u64) as usize];
            display.set_pixel(x as isize, y as isize, character, colour, Colour::Black);
        }
    }
    display
}

/// Paints or erases a 3 by 3 block whose top left cell is `(x, y)`.
fn paint_block(display: &mut Display, x: isize, y: isize, erase: bool) {
    for dy in 0..3 {
        for dx in 0..3 {
            if erase {
                display.set_pixel(x + dx, y + dy, ' ', Colour::Black, Colour::White);
            } else {
                display.set_pixel(x + dx, y + dy, '#', Colour::Red, Colour::Blue);
            }
        }
    }
}

/// The frames of one scenario at one size.
fn frames(scenario: Scenario, width: u32, height: u32) -> Vec<Display> {
    match scenario {
        Scenario::Still => {
            let frame = plasma_frame(width, height, 0);
            (0..FRAMES).map(|_| frame.clone()).collect()
        }
        Scenario::Moving => {
            let span = width.saturating_sub(3).max(1) as u64;
            let top = (height / 2).saturating_sub(1) as isize;
            let mut display = Display::new(width, height);
            let mut previous: Option<isize> = None;
            (0..FRAMES)
                .map(|frame| {
                    let x = (frame % span) as isize;
                    if let Some(old) = previous {
                        paint_block(&mut display, old, top, true);
                    }
                    paint_block(&mut display, x, top, false);
                    previous = Some(x);
                    display.clone()
                })
                .collect()
        }
        Scenario::Plasma => (0..FRAMES)
            .map(|frame| plasma_frame(width, height, frame))
            .collect(),
    }
}

/// Median time per call of `f`, in nanoseconds.
fn time_per_call(mut f: impl FnMut()) -> f64 {
    let mut calls = 1u64;
    loop {
        let start = Instant::now();
        for _ in 0..calls {
            f();
        }
        if start.elapsed() >= SAMPLE_TIME / 10 {
            break;
        }
        calls *= 2;
    }
    let calls = calls * 10;

    let mut samples: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..calls {
                f();
            }
            start.elapsed().as_nanos() as f64 / calls as f64
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[SAMPLES / 2]
}

/// What the machine is, for the numbers to be read against.
fn machine() -> String {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let model = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, model)| model.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{model}, {threads} threads, {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn main() {
    println!("terminal_graphics_interactive render benchmarks (median of {SAMPLES} samples)");
    println!("machine: {}\n", machine());

    for (name, scenario) in Scenario::ALL {
        for &(width, height) in &SIZES {
            let frames = frames(scenario, width, height);

            let mut index = 0usize;
            let full_ns = time_per_call(|| {
                let frame = &frames[index % frames.len()];
                index += 1;
                black_box(frame.render());
            });
            let full_bytes: usize = frames.iter().map(|f| f.render().len()).sum();
            let full_bytes = full_bytes as f64 / frames.len() as f64;

            let mut renderer = Renderer::new();
            let mut out: Vec<u8> = Vec::new();
            for frame in &frames {
                out.clear();
                renderer.draw_to(frame, &mut out).unwrap();
            }
            let mut bytes = 0usize;
            for frame in &frames {
                out.clear();
                bytes += renderer.draw_to(frame, &mut out).unwrap();
            }
            let renderer_bytes = bytes as f64 / frames.len() as f64;

            let mut index = 0usize;
            let renderer_ns = time_per_call(|| {
                let frame = &frames[index % frames.len()];
                index += 1;
                out.clear();
                black_box(renderer.draw_to(frame, &mut out).unwrap());
            });

            println!(
                "{name:<8} {width:>4}x{height:<3} full redraw {:>9.1} us {full_bytes:>9.0} B | renderer {:>9.1} us {renderer_bytes:>9.0} B",
                full_ns / 1000.0,
                renderer_ns / 1000.0,
            );
        }
    }
}

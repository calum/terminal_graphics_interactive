//! Parser benchmarks. Run with `cargo bench`.
//!
//! Uses only `std` (no benchmark crate) so the crate keeps a single
//! dependency. Reports the median time to parse each stream, as events per
//! second and megabytes per second.

use std::hint::black_box;
use std::time::{Duration, Instant};
use terminal_graphics_interactive::Parser;

const SAMPLES: usize = 15;
const SAMPLE_TIME: Duration = Duration::from_millis(100);

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

/// A drag across the screen: many SGR mouse reports, the busiest input there is.
fn mouse_drag(events: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for i in 0..events {
        bytes.extend_from_slice(format!("\x1b[<32;{};{}M", 1 + i % 200, 1 + i % 60).as_bytes());
    }
    bytes
}

/// Typing with arrow keys and function keys mixed in.
fn keyboard(events: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for i in 0..events {
        match i % 4 {
            0 => bytes.extend_from_slice(b"a"),
            1 => bytes.extend_from_slice(b"\x1b[A"),
            2 => bytes.extend_from_slice(b"\x1b[1;5C"),
            _ => bytes.extend_from_slice(b"\x1b[15~"),
        }
    }
    bytes
}

fn bench(name: &str, input: &[u8], events: usize, chunk: usize) {
    let mut parser = Parser::new();
    let ns = time_per_call(|| {
        for part in input.chunks(chunk) {
            parser.feed(black_box(part));
            while let Some(event) = parser.next_event() {
                black_box(event);
            }
        }
    });
    println!(
        "{name:<26} chunk {chunk:>6} B {:>9.1} ns/event {:>8.1} M events/s {:>8.1} MB/s",
        ns / events as f64,
        events as f64 / ns * 1000.0,
        input.len() as f64 / ns * 1000.0,
    );
}

fn main() {
    println!("terminal_graphics_interactive parser benchmarks (median of {SAMPLES} samples)\n");
    let events = 10_000;
    let drag = mouse_drag(events);
    let keys = keyboard(events);
    // a read from a real terminal is a handful of events; a paste is much more
    for chunk in [64, 4096, 1 << 20] {
        bench("mouse drag (SGR)", &drag, events, chunk);
    }
    println!();
    for chunk in [64, 4096, 1 << 20] {
        bench("keyboard", &keys, events, chunk);
    }
}

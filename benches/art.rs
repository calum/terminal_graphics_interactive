//! Art benchmarks. Run with `cargo bench --bench art`.
//!
//! Uses only `std` (no benchmark crate). Reports the median time to compute
//! one frame, and what share of the 33 ms a frame has at 30 frames a second
//! that is. These measure the maths only: turning a `Display` into output is
//! `terminal_graphics`' job and is benchmarked there.

use std::hint::black_box;
use std::time::{Duration, Instant};
use terminal_graphics_interactive::{sketch_frame, Display, Palette, Pointer, World};

const SAMPLES: usize = 15;
const SAMPLE_TIME: Duration = Duration::from_millis(100);
const FRAME_BUDGET_US: f64 = 33_000.0;

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

fn report(name: &str, size: (u32, u32), ns: f64) {
    let micros = ns / 1000.0;
    println!(
        "{name:<34} {:>4}x{:<3} {micros:>10.1} us/frame {:>6.1}% of a 30 fps frame",
        size.0,
        size.1,
        micros / FRAME_BUDGET_US * 100.0
    );
}

fn main() {
    println!("terminal_graphics_interactive art benchmarks (median of {SAMPLES} samples)\n");

    for &(width, height) in &[(80u32, 24u32), (200, 60)] {
        let pointer = Pointer {
            x: 30.0,
            y: 10.0,
            ..Pointer::default()
        };

        // the plasma example: three sines and a distance per cell
        let mut display = Display::new(width, height);
        let mut t = 0.0f32;
        let ns = time_per_call(|| {
            t += 0.033;
            sketch_frame(
                &mut display,
                &Palette::fire(),
                t,
                black_box(&pointer),
                |cell| {
                    let ripple = cell.dist_to_mouse() * 0.25 - cell.t * 3.0;
                    ((cell.x * 0.15 + cell.t).sin()
                        + (cell.y * 0.2 - cell.t * 0.7).sin()
                        + ripple.sin())
                        / 3.0
                },
            );
        });
        report("sketch: plasma (3 sines per cell)", (width, height), ns);

        let mut display = Display::new(width, height);
        let ns = time_per_call(|| {
            sketch_frame(
                &mut display,
                &Palette::ocean(),
                1.0,
                black_box(&pointer),
                |cell| cell.x.sin(),
            );
        });
        report("sketch: one sine per cell", (width, height), ns);
    }
    println!();

    for &(width, height) in &[(80u32, 24u32), (200, 60)] {
        let pointer = Pointer {
            x: 30.0,
            y: 10.0,
            ..Pointer::default()
        };
        for (name, world) in [
            (
                "world: 5 big balls with trails",
                World::new()
                    .balls(5)
                    .size(3.5)
                    .gravity(0.02)
                    .mouse_pulls()
                    .trails(),
            ),
            (
                "world: 64 small balls with trails",
                World::new()
                    .balls(64)
                    .size(1.0)
                    .gravity(0.02)
                    .mouse_pulls()
                    .trails(),
            ),
        ] {
            let mut simulation = world.simulation(42);
            simulation.resize(width, height);
            let mut display = Display::new(width, height);
            let ns = time_per_call(|| {
                simulation.step(black_box(&pointer));
                simulation.render(&mut display);
            });
            report(name, (width, height), ns);
        }
    }
}

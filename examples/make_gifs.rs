//! Generates the animated GIFs in `assets/` that the README shows.
//!
//! ```text
//! cargo run --release --example make_gifs [output-directory]
//! ```
//!
//! Each GIF runs the real source of an example (`examples/plasma.rs`,
//! `examples/balls.rs`) through the same interpreter the website uses, against
//! the real library, while a scripted mouse moves, clicks and drags. The mouse
//! arrow and click rings are drawn on top so you can see what it is doing.
//! Everything here is Rust, so the GIFs can be rebuilt by anyone.

#[allow(dead_code)]
#[path = "site/gif.rs"]
mod gif;
#[allow(dead_code)]
#[path = "site/raster.rs"]
mod raster;
#[allow(dead_code)]
#[path = "site/script/mod.rs"]
mod script;

use raster::{Mouse, Raster};
use std::path::{Path, PathBuf};
use terminal_graphics_interactive::{sketch_frame, Display, Pointer};

/// Where the scripted mouse is at one moment, in cells.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MouseFrame {
    x: f32,
    y: f32,
    left: bool,
    right: bool,
    /// The left button was pressed on exactly this frame.
    click: bool,
}

struct Take {
    name: &'static str,
    source: &'static str,
    columns: u32,
    rows: u32,
    frames: usize,
    /// How many 30 frames a second steps of the animation pass between pictures.
    steps_per_frame: u32,
    delay: u16,
    seed: u32,
    /// The mouse for picture `frame` of `frames`.
    mouse: fn(usize, usize) -> MouseFrame,
}

const PLASMA_SOURCE: &str = include_str!("plasma.rs");
const BALLS_SOURCE: &str = include_str!("balls.rs");

/// Moves from `from` to `to` as `t` goes from 0.0 to 1.0, easing at both ends.
fn glide(from: f32, to: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    from + (to - from) * eased
}

/// The plasma: the mouse drifts around the screen in a loop, then clicks once.
fn plasma_mouse(frame: usize, total: usize) -> MouseFrame {
    let u = frame as f32 / total as f32;
    let tau = std::f32::consts::TAU;
    let click_at = total / 2;
    MouseFrame {
        x: 26.0 + 17.0 * (tau * u).sin(),
        y: 13.0 + 8.0 * (2.0 * tau * u + 0.6).sin(),
        left: (click_at..click_at + 3).contains(&frame),
        right: false,
        click: frame == click_at,
    }
}

/// The balls: the pointer pulls them across, a click adds a ball, then the
/// right button pushes them away.
fn balls_mouse(frame: usize, _total: usize) -> MouseFrame {
    let at = |from: (f32, f32), to: (f32, f32), start: usize, length: usize| {
        let t = (frame.saturating_sub(start)) as f32 / length as f32;
        (glide(from.0, to.0, t), glide(from.1, to.1, t))
    };
    let (x, y) = match frame {
        0..=27 => at((40.0, 5.0), (12.0, 19.0), 0, 27),
        28..=45 => at((12.0, 19.0), (30.0, 12.0), 34, 11),
        46..=75 => at((30.0, 12.0), (24.0, 14.0), 46, 29),
        _ => at((24.0, 14.0), (46.0, 5.0), 76, 13),
    };
    MouseFrame {
        x,
        y,
        left: (30..33).contains(&frame),
        right: (50..72).contains(&frame),
        click: frame == 30,
    }
}

fn takes() -> Vec<Take> {
    vec![
        Take {
            name: "plasma",
            source: PLASMA_SOURCE,
            columns: 52,
            rows: 26,
            frames: 72,
            steps_per_frame: 2,
            delay: 7,
            seed: 1,
            mouse: plasma_mouse,
        },
        Take {
            name: "balls",
            source: BALLS_SOURCE,
            columns: 52,
            rows: 26,
            frames: 90,
            steps_per_frame: 2,
            delay: 7,
            seed: 7,
            mouse: balls_mouse,
        },
    ]
}

/// How long a click ring lasts, in pictures.
const RING_FRAMES: usize = 6;

/// The pictures of a take, mouse included.
fn pictures(take: &Take) -> Result<Vec<Raster>, String> {
    let program = script::compile(take.source).map_err(|error| error.to_string())?;
    let formula = program.formula();
    let mut display = Display::new(take.columns, take.rows);
    let mut simulation = match program.scene() {
        script::Scene::World(world) => {
            let mut simulation = world.simulation(take.seed);
            simulation.resize(take.columns, take.rows);
            Some(simulation)
        }
        script::Scene::Sketch(_) => None,
    };

    let mut pictures = Vec::with_capacity(take.frames);
    let mut seconds = 0.0f32;
    // where and when the latest click happened, in pixels and pictures
    let mut ring: Option<(usize, f32, f32)> = None;

    for frame in 0..take.frames {
        let mouse = (take.mouse)(frame, take.frames);
        let mut pointer = Pointer {
            x: mouse.x,
            y: mouse.y,
            left: mouse.left,
            right: mouse.right,
            clicks: Vec::new(),
        };

        match (program.scene(), simulation.as_mut()) {
            (script::Scene::Sketch(palette), _) => {
                seconds += take.steps_per_frame as f32 / 30.0;
                sketch_frame(&mut display, palette, seconds, &pointer, |cell| {
                    formula.call(cell)
                });
                if let Some(why) = formula.failure() {
                    return Err(why);
                }
            }
            (script::Scene::World(_), Some(simulation)) => {
                for step in 0..take.steps_per_frame {
                    pointer.clicks = if mouse.click && step == 0 {
                        vec![(mouse.x, mouse.y)]
                    } else {
                        Vec::new()
                    };
                    simulation.step(&pointer);
                }
                simulation.render(&mut display);
            }
            _ => return Err("internal error: a world without a simulation".into()),
        }

        let mut picture = raster::render(&display);
        // the arrow's tip is the middle of the cell the mouse is over
        let tip_x = (mouse.x + 0.5) * raster::CELL_WIDTH as f32;
        let tip_y = (mouse.y + 0.5) * raster::CELL_HEIGHT as f32;

        if mouse.click {
            ring = Some((frame, tip_x, tip_y));
        }
        if let Some((started, x, y)) = ring {
            let age = frame - started;
            if age < RING_FRAMES {
                let strength = 1.0 - age as f32 / RING_FRAMES as f32;
                raster::draw_ring(
                    &mut picture,
                    x,
                    y,
                    4.0 + age as f32 * 3.0,
                    [255, 230, 0],
                    strength,
                );
            }
        }
        raster::draw_cursor(
            &mut picture,
            Mouse {
                x: tip_x,
                y: tip_y,
                left: mouse.left,
                right: mouse.right,
            },
        );
        pictures.push(picture);
    }
    Ok(pictures)
}

/// The finished GIF.
fn gif_bytes(take: &Take) -> Result<Vec<u8>, String> {
    let pictures = pictures(take)?;
    let Some(first) = pictures.first() else {
        return Err("a take needs at least one frame".into());
    };
    let (width, height) = (first.width, first.height);
    let (palette, indexes) = raster::quantize(&pictures);
    let frames: Vec<gif::Frame> = indexes
        .into_iter()
        .map(|pixels| gif::Frame {
            pixels,
            delay: take.delay,
        })
        .collect();
    Ok(gif::encode(width as u16, height as u16, &palette, &frames))
}

fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("assets"), PathBuf::from);
    std::fs::create_dir_all(&out)?;
    for take in takes() {
        match gif_bytes(&take) {
            Ok(bytes) => {
                let path: PathBuf = Path::new(&out).join(format!("{}.gif", take.name));
                std::fs::write(&path, &bytes)?;
                println!("{}: {} KiB", path.display(), bytes.len() / 1024);
            }
            Err(message) => {
                eprintln!("{}: {message}", take.name);
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLASMA: &str = include_str!("plasma.rs");
    const BALLS: &str = include_str!("balls.rs");

    fn no_mouse(_: usize, _: usize) -> MouseFrame {
        MouseFrame {
            x: 4.0,
            y: 3.0,
            ..MouseFrame::default()
        }
    }

    fn clicking(frame: usize, _: usize) -> MouseFrame {
        MouseFrame {
            x: 6.0 + frame as f32,
            y: 4.0,
            left: frame == 2 || frame == 3,
            right: false,
            click: frame == 2,
        }
    }

    fn small(source: &'static str, mouse: fn(usize, usize) -> MouseFrame) -> Take {
        Take {
            name: "test",
            source,
            columns: 24,
            rows: 8,
            frames: 6,
            steps_per_frame: 2,
            delay: 7,
            seed: 11,
            mouse,
        }
    }

    #[test]
    fn a_take_makes_one_picture_per_frame_at_the_cell_size() {
        for source in [PLASMA, BALLS] {
            let pictures = pictures(&small(source, no_mouse)).unwrap();
            assert_eq!(pictures.len(), 6);
            for picture in &pictures {
                assert_eq!(picture.width, 24 * raster::CELL_WIDTH);
                assert_eq!(picture.height, 8 * raster::CELL_HEIGHT);
            }
        }
    }

    #[test]
    fn the_pictures_animate() {
        for source in [PLASMA, BALLS] {
            let pictures = pictures(&small(source, no_mouse)).unwrap();
            assert_ne!(pictures[0], pictures[5], "the first and last frames differ");
        }
    }

    #[test]
    fn the_scripted_mouse_arrow_is_drawn_where_the_script_says() {
        let with_mouse = pictures(&small(PLASMA, clicking)).unwrap();
        // frame 0: the tip is at the middle of cell (6, 4)
        let (tip_x, tip_y) = (
            6 * raster::CELL_WIDTH + raster::CELL_WIDTH / 2,
            4 * raster::CELL_HEIGHT + raster::CELL_HEIGHT / 2,
        );
        assert_eq!(
            with_mouse[0].get(tip_x, tip_y),
            [0, 0, 0],
            "the arrow's outline is under the tip"
        );
        // frame 1 has moved one cell to the right
        let moved = (tip_x + raster::CELL_WIDTH, tip_y);
        assert_eq!(with_mouse[1].get(moved.0, moved.1), [0, 0, 0]);
    }

    #[test]
    fn holding_the_button_turns_the_arrow_yellow() {
        let pictures = pictures(&small(PLASMA, clicking)).unwrap();
        let has_yellow = |picture: &Raster| picture.pixels.contains(&[255, 230, 0]);
        assert!(!has_yellow(&pictures[0]));
        assert!(has_yellow(&pictures[2]));
        assert!(has_yellow(&pictures[3]));
        assert!(!has_yellow(&pictures[5]));
    }

    #[test]
    fn a_click_sends_out_a_ring_that_fades() {
        let with_ring = pictures(&small(PLASMA, clicking)).unwrap();
        let without = pictures(&small(PLASMA, |frame, total| MouseFrame {
            click: false,
            left: false,
            ..clicking(frame, total)
        }))
        .unwrap();
        // the ring is drawn on the click frame and the ones after it
        assert_ne!(with_ring[3], without[3]);
        assert_ne!(with_ring[4], without[4]);
        assert_ne!(with_ring[2], without[2]);
        // and not before
        assert_eq!(with_ring[1], without[1]);
    }

    #[test]
    fn the_balls_follow_the_scripted_pointer_and_a_click_adds_one() {
        let attract = |_: usize, _: usize| MouseFrame {
            x: 20.0,
            y: 2.0,
            ..MouseFrame::default()
        };
        let pictures_a = pictures(&small(BALLS, attract)).unwrap();
        let pictures_b = pictures(&small(BALLS, |_, _| MouseFrame {
            x: 2.0,
            y: 6.0,
            ..MouseFrame::default()
        }))
        .unwrap();
        assert_ne!(
            pictures_a[5], pictures_b[5],
            "the pointer changes where the balls go"
        );
    }

    #[test]
    fn a_take_is_deterministic() {
        for source in [PLASMA, BALLS] {
            let a = pictures(&small(source, clicking)).unwrap();
            let b = pictures(&small(source, clicking)).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn the_gif_is_valid_looks_like_a_gif_and_is_the_same_every_time() {
        let bytes = gif_bytes(&small(PLASMA, clicking)).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(bytes.last(), Some(&0x3B));
        assert_eq!(
            u16::from_le_bytes([bytes[6], bytes[7]]) as usize,
            24 * raster::CELL_WIDTH
        );
        assert_eq!(
            u16::from_le_bytes([bytes[8], bytes[9]]) as usize,
            8 * raster::CELL_HEIGHT
        );
        assert_eq!(bytes, gif_bytes(&small(PLASMA, clicking)).unwrap());
    }

    #[test]
    fn a_broken_script_is_reported_not_drawn() {
        let broken: &'static str = "fn main() -> std::io::Result<()> { sketch(|cell| cell.x * 2) }";
        let message = pictures(&small(broken, no_mouse)).unwrap_err();
        assert!(message.contains("mismatched types"), "{message}");
    }

    #[test]
    fn a_formula_that_stops_working_is_reported() {
        let runaway: &'static str = "fn main() -> std::io::Result<()> { sketch(|cell| { let mut s = 0.0; for i in 0..2000000000 { s += 1.0; } s }) }";
        let message = pictures(&small(runaway, no_mouse)).unwrap_err();
        assert!(message.contains("too much work"), "{message}");
    }

    #[test]
    fn the_real_takes_are_set_up_sensibly() {
        let takes = takes();
        assert!(takes.len() >= 2);
        for take in &takes {
            assert!(take.frames >= 30 && take.frames <= 200, "{}", take.name);
            assert!(take.columns >= 20 && take.rows >= 10);
            // the picture must fit a README
            assert!(take.columns as usize * raster::CELL_WIDTH <= 640);
            // the scripted mouse stays on the screen for the whole take
            for frame in 0..take.frames {
                let mouse = (take.mouse)(frame, take.frames);
                assert!(
                    (0.0..take.columns as f32).contains(&mouse.x),
                    "{} x at {frame}",
                    take.name
                );
                assert!(
                    (0.0..take.rows as f32).contains(&mouse.y),
                    "{} y at {frame}",
                    take.name
                );
            }
            // there is a click to show
            assert!(
                (0..take.frames).any(|f| (take.mouse)(f, take.frames).click),
                "{}",
                take.name
            );
            assert!(compile_ok(take.source), "{} must compile", take.name);
        }
    }

    fn compile_ok(source: &str) -> bool {
        script::compile(source).is_ok()
    }

    #[test]
    fn the_takes_use_the_real_example_files() {
        let names: Vec<&str> = takes().iter().map(|t| t.name).collect();
        assert!(names.contains(&"plasma") && names.contains(&"balls"));
        for take in takes() {
            let expected = match take.name {
                "plasma" => PLASMA,
                "balls" => BALLS,
                _ => continue,
            };
            assert_eq!(take.source, expected, "{}", take.name);
        }
    }
}

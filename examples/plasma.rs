use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Move the mouse to send ripples across it. Press q to quit.
    sketch_with(Palette::fire(), |cell| {
        let ripple = cell.dist_to_mouse() * 0.25 - cell.t * 3.0;
        ((cell.x * 0.15 + cell.t).sin() + (cell.y * 0.2 - cell.t * 0.7).sin() + ripple.sin()) / 3.0
    })
}

use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Fly into a spinning tunnel. Hold the mouse button to go faster.
    sketch_with(Palette::ocean(), |cell| {
        let dx = cell.x - cell.width / 2.0;
        let dy = cell.y - cell.height / 2.0;
        let angle = dy.atan2(dx);
        let depth = 8.0 / (dx.hypot(dy) + 1.0);
        let speed = if cell.mouse_down { 6.0 } else { 2.0 };
        (angle * 3.0 + depth * 4.0 - cell.t * speed).sin()
    })
}

use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Rainbow rings that follow the mouse. Hold the button to mix in waves.
    sketch_with(Palette::rainbow(), |cell| {
        let rings = (cell.dist_to_mouse() * 0.35 - cell.t * 2.0).sin();
        let waves = (cell.x * 0.1 + cell.y * 0.1 + cell.t).sin();
        if cell.mouse_down {
            rings * waves
        } else {
            (rings + waves) / 2.0
        }
    })
}

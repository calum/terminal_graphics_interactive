use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // An ocean. The mouse makes a glow, and the waves roll on.
    sketch_with(Palette::ocean(), |cell| {
        let swell =
            (cell.x * 0.08 + cell.t).sin() * 0.5 + (cell.x * 0.19 - cell.t * 1.3).sin() * 0.25;
        let surface = cell.height / 2.0 + swell * 6.0;
        let glow = (-(cell.dist_to_mouse() * 0.12)).exp();
        ((surface - cell.y) * 0.25 + glow * 0.5).clamp(-1.0, 1.0)
    })
}

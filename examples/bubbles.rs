use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // Bubbles float up (gravity is negative). The mouse pulls them. Click to add one.
    World::new()
        .balls(14)
        .size(1.6)
        .gravity(-0.004)
        .mouse_pulls()
        .trails()
        .palette(Palette::ocean())
        .run()
}

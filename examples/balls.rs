use terminal_graphics_interactive::prelude::*;

fn main() -> std::io::Result<()> {
    // The mouse pulls the balls in, the right button pushes them away, a click
    // adds a ball. Press q to quit.
    World::new()
        .balls(5)
        .size(3.5)
        .gravity(0.02)
        .mouse_pulls()
        .trails()
        .run()
}

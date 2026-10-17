//! One import should be enough: `use terminal_graphics_interactive::prelude::*;`
//! brings in the graphics types and the interactive ones.

use terminal_graphics_interactive::prelude::*;

#[test]
fn the_prelude_has_the_graphics_types() {
    let mut display = Display::new(4, 2);
    display.set_pixel(0, 0, 'x', Colour::Red, Colour::Black);
    let mut graphics = Graphics::new();
    graphics.add("box", Rect::new(0, 0, 2, 2, Colour::Blue));
    graphics.draw(&mut display);
    Text::new(0, 0, "hi", Colour::White, Colour::Black).draw(&mut display);
    let _ = Ball::new(0.0, 0.0, 1.0, 0.0, Colour::Red);
    let _ = Container::new(0, 0, 3, 3, Colour::Red, Colour::Black);
}

#[test]
fn the_prelude_has_the_interactive_types() {
    let key = Key::new(KeyCode::Char('q'));
    assert!(key.is_char('q'));
    let mouse = Mouse {
        kind: MouseKind::Down(MouseButton::Left),
        x: 1,
        y: 1,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(mouse.cell_in(&Display::new(4, 4)), Some((1, 1)));
    let mut parser = Parser::new();
    parser.feed(b"a");
    assert!(matches!(parser.next_event(), Some(Event::Key(_))));
    // Terminal and RawMode need a real terminal, so only check they are named
    let _ = Terminal::start as fn() -> std::io::Result<Terminal>;
    let _ = RawMode::enable as fn() -> std::io::Result<RawMode>;
}

#[test]
fn the_prelude_has_the_art_api() {
    // naming the functions inside wrappers that are never called proves they
    // are in scope without starting a terminal
    fn _plain<F: FnMut(&Cell) -> f32>(formula: F) -> std::io::Result<()> {
        sketch(formula)
    }
    fn _with_palette<F: FnMut(&Cell) -> f32>(formula: F) -> std::io::Result<()> {
        sketch_with(Palette::fire(), formula)
    }
    let palette = Palette::new(&[Colour::Black, Colour::Red]);
    assert_eq!(palette.colours().len(), 2);
    let _ = World::new()
        .balls(5)
        .size(3.5)
        .gravity(0.02)
        .mouse_pulls()
        .trails()
        .palette(Palette::ocean());
}

#[test]
fn the_art_api_can_be_used_without_a_terminal() {
    // render one frame of a sketch into a Display
    let mut display = Display::new(20, 6);
    let pointer = Pointer::default();
    sketch_frame(&mut display, &Palette::ocean(), 1.0, &pointer, |cell| {
        (cell.x * 0.3 + cell.t).sin()
    });
    assert_ne!(
        display.pixel(0, 0),
        display.pixel(5, 0),
        "the formula changes across the row"
    );

    // and advance a physics world frame by frame
    let mut simulation = World::new().balls(3).size(2.0).gravity(0.02).simulation(7);
    simulation.resize(20, 6);
    for _ in 0..50 {
        simulation.step(&pointer);
        simulation.render(&mut display);
    }
    assert_eq!(simulation.positions().len(), 3);
    for (x, y) in simulation.positions() {
        assert!((0.0..20.0).contains(&x) && (0.0..6.0).contains(&y));
    }
}

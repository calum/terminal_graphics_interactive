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

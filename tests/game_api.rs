//! The game API used the way a user would, through the public surface only:
//! a two-scene game (menu, play, pause, game over) played headless.

use terminal_graphics_interactive::game::*;

struct Menu;

impl Game for Menu {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play { score: 0 })
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        screen.text(0, 0, "press Enter", Colour::White);
    }
}

struct Play {
    score: u32,
}

impl Game for Play {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Esc) {
            return Next::push(Paused);
        }
        self.score += 1;
        if self.score == 5 {
            return Next::switch(GameOver { score: self.score });
        }
        Next::stay()
    }

    fn draw(&self, screen: &mut Screen) {
        screen.text(0, 0, &format!("score {}", self.score), Colour::Yellow);
    }
}

/// Drawn over the game, which stays visible underneath.
struct Paused;

impl Game for Paused {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Esc) {
            Next::pop()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        screen.text(0, 1, "PAUSED", Colour::Red);
    }
}

struct GameOver {
    score: u32,
}

impl Game for GameOver {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        screen.text(0, 0, &format!("GAME OVER {}", self.score), Colour::Red);
    }
}

#[test]
fn a_menu_play_pause_resume_and_game_over_flow_runs_headless() {
    let mut game = Headless::new(Menu).size(30, 3);
    assert!(game.text().contains("press Enter"));

    game.tap(KeyCode::Enter);
    game.step(2);
    assert!(game.text().contains("score 3"), "{}", game.text());

    // pause: the game is drawn underneath, and its score stops
    game.tap(KeyCode::Esc);
    let paused = game.text();
    assert!(paused.contains("PAUSED"), "{paused}");
    assert!(paused.contains("score"), "the game stays visible: {paused}");
    game.step(30);
    assert_eq!(game.text(), paused, "nothing moves while paused");

    // resume: the score counts again, up to game over
    game.tap(KeyCode::Esc);
    game.step(10);
    assert!(game.text().contains("GAME OVER 5"), "{}", game.text());
    assert!(!game.quit(), "game over waits for Enter");

    game.tap(KeyCode::Enter);
    assert!(game.quit());
}

#[test]
fn one_glob_import_brings_in_the_drawing_and_art_names() {
    // a compile-time check: these names must all come from `game::*`
    let mut display = Display::new(4, 2);
    let palette = Palette::ocean();
    let pointer = Pointer::default();
    sketch_frame(&mut display, &palette, 0.0, &pointer, |_cell| 0.0);
    let _world: World = World::new();
    let _rng = Rng::new(1);
    let _colour = Colour::Red;
}

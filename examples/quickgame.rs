//! QUICKGAME: steer a `@` with the arrows or WASD and collect the `*` stars
//! for 20 seconds. Enter starts, q quits, Ctrl+C always quits.

use terminal_graphics_interactive::game::*;

fn main() -> std::io::Result<()> {
    run(Title { score: None })
}

/// The title. After a round it shows the score instead.
struct Title {
    score: Option<u32>,
}

impl Game for Title {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Enter) {
            Next::switch(Play::new(ctx.seed))
        } else if ctx.keys.pressed(KeyCode::Char('q')) {
            Next::quit()
        } else {
            Next::stay()
        }
    }

    fn draw(&self, screen: &mut Screen) {
        screen.clear(Colour::Black);
        screen.text_centred(8, "QUICKGAME", Colour::BrightYellow);
        let line = match self.score {
            Some(score) => format!("time up: {score} stars"),
            None => "collect the stars before time runs out".to_string(),
        };
        screen.text_centred(10, &line, Colour::White);
        screen.text_centred(12, "press Enter to play", Colour::BrightWhite);
    }
}

/// One round: steer, collect stars, beat the clock.
struct Play {
    rng: Rng,
    x: f32,
    y: f32,
    score: u32,
    time_left: f32,
    stars: Pool<(i32, i32)>,
}

impl Play {
    fn new(seed: u64) -> Play {
        Play {
            rng: Rng::new(seed),
            x: 20.0,
            y: 8.0,
            score: 0,
            time_left: 20.0,
            stars: Pool::with_capacity(12),
        }
    }
}

impl Game for Play {
    fn update(&mut self, ctx: &mut Context<'_>) -> Next {
        if ctx.keys.pressed(KeyCode::Char('q')) {
            return Next::quit();
        }
        let dx = ctx.keys.axis(KeyCode::Left, KeyCode::Right)
            + ctx.keys.axis(KeyCode::Char('a'), KeyCode::Char('d'));
        let dy = ctx.keys.axis(KeyCode::Up, KeyCode::Down)
            + ctx.keys.axis(KeyCode::Char('w'), KeyCode::Char('s'));
        // rows move half as fast: a cell is about twice as tall as it is wide
        let (cols, rows) = (ctx.size.0 as i32, ctx.size.1 as i32);
        let max_x = (cols - 1).max(0) as f32;
        let max_y = (rows - 1).max(0) as f32;
        self.x = (self.x + dx.clamp(-1, 1) as f32 * 24.0 * ctx.dt).clamp(0.0, max_x);
        self.y = (self.y + dy.clamp(-1, 1) as f32 * 12.0 * ctx.dt).clamp(0.0, max_y);

        let player = (self.x.round() as i32, self.y.round() as i32);
        let before = self.stars.len();
        self.stars.retain(|star| *star != player);
        self.score += (before - self.stars.len()) as u32;
        if ctx.tick % 36 == 0 {
            let star = (self.rng.range_i32(0, cols), self.rng.range_i32(0, rows));
            let _ = self.stars.push(star); // a full pool gives the star back
        }

        self.time_left -= ctx.dt;
        if self.time_left <= 0.0 {
            return Next::switch(Title {
                score: Some(self.score),
            });
        }
        Next::stay()
    }

    fn draw(&self, screen: &mut Screen) {
        screen.clear(Colour::Black);
        for &(x, y) in self.stars.iter() {
            screen.put(x, y, '*', Colour::BrightYellow);
        }
        let (x, y) = (self.x.round() as i32, self.y.round() as i32);
        screen.put(x, y, '@', Colour::BrightCyan);
        let hud = format!("score {}   time {}", self.score, self.time_left.ceil());
        screen.text(0, 0, &hud, Colour::White);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The column and row of the player's `@`, counted in characters.
    fn player_at(text: &str) -> Option<(usize, usize)> {
        text.lines()
            .enumerate()
            .find_map(|(row, line)| line.chars().position(|c| c == '@').map(|col| (col, row)))
    }

    #[test]
    fn the_title_says_how_to_start() {
        let mut game = Headless::new(Title { score: None });
        assert!(game.text().contains("press Enter to play"));
    }

    #[test]
    fn enter_starts_a_round_with_no_score_and_twenty_seconds() {
        let mut game = Headless::new(Title { score: None });
        game.tap(KeyCode::Enter);
        let text = game.text();
        assert!(text.contains("score 0"), "{text}");
        assert!(text.contains("time 20"), "{text}");
    }

    #[test]
    fn a_held_arrow_moves_the_player_right_and_not_down() {
        let mut game = Headless::new(Title { score: None }).size(40, 12);
        game.tap(KeyCode::Enter);
        let before = player_at(&game.text()).expect("the player is drawn");
        game.press(KeyCode::Right);
        game.step(30);
        game.release(KeyCode::Right);
        let after = player_at(&game.text()).expect("the player is drawn");
        assert!(after.0 > before.0, "moved from {before:?} to {after:?}");
        assert_eq!(after.1, before.1, "a sideways key does not move down");
    }

    #[test]
    fn a_star_under_the_player_is_collected() {
        let mut play = Play::new(1);
        let _ = play.stars.push((20, 8)); // exactly where the player starts
        let mut game = Headless::new(play).size(40, 12);
        game.step(1);
        let text = game.text();
        assert!(text.contains("score 1"), "{text}");
    }

    #[test]
    fn the_round_ends_and_the_title_shows_the_score() {
        let mut game = Headless::new(Title { score: None }).size(40, 12);
        game.tap(KeyCode::Enter);
        game.step(20 * 60 + 10);
        let text = game.text();
        assert!(text.contains("time up: "), "{text}");
        assert!(text.contains(" stars"), "{text}");
    }

    #[test]
    fn q_quits_from_the_title_and_during_a_round() {
        let mut game = Headless::new(Title { score: None });
        game.tap(KeyCode::Char('q'));
        assert!(game.quit(), "q quits from the title");

        let mut game = Headless::new(Title { score: None });
        game.tap(KeyCode::Enter);
        game.tap(KeyCode::Char('q'));
        assert!(game.quit(), "q quits during a round");
    }
}

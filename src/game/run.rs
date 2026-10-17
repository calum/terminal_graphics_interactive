//! [`run`]: the game loop on a real terminal.

use super::scene::{time_of, Scenes, STEP};
use super::{Game, Keys, Screen};
use crate::clock::FixedStep;
use crate::{Event, Pointer, Terminal};
use std::io;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The most updates one wake-up may run. If the machine falls further behind,
/// the time is dropped rather than owed, so a slow frame never spirals.
const MAX_CATCH_UP: u32 = 5;

/// Plays `game` in the terminal until it quits, at 60 updates a second.
///
/// It starts the terminal, runs the game, and restores the terminal when it
/// returns. Ctrl+C always quits. `q` and Esc are not special: the game decides
/// what they do, so most games use them. Run it from `main`:
///
/// ```no_run
/// use terminal_graphics_interactive::game::*;
///
/// struct Menu;
/// struct Play { score: u32 }
///
/// impl Game for Menu {
///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
///         if ctx.keys.pressed(KeyCode::Enter) { Next::switch(Play { score: 0 }) } else { Next::stay() }
///     }
///     fn draw(&self, screen: &mut Screen) {
///         screen.clear(Colour::Black);
///         screen.text_centred(screen.height() / 2, "press Enter to play", Colour::White);
///     }
/// }
///
/// impl Game for Play {
///     fn update(&mut self, ctx: &mut Context<'_>) -> Next {
///         if ctx.keys.pressed(KeyCode::Esc) { return Next::quit(); }
///         self.score += 1;
///         Next::stay()
///     }
///     fn draw(&self, screen: &mut Screen) {
///         screen.clear(Colour::Black);
///         screen.text(0, 0, &format!("score {}", self.score), Colour::Yellow);
///     }
/// }
///
/// fn main() -> std::io::Result<()> {
///     run(Menu)
/// }
/// ```
///
/// This needs a real terminal (a pseudo terminal over SSH with `ssh -t`). Its
/// error says so when stdin is not one. To test a game without a terminal, use
/// [`Headless`](super::Headless).
pub fn run(game: impl Game + 'static) -> io::Result<()> {
    let mut terminal = Terminal::start()?;
    terminal.report_mouse_movement()?;
    let real = terminal.enable_key_release()?;
    let mut keys = Keys::new().real_release(real);

    let (width, height) = terminal.size();
    // one row shorter than the window, which `Terminal::draw` needs
    let mut screen = Screen::new(width.max(1), height.saturating_sub(1).max(1));
    let seed = seed_from_clock();
    let mut scenes = Scenes::new(Box::new(game));
    let mut pointer = Pointer::default();
    let mut clock = FixedStep::new(STEP, MAX_CATCH_UP);
    let mut last = Instant::now();
    let mut tick = 0u64;

    loop {
        // sleep until the next update is due, or wake for an input event
        if let Some(event) = terminal.poll(clock.until_next())? {
            match &event {
                Event::Key(key) if key.is_ctrl('c') => return Ok(()),
                Event::Key(_) | Event::KeyUp(_) => keys.event(&event, time_of(tick)),
                Event::Mouse(mouse) => pointer.update(mouse),
                _ => {}
            }
        }
        let now = Instant::now();
        let due = clock.advance(now.duration_since(last));
        last = now;
        if due == 0 {
            continue;
        }
        for _ in 0..due {
            if !scenes.step(&mut keys, &mut pointer, tick, screen.size(), seed) {
                return Ok(());
            }
            tick += 1;
        }
        scenes.draw(&mut screen, tick);
        terminal.draw(screen.display())?;
    }
}

/// A seed from the clock, so every run plays differently. Falls back to 1
/// if the clock is set before 1970.
fn seed_from_clock() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_nanos() as u64,
        Err(_) => 1,
    }
}

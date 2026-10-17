//! What every art script needs and none should write: the frame loop, the
//! pointer, and quitting.

use crate::{Display, Event, KeyCode, Mouse, MouseButton, MouseKind, Terminal};
use std::io;
use std::time::{Duration, Instant};

/// The time between frames: about 30 frames a second.
const FRAME: Duration = Duration::from_millis(33);

/// Where the mouse is and what it is doing, in cells.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pointer {
    /// Column the pointer is over.
    pub x: f32,
    /// Row the pointer is over.
    pub y: f32,
    /// The left button is held.
    pub left: bool,
    /// The right button is held.
    pub right: bool,
    /// Where the left button was pressed since the last frame.
    pub clicks: Vec<(f32, f32)>,
}

impl Pointer {
    /// Takes in one mouse event.
    pub fn update(&mut self, mouse: &Mouse) {
        self.x = mouse.x as f32;
        self.y = mouse.y as f32;
        match mouse.kind {
            MouseKind::Down(MouseButton::Left) => {
                self.left = true;
                self.clicks.push((self.x, self.y));
            }
            MouseKind::Down(MouseButton::Right) => self.right = true,
            MouseKind::Up(MouseButton::Left) => self.left = false,
            MouseKind::Up(MouseButton::Right) => self.right = false,
            // a drag proves the button is down even if the press was missed
            MouseKind::Drag(MouseButton::Left) => self.left = true,
            MouseKind::Drag(MouseButton::Right) => self.right = true,
            _ => {}
        }
    }

    /// Call after each frame: forgets the clicks that frame has seen.
    pub fn end_frame(&mut self) {
        self.clicks.clear();
    }
}

/// Runs `frame(display, seconds, pointer)` about 30 times a second until the
/// user presses q, Esc or Ctrl+C. Starts a [`Terminal`] session, reports mouse
/// movement, and draws the display after every frame.
pub(crate) fn run_loop(mut frame: impl FnMut(&mut Display, f32, &Pointer)) -> io::Result<()> {
    let mut terminal = Terminal::start()?;
    terminal.report_mouse_movement()?;

    let (width, height) = terminal.size();
    // one row shorter than the window, which `Terminal::draw` needs
    let mut display = Display::new(width, height.saturating_sub(1));
    let mut pointer = Pointer::default();
    let start = Instant::now();
    let mut next_frame = start + FRAME;

    loop {
        frame(&mut display, start.elapsed().as_secs_f32(), &pointer);
        pointer.end_frame();
        terminal.draw(&display)?;

        // spend the time until the next frame handling input
        loop {
            let wait = next_frame.saturating_duration_since(Instant::now());
            match terminal.poll(wait)? {
                Some(Event::Key(key)) => {
                    if key.is_char('q') || key.is_ctrl('c') || key.code == KeyCode::Esc {
                        return Ok(());
                    }
                }
                Some(Event::Mouse(mouse)) => pointer.update(&mouse),
                Some(_) => {}
                None => break,
            }
        }
        // if we fell behind, do not try to catch up with a burst of frames
        next_frame = (next_frame + FRAME).max(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Modifiers;

    fn mouse(kind: MouseKind, x: isize, y: isize) -> Mouse {
        Mouse {
            kind,
            x,
            y,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn every_mouse_event_updates_the_position() {
        let mut pointer = Pointer::default();
        for kind in [
            MouseKind::Moved,
            MouseKind::ScrollUp,
            MouseKind::ScrollDown,
            MouseKind::Down(MouseButton::Middle),
            MouseKind::Up(MouseButton::Middle),
        ] {
            pointer.update(&mouse(kind, 12, 7));
            assert_eq!((pointer.x, pointer.y), (12.0, 7.0), "{kind:?}");
        }
    }

    #[test]
    fn the_left_button_is_held_between_press_and_release() {
        let mut pointer = Pointer::default();
        pointer.update(&mouse(MouseKind::Down(MouseButton::Left), 3, 4));
        assert!(pointer.left && !pointer.right);
        pointer.update(&mouse(MouseKind::Up(MouseButton::Left), 3, 4));
        assert!(!pointer.left);
    }

    #[test]
    fn the_right_button_is_tracked_separately() {
        let mut pointer = Pointer::default();
        pointer.update(&mouse(MouseKind::Down(MouseButton::Right), 3, 4));
        assert!(pointer.right && !pointer.left);
        pointer.update(&mouse(MouseKind::Up(MouseButton::Right), 3, 4));
        assert!(!pointer.right);
    }

    #[test]
    fn dragging_means_the_button_is_down_even_if_the_press_was_missed() {
        let mut pointer = Pointer::default();
        pointer.update(&mouse(MouseKind::Drag(MouseButton::Left), 1, 1));
        assert!(pointer.left);
        pointer.update(&mouse(MouseKind::Drag(MouseButton::Right), 1, 1));
        assert!(pointer.right);
    }

    #[test]
    fn a_left_press_records_a_click_and_the_frame_end_forgets_it() {
        let mut pointer = Pointer::default();
        pointer.update(&mouse(MouseKind::Down(MouseButton::Left), 5, 6));
        pointer.update(&mouse(MouseKind::Down(MouseButton::Left), 8, 9));
        assert_eq!(pointer.clicks, vec![(5.0, 6.0), (8.0, 9.0)]);
        pointer.end_frame();
        assert!(pointer.clicks.is_empty());
        assert_eq!((pointer.x, pointer.y), (8.0, 9.0), "position stays");
    }

    #[test]
    fn other_buttons_and_releases_are_not_clicks() {
        let mut pointer = Pointer::default();
        pointer.update(&mouse(MouseKind::Down(MouseButton::Right), 1, 1));
        pointer.update(&mouse(MouseKind::Down(MouseButton::Middle), 1, 1));
        pointer.update(&mouse(MouseKind::Up(MouseButton::Left), 1, 1));
        pointer.update(&mouse(MouseKind::Drag(MouseButton::Left), 1, 1));
        assert!(pointer.clicks.is_empty());
    }
}

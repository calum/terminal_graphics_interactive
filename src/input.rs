//! Turns chunks of bytes arriving on a channel into events, with timeouts.
//!
//! The channel is fed by a reader thread (see `terminal.rs`) because reading
//! stdin blocks. Keeping the timing logic here, away from the thread and the
//! terminal, means it can be unit tested with a plain channel.

use crate::event::Event;
use crate::parser::Parser;
use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

/// How long a lone Escape byte waits for the rest of a sequence before it is
/// treated as the Escape key. Terminals send a whole sequence at once, so this
/// only has to cover a slow SSH link.
pub(crate) const ESCAPE_WAIT: Duration = Duration::from_millis(25);

/// The longest a single poll waits (a day). Longer timeouts are shortened to it.
pub(crate) const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) struct Input {
    chunks: Receiver<Vec<u8>>,
    parser: Parser,
    /// When bytes that could not yet be parsed first arrived.
    pending_since: Option<Instant>,
    /// Events that arrived while waiting for something else (the size reply).
    queue: VecDeque<Event>,
}

impl Input {
    pub(crate) fn new(chunks: Receiver<Vec<u8>>) -> Input {
        Input {
            chunks,
            parser: Parser::new(),
            pending_since: None,
            queue: VecDeque::new(),
        }
    }

    /// Waits up to `timeout` for the terminal's reply to a cursor position
    /// query that the caller has just sent, and returns `(width, height)`
    /// (the cursor was moved to the bottom right corner first). Keys and mouse
    /// events that arrive meanwhile are kept for `poll`.
    pub(crate) fn request_size(&mut self, timeout: Duration) -> Option<(u32, u32)> {
        self.parser.expect_cursor_position();
        let deadline = Instant::now() + timeout.min(MAX_TIMEOUT);
        let size = loop {
            // events that arrive meanwhile are kept for `poll`
            while let Some(event) = self.parser.next_event() {
                self.queue.push_back(event);
            }
            if let Some((row, column)) = self.parser.take_cursor_position() {
                break Some((column, row));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break None;
            }
            match self.chunks.recv_timeout(remaining) {
                Ok(bytes) => {
                    self.parser.feed(&bytes);
                    self.pending_since = None;
                }
                Err(_) => break None,
            }
        };
        self.parser.cancel_cursor_position();
        size
    }

    /// Waits up to `timeout` for the next event. `Ok(None)` means the time ran
    /// out; an error means the input has ended (stdin was closed).
    pub(crate) fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if let Some(event) = self.queue.pop_front() {
            return Ok(Some(event));
        }
        // Instant arithmetic panics on overflow, so cap absurd timeouts
        let deadline = Instant::now() + timeout.min(MAX_TIMEOUT);
        loop {
            // take everything that has already arrived before deciding anything
            let mut closed = false;
            loop {
                match self.chunks.try_recv() {
                    Ok(bytes) => {
                        self.parser.feed(&bytes);
                        self.pending_since = None;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        closed = true;
                        break;
                    }
                }
            }

            if let Some(event) = self.parser.next_event() {
                return Ok(Some(event));
            }
            if closed {
                // nothing more is coming, so a lone Escape is the Escape key
                return match self.parser.flush() {
                    Some(event) => Ok(Some(event)),
                    None => Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "terminal input closed (stdin ended)",
                    )),
                };
            }

            let now = Instant::now();
            let mut wait = deadline.saturating_duration_since(now);
            if self.parser.has_pending() {
                // the start of a sequence: give the rest a moment to arrive
                let settle_at = *self.pending_since.get_or_insert(now) + ESCAPE_WAIT;
                if now >= settle_at {
                    self.pending_since = None;
                    if let Some(event) = self.parser.flush() {
                        return Ok(Some(event));
                    }
                } else {
                    wait = wait.min(settle_at - now);
                }
            } else {
                self.pending_since = None;
            }

            match self.chunks.recv_timeout(wait) {
                Ok(bytes) => {
                    self.parser.feed(&bytes);
                    self.pending_since = None;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                }
                // reported as `closed` by try_recv on the next pass
                Err(RecvTimeoutError::Disconnected) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Key, KeyCode, MouseButton, MouseKind};
    use std::sync::mpsc::{channel, Sender};
    use std::thread::sleep;

    fn input() -> (Sender<Vec<u8>>, Input) {
        let (tx, rx) = channel();
        (tx, Input::new(rx))
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(Key::new(code))
    }

    const SHORT: Duration = Duration::from_millis(5);
    const LONG: Duration = Duration::from_secs(2);

    #[test]
    fn returns_none_when_nothing_arrives_before_the_timeout() {
        let (_tx, mut input) = input();
        let start = Instant::now();
        assert_eq!(input.poll(Duration::from_millis(30)).unwrap(), None);
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn returns_an_event_as_soon_as_its_bytes_arrive() {
        let (tx, mut input) = input();
        tx.send(b"a".to_vec()).unwrap();
        let start = Instant::now();
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Char('a'))));
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn several_events_in_one_chunk_come_out_one_per_poll_in_order() {
        let (tx, mut input) = input();
        tx.send(b"ab\x1b[A".to_vec()).unwrap();
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Char('a'))));
        assert_eq!(input.poll(SHORT).unwrap(), Some(key(KeyCode::Char('b'))));
        assert_eq!(input.poll(SHORT).unwrap(), Some(key(KeyCode::Up)));
        assert_eq!(input.poll(SHORT).unwrap(), None);
    }

    #[test]
    fn a_sequence_split_across_chunks_is_joined() {
        let (tx, mut input) = input();
        tx.send(b"\x1b[<0;3".to_vec()).unwrap();
        tx.send(b";4M".to_vec()).unwrap();
        match input.poll(LONG).unwrap() {
            Some(Event::Mouse(m)) => {
                assert_eq!(m.kind, MouseKind::Down(MouseButton::Left));
                assert_eq!((m.x, m.y), (2, 3));
            }
            other => panic!("expected a mouse event, got {other:?}"),
        }
    }

    #[test]
    fn a_lone_escape_becomes_the_escape_key_after_the_wait() {
        let (tx, mut input) = input();
        tx.send(b"\x1b".to_vec()).unwrap();
        let start = Instant::now();
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Esc)));
        let waited = start.elapsed();
        assert!(waited >= ESCAPE_WAIT, "settled too early: {waited:?}");
        assert!(
            waited < Duration::from_secs(1),
            "waited too long: {waited:?}"
        );
    }

    #[test]
    fn escape_followed_by_the_rest_of_a_sequence_is_not_the_escape_key() {
        let (tx, mut input) = input();
        tx.send(b"\x1b".to_vec()).unwrap();
        assert_eq!(input.poll(Duration::from_millis(1)).unwrap(), None);
        tx.send(b"[A".to_vec()).unwrap();
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Up)));
    }

    #[test]
    fn the_escape_wait_carries_over_between_short_polls() {
        // a game loop polling every few milliseconds must still see Esc
        let (tx, mut input) = input();
        tx.send(b"\x1b".to_vec()).unwrap();
        assert_eq!(input.poll(Duration::ZERO).unwrap(), None);
        sleep(ESCAPE_WAIT * 3);
        assert_eq!(input.poll(Duration::ZERO).unwrap(), Some(key(KeyCode::Esc)));
    }

    #[test]
    fn request_size_returns_width_and_height_from_the_reply() {
        let (tx, mut input) = input();
        tx.send(b"\x1b[24;80R".to_vec()).unwrap();
        assert_eq!(input.request_size(LONG), Some((80, 24)));
    }

    #[test]
    fn request_size_waits_for_a_reply_that_arrives_later() {
        let (tx, mut input) = input();
        let sender = std::thread::spawn(move || {
            sleep(Duration::from_millis(40));
            tx.send(b"\x1b[30;100R".to_vec()).unwrap();
            tx
        });
        assert_eq!(input.request_size(LONG), Some((100, 30)));
        drop(sender.join().unwrap());
    }

    #[test]
    fn request_size_gives_up_after_the_timeout() {
        let (_tx, mut input) = input();
        let start = Instant::now();
        assert_eq!(input.request_size(Duration::from_millis(30)), None);
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn keys_pressed_while_waiting_for_the_size_are_not_lost() {
        let (tx, mut input) = input();
        tx.send(b"ab\x1b[24;80Rc".to_vec()).unwrap();
        assert_eq!(input.request_size(LONG), Some((80, 24)));
        assert_eq!(input.poll(SHORT).unwrap(), Some(key(KeyCode::Char('a'))));
        assert_eq!(input.poll(SHORT).unwrap(), Some(key(KeyCode::Char('b'))));
        assert_eq!(input.poll(SHORT).unwrap(), Some(key(KeyCode::Char('c'))));
        assert_eq!(input.poll(SHORT).unwrap(), None);
    }

    #[test]
    fn after_a_timed_out_request_a_lookalike_key_is_a_key_again() {
        let (tx, mut input) = input();
        assert_eq!(input.request_size(Duration::from_millis(10)), None);
        tx.send(b"\x1b[1;5R".to_vec()).unwrap();
        match input.poll(LONG).unwrap() {
            Some(Event::Key(k)) => assert_eq!(k.code, KeyCode::F(3)),
            other => panic!("expected Ctrl+F3, got {other:?}"),
        }
    }

    #[test]
    fn request_size_on_closed_input_returns_none() {
        let (tx, mut input) = input();
        drop(tx);
        assert_eq!(input.request_size(LONG), None);
    }

    #[test]
    fn an_enormous_timeout_does_not_overflow_or_panic() {
        let (tx, mut input) = input();
        tx.send(b"a".to_vec()).unwrap();
        assert_eq!(
            input.poll(Duration::MAX).unwrap(),
            Some(key(KeyCode::Char('a')))
        );
    }

    #[test]
    fn polling_with_a_zero_timeout_does_not_block() {
        let (_tx, mut input) = input();
        let start = Instant::now();
        assert_eq!(input.poll(Duration::ZERO).unwrap(), None);
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn closed_input_still_delivers_what_was_sent_then_reports_the_end() {
        let (tx, mut input) = input();
        tx.send(b"a".to_vec()).unwrap();
        drop(tx);
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Char('a'))));
        let error = input.poll(LONG).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn a_pending_escape_is_delivered_when_the_input_closes() {
        let (tx, mut input) = input();
        tx.send(b"\x1b".to_vec()).unwrap();
        drop(tx);
        assert_eq!(input.poll(LONG).unwrap(), Some(key(KeyCode::Esc)));
        assert!(input.poll(LONG).is_err());
    }
}

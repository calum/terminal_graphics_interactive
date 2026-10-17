//! Turns chunks of bytes arriving on a channel into events, with timeouts.
//!
//! The channel is fed by a reader thread (see `terminal.rs`) because reading
//! stdin blocks. Keeping the timing logic here, away from the thread and the
//! terminal, means it can be unit tested with a plain channel.

use crate::event::Event;
use crate::parser::Parser;
use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

/// How long a lone Escape byte waits for the rest of a sequence before it is
/// treated as the Escape key. Terminals send a whole sequence at once, so this
/// only has to cover a slow SSH link.
pub(crate) const ESCAPE_WAIT: Duration = Duration::from_millis(25);

pub(crate) struct Input {
    chunks: Receiver<Vec<u8>>,
    parser: Parser,
    /// When bytes that could not yet be parsed first arrived.
    pending_since: Option<Instant>,
}

impl Input {
    pub(crate) fn new(chunks: Receiver<Vec<u8>>) -> Input {
        Input {
            chunks,
            parser: Parser::new(),
            pending_since: None,
        }
    }

    /// Waits up to `timeout` for the next event. `Ok(None)` means the time ran
    /// out; an error means the input has ended (stdin was closed).
    pub(crate) fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        let deadline = Instant::now() + timeout;
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

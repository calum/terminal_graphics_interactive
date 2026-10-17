//! Turns the bytes a terminal sends on stdin into [`Event`]s.

use crate::event::{Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};

const ESC: u8 = 0x1b;

/// Longest CSI sequence we wait for before giving up on it as garbage.
const MAX_CSI_LEN: usize = 64;

/// The result of looking at the front of the buffer.
enum Step {
    /// An event, made from this many bytes.
    Event(Event, usize),
    /// This many bytes are not an event; drop them.
    Skip(usize),
    /// More bytes are needed to decide.
    Incomplete,
}

/// An incremental parser for terminal input.
///
/// Feed it bytes as they arrive with [`Parser::feed`], then pull events out
/// with [`Parser::next_event`]. A sequence split across two reads is handled:
/// the parser waits for the rest. Unknown or malformed sequences are skipped
/// and never panic or block later input.
#[derive(Debug, Default)]
pub struct Parser {
    buffer: Vec<u8>,
    /// Bytes before this index are already parsed. Moving an index is O(1); removing
    /// them from the front of the buffer after every event would make a large
    /// paste quadratic.
    start: usize,
    /// A cursor position report is on its way (see `expect_cursor_position`).
    expect_report: bool,
    /// The last report received, until it is taken.
    report: Option<(u32, u32)>,
}

impl Parser {
    /// Creates an empty parser.
    pub fn new() -> Parser {
        Parser::default()
    }

    /// Adds input bytes to the parser.
    pub fn feed(&mut self, bytes: &[u8]) {
        // drop what has been parsed, once per feed, so the buffer stays small
        if self.start > 0 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
        self.buffer.extend_from_slice(bytes);
    }

    /// The next complete event, or `None` if more bytes are needed.
    pub fn next_event(&mut self) -> Option<Event> {
        loop {
            let front = &self.buffer[self.start..];
            if self.expect_report {
                if let Some((used, row, column)) = parse_cursor_report(front) {
                    self.report = Some((row, column));
                    self.expect_report = false;
                    self.start += used;
                    continue;
                }
            }
            match parse_front(front) {
                Step::Event(event, used) => {
                    self.start += used;
                    return Some(event);
                }
                Step::Skip(used) => self.start += used,
                Step::Incomplete => return None,
            }
        }
    }

    /// Tells the parser that a cursor position report (`ESC [ row ; col R`) is
    /// on its way, in answer to a `ESC [ 6 n` query. The next such sequence is
    /// then kept for [`Parser::take_cursor_position`] instead of being read as
    /// a key, because it is otherwise indistinguishable from Ctrl+F3.
    pub fn expect_cursor_position(&mut self) {
        self.expect_report = true;
    }

    /// The `(row, column)` of the last cursor position report, counted from 1,
    /// or `None` if none has arrived since this was last called.
    pub fn take_cursor_position(&mut self) -> Option<(u32, u32)> {
        self.report.take()
    }

    /// Stops expecting a cursor position report (the query got no answer).
    pub(crate) fn cancel_cursor_position(&mut self) {
        self.expect_report = false;
    }

    /// Whether bytes are waiting for the rest of an escape sequence. After
    /// [`Parser::next_event`] returns `None`, this is `true` if more input could
    /// still complete an event, so the caller should wait a moment and then
    /// call [`Parser::flush`].
    pub fn has_pending(&self) -> bool {
        self.start < self.buffer.len()
    }

    /// Call when no more bytes are coming for a moment (for example after a
    /// short read timeout). A lone Escape byte could be the Escape key or the
    /// start of a longer sequence; this settles it as the Escape key.
    pub fn flush(&mut self) -> Option<Event> {
        if self.buffer[self.start..] == [ESC] {
            self.start += 1;
            return Some(Event::Key(Key::new(KeyCode::Esc)));
        }
        None
    }
}

/// `ESC [ row ; column R` at the front of `buf`: the bytes used, the row and the column.
fn parse_cursor_report(buf: &[u8]) -> Option<(usize, u32, u32)> {
    let rest = buf.strip_prefix(&[ESC, b'['])?;
    let end = rest.iter().position(|&b| b == b'R')?;
    let (row, column) = std::str::from_utf8(&rest[..end]).ok()?.split_once(';')?;
    Some((2 + end + 1, row.parse().ok()?, column.parse().ok()?))
}

fn parse_front(buf: &[u8]) -> Step {
    match buf {
        [] => Step::Incomplete,
        [ESC] => Step::Incomplete,
        [ESC, b'[', rest @ ..] => parse_csi(rest),
        [ESC, b'O', rest @ ..] => parse_ss3(rest),
        // two escapes in a row: the first is the Escape key
        [ESC, ESC, ..] => Step::Event(Event::Key(Key::new(KeyCode::Esc)), 1),
        // alt plus a key arrives as escape then the key
        [ESC, rest @ ..] => match parse_char(rest) {
            Step::Event(Event::Key(mut key), used) => {
                key.modifiers.alt = true;
                Step::Event(Event::Key(key), used + 1)
            }
            Step::Event(event, used) => Step::Event(event, used + 1),
            Step::Skip(used) => Step::Skip(used + 1),
            Step::Incomplete => Step::Incomplete,
        },
        _ => parse_char(buf),
    }
}

fn key_event(code: KeyCode, modifiers: Modifiers, used: usize) -> Step {
    Step::Event(Event::Key(Key { code, modifiers }), used)
}

/// A plain character or control key at the front of `buf` (not an escape).
fn parse_char(buf: &[u8]) -> Step {
    let byte = buf[0];
    let none = Modifiers::NONE;
    match byte {
        b'\r' | b'\n' => key_event(KeyCode::Enter, none, 1),
        b'\t' => key_event(KeyCode::Tab, none, 1),
        0x7f | 0x08 => key_event(KeyCode::Backspace, none, 1),
        // Ctrl+A .. Ctrl+Z
        0x01..=0x1a => {
            let letter = (b'a' + byte - 1) as char;
            let modifiers = Modifiers { ctrl: true, ..none };
            key_event(KeyCode::Char(letter), modifiers, 1)
        }
        0x20..=0x7e => key_event(KeyCode::Char(byte as char), none, 1),
        0xc2..=0xf4 => {
            let needed = match byte {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };
            if buf.len() < needed {
                return Step::Incomplete;
            }
            match std::str::from_utf8(&buf[..needed])
                .ok()
                .and_then(|s| s.chars().next())
            {
                Some(c) => key_event(KeyCode::Char(c), none, needed),
                None => Step::Skip(1),
            }
        }
        // NUL, other control bytes, stray UTF-8 continuation bytes
        _ => Step::Skip(1),
    }
}

/// Decodes the modifier parameter of a key sequence (1 + shift/alt/ctrl bits).
fn modifiers_from_param(param: Option<u32>) -> Modifiers {
    let bits = param.unwrap_or(1).saturating_sub(1);
    Modifiers {
        shift: bits & 1 != 0,
        alt: bits & 2 != 0,
        ctrl: bits & 4 != 0,
    }
}

/// `ESC O x`: arrows and F1 to F4 in application mode. `rest` follows `ESC O`.
fn parse_ss3(rest: &[u8]) -> Step {
    let Some(&final_byte) = rest.first() else {
        return Step::Incomplete;
    };
    let code = match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P'..=b'S' => KeyCode::F(final_byte - b'P' + 1),
        _ => return Step::Skip(3),
    };
    key_event(code, Modifiers::NONE, 3)
}

/// `ESC [ ...`. `rest` follows `ESC [`.
fn parse_csi(rest: &[u8]) -> Step {
    // parameter bytes 0x30-0x3f, intermediate bytes 0x20-0x2f, then a final byte
    let mut end = None;
    for (i, &b) in rest.iter().enumerate() {
        match b {
            0x20..=0x3f => {}
            0x40..=0x7e => {
                end = Some(i);
                break;
            }
            // not part of a CSI sequence: abandon it, keep this byte
            _ => return Step::Skip(2 + i),
        }
    }
    let Some(end) = end else {
        return if rest.len() > MAX_CSI_LEN {
            Step::Skip(2)
        } else {
            Step::Incomplete
        };
    };
    let used = 2 + end + 1;
    let params = &rest[..end];
    let final_byte = rest[end];

    if params.first() == Some(&b'<') && (final_byte == b'M' || final_byte == b'm') {
        return match parse_sgr_mouse(&params[1..], final_byte == b'M') {
            Some(mouse) => Step::Event(Event::Mouse(mouse), used),
            None => Step::Skip(used),
        };
    }

    // only the first two parameters matter for keys: the key, then the modifiers
    let mut numbers = [None; 2];
    for (slot, part) in numbers.iter_mut().zip(params.split(|&b| b == b';')) {
        *slot = std::str::from_utf8(part).ok().and_then(|s| s.parse().ok());
    }
    let first = numbers[0];
    let modifiers = |index: usize| modifiers_from_param(numbers[index]);

    let step = match final_byte {
        b'A' => Some((KeyCode::Up, modifiers(1))),
        b'B' => Some((KeyCode::Down, modifiers(1))),
        b'C' => Some((KeyCode::Right, modifiers(1))),
        b'D' => Some((KeyCode::Left, modifiers(1))),
        b'H' => Some((KeyCode::Home, modifiers(1))),
        b'F' => Some((KeyCode::End, modifiers(1))),
        b'P'..=b'S' => Some((KeyCode::F(final_byte - b'P' + 1), modifiers(1))),
        b'Z' => Some((
            KeyCode::Tab,
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        )),
        b'~' => first.and_then(tilde_key).map(|code| (code, modifiers(1))),
        _ => None,
    };
    match step {
        Some((code, modifiers)) => key_event(code, modifiers, used),
        None => Step::Skip(used),
    }
}

/// The key for `ESC [ n ~`.
fn tilde_key(n: u32) -> Option<KeyCode> {
    Some(match n {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F((n - 10) as u8),
        17..=21 => KeyCode::F((n - 11) as u8),
        23 | 24 => KeyCode::F((n - 12) as u8),
        _ => return None,
    })
}

/// `params` is what follows `ESC [ <`, for example `0;12;5`.
fn parse_sgr_mouse(params: &[u8], pressed: bool) -> Option<Mouse> {
    let text = std::str::from_utf8(params).ok()?;
    let mut parts = text.split(';');
    let code: u32 = parts.next()?.parse().ok()?;
    let x: isize = parts.next()?.parse().ok()?;
    let y: isize = parts.next()?.parse().ok()?;
    if parts.next().is_some() || x < 1 || y < 1 {
        return None;
    }

    let button = match code & 3 {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        _ => None,
    };
    let modifiers = Modifiers {
        shift: code & 4 != 0,
        alt: code & 8 != 0,
        ctrl: code & 16 != 0,
    };
    let kind = if code & 64 != 0 {
        // wheel: 0 up, 1 down; sideways scrolling is ignored
        match (code & 3, pressed) {
            (0, true) => MouseKind::ScrollUp,
            (1, true) => MouseKind::ScrollDown,
            _ => return None,
        }
    } else if code & 32 != 0 {
        match button {
            Some(button) => MouseKind::Drag(button),
            None => MouseKind::Moved,
        }
    } else if pressed {
        MouseKind::Down(button?)
    } else {
        MouseKind::Up(button?)
    };

    Some(Mouse {
        kind,
        x: x - 1,
        y: y - 1,
        modifiers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};

    fn parse(input: &[u8]) -> Vec<Event> {
        let mut p = Parser::new();
        p.feed(input);
        let mut events = Vec::new();
        while let Some(e) = p.next_event() {
            events.push(e);
        }
        events
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(Key::new(code))
    }

    fn key_mod(code: KeyCode, shift: bool, alt: bool, ctrl: bool) -> Event {
        Event::Key(Key {
            code,
            modifiers: Modifiers { shift, alt, ctrl },
        })
    }

    fn mouse(kind: MouseKind, x: isize, y: isize) -> Event {
        Event::Mouse(Mouse {
            kind,
            x,
            y,
            modifiers: Modifiers::NONE,
        })
    }

    #[test]
    fn plain_characters() {
        assert_eq!(
            parse(b"ab"),
            vec![key(KeyCode::Char('a')), key(KeyCode::Char('b'))]
        );
    }

    #[test]
    fn utf8_characters() {
        assert_eq!(
            parse("é€".as_bytes()),
            vec![key(KeyCode::Char('é')), key(KeyCode::Char('€'))]
        );
    }

    #[test]
    fn utf8_split_across_feeds_waits_for_the_rest() {
        let bytes = "€".as_bytes();
        let mut p = Parser::new();
        p.feed(&bytes[..2]);
        assert_eq!(p.next_event(), None);
        p.feed(&bytes[2..]);
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('€'))));
    }

    #[test]
    fn control_keys() {
        assert_eq!(parse(b"\r"), vec![key(KeyCode::Enter)]);
        assert_eq!(parse(b"\n"), vec![key(KeyCode::Enter)]);
        assert_eq!(parse(b"\t"), vec![key(KeyCode::Tab)]);
        assert_eq!(parse(b"\x7f"), vec![key(KeyCode::Backspace)]);
        assert_eq!(parse(b"\x08"), vec![key(KeyCode::Backspace)]);
    }

    #[test]
    fn ctrl_letters_are_the_letter_with_ctrl() {
        assert_eq!(
            parse(b"\x03"),
            vec![key_mod(KeyCode::Char('c'), false, false, true)]
        );
        assert_eq!(
            parse(b"\x01"),
            vec![key_mod(KeyCode::Char('a'), false, false, true)]
        );
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(parse(b"\x1b[A"), vec![key(KeyCode::Up)]);
        assert_eq!(parse(b"\x1b[B"), vec![key(KeyCode::Down)]);
        assert_eq!(parse(b"\x1b[C"), vec![key(KeyCode::Right)]);
        assert_eq!(parse(b"\x1b[D"), vec![key(KeyCode::Left)]);
        assert_eq!(parse(b"\x1bOA"), vec![key(KeyCode::Up)]);
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(parse(b"\x1b[H"), vec![key(KeyCode::Home)]);
        assert_eq!(parse(b"\x1b[F"), vec![key(KeyCode::End)]);
        assert_eq!(parse(b"\x1b[2~"), vec![key(KeyCode::Insert)]);
        assert_eq!(parse(b"\x1b[3~"), vec![key(KeyCode::Delete)]);
        assert_eq!(parse(b"\x1b[5~"), vec![key(KeyCode::PageUp)]);
        assert_eq!(parse(b"\x1b[6~"), vec![key(KeyCode::PageDown)]);
    }

    #[test]
    fn function_keys() {
        assert_eq!(parse(b"\x1bOP"), vec![key(KeyCode::F(1))]);
        assert_eq!(parse(b"\x1bOS"), vec![key(KeyCode::F(4))]);
        assert_eq!(parse(b"\x1b[15~"), vec![key(KeyCode::F(5))]);
        assert_eq!(parse(b"\x1b[24~"), vec![key(KeyCode::F(12))]);
    }

    #[test]
    fn modifier_parameters_on_arrow_keys() {
        // 2 = shift, 3 = alt, 5 = ctrl, 6 = ctrl+shift
        assert_eq!(
            parse(b"\x1b[1;2A"),
            vec![key_mod(KeyCode::Up, true, false, false)]
        );
        assert_eq!(
            parse(b"\x1b[1;3D"),
            vec![key_mod(KeyCode::Left, false, true, false)]
        );
        assert_eq!(
            parse(b"\x1b[1;5C"),
            vec![key_mod(KeyCode::Right, false, false, true)]
        );
        assert_eq!(
            parse(b"\x1b[1;6B"),
            vec![key_mod(KeyCode::Down, true, false, true)]
        );
        assert_eq!(
            parse(b"\x1b[3;5~"),
            vec![key_mod(KeyCode::Delete, false, false, true)]
        );
    }

    #[test]
    fn alt_plus_character_is_escape_then_character() {
        assert_eq!(
            parse(b"\x1bx"),
            vec![key_mod(KeyCode::Char('x'), false, true, false)]
        );
    }

    #[test]
    fn lone_escape_waits_until_flushed() {
        let mut p = Parser::new();
        p.feed(b"\x1b");
        assert_eq!(p.next_event(), None);
        assert_eq!(p.flush(), Some(key(KeyCode::Esc)));
        assert_eq!(p.next_event(), None);
    }

    #[test]
    fn sequence_split_across_feeds_waits_for_the_rest() {
        let mut p = Parser::new();
        p.feed(b"\x1b[");
        assert_eq!(p.next_event(), None);
        p.feed(b"A");
        assert_eq!(p.next_event(), Some(key(KeyCode::Up)));
    }

    #[test]
    fn sgr_mouse_press_and_release_are_zero_based() {
        assert_eq!(
            parse(b"\x1b[<0;12;5M"),
            vec![mouse(MouseKind::Down(MouseButton::Left), 11, 4)]
        );
        assert_eq!(
            parse(b"\x1b[<0;12;5m"),
            vec![mouse(MouseKind::Up(MouseButton::Left), 11, 4)]
        );
        assert_eq!(
            parse(b"\x1b[<1;1;1M"),
            vec![mouse(MouseKind::Down(MouseButton::Middle), 0, 0)]
        );
        assert_eq!(
            parse(b"\x1b[<2;3;4M"),
            vec![mouse(MouseKind::Down(MouseButton::Right), 2, 3)]
        );
    }

    #[test]
    fn sgr_mouse_drag_move_and_scroll() {
        assert_eq!(
            parse(b"\x1b[<32;10;10M"),
            vec![mouse(MouseKind::Drag(MouseButton::Left), 9, 9)]
        );
        assert_eq!(
            parse(b"\x1b[<35;10;10M"),
            vec![mouse(MouseKind::Moved, 9, 9)]
        );
        assert_eq!(
            parse(b"\x1b[<64;10;10M"),
            vec![mouse(MouseKind::ScrollUp, 9, 9)]
        );
        assert_eq!(
            parse(b"\x1b[<65;10;10M"),
            vec![mouse(MouseKind::ScrollDown, 9, 9)]
        );
    }

    #[test]
    fn sgr_mouse_modifiers() {
        // 4 = shift, 8 = alt, 16 = ctrl
        assert_eq!(
            parse(b"\x1b[<16;2;2M"),
            vec![Event::Mouse(Mouse {
                kind: MouseKind::Down(MouseButton::Left),
                x: 1,
                y: 1,
                modifiers: Modifiers {
                    shift: false,
                    alt: false,
                    ctrl: true
                },
            })]
        );
    }

    #[test]
    fn sgr_mouse_handles_large_coordinates() {
        assert_eq!(
            parse(b"\x1b[<0;300;250M"),
            vec![mouse(MouseKind::Down(MouseButton::Left), 299, 249)]
        );
    }

    #[test]
    fn events_in_one_buffer_come_out_in_order() {
        assert_eq!(
            parse(b"a\x1b[A\x1b[<0;1;1Mb"),
            vec![
                key(KeyCode::Char('a')),
                key(KeyCode::Up),
                mouse(MouseKind::Down(MouseButton::Left), 0, 0),
                key(KeyCode::Char('b')),
            ]
        );
    }

    #[test]
    fn unknown_sequences_are_skipped_without_blocking_later_input() {
        assert_eq!(parse(b"\x1b[999zq"), vec![key(KeyCode::Char('q'))]);
        assert_eq!(parse(b"\x1b[<9;9;9yq"), vec![key(KeyCode::Char('q'))]);
    }

    #[test]
    fn has_pending_is_true_only_while_bytes_wait_for_the_rest_of_a_sequence() {
        let mut p = Parser::new();
        assert!(!p.has_pending());
        p.feed(b"\x1b[");
        assert_eq!(p.next_event(), None);
        assert!(p.has_pending());
        p.feed(b"A");
        assert!(p.next_event().is_some());
        assert!(!p.has_pending());
    }

    #[test]
    fn a_cursor_position_like_sequence_is_ctrl_f3_unless_a_report_was_expected() {
        assert_eq!(
            parse(b"\x1b[1;5R"),
            vec![key_mod(KeyCode::F(3), false, false, true)]
        );
    }

    #[test]
    fn an_expected_cursor_position_report_is_taken_not_returned_as_an_event() {
        let mut p = Parser::new();
        p.expect_cursor_position();
        p.feed(b"\x1b[24;80R");
        assert_eq!(p.next_event(), None);
        assert_eq!(p.take_cursor_position(), Some((24, 80)));
        assert_eq!(p.take_cursor_position(), None, "taking clears it");
    }

    #[test]
    fn only_one_report_is_swallowed_per_expectation() {
        let mut p = Parser::new();
        p.expect_cursor_position();
        p.feed(b"\x1b[24;80R\x1b[1;5R");
        assert_eq!(
            p.next_event(),
            Some(key_mod(KeyCode::F(3), false, false, true))
        );
        assert_eq!(p.take_cursor_position(), Some((24, 80)));
    }

    #[test]
    fn a_report_split_across_feeds_and_surrounded_by_keys_is_handled() {
        let mut p = Parser::new();
        p.expect_cursor_position();
        p.feed(b"a\x1b[24");
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('a'))));
        assert_eq!(p.next_event(), None);
        p.feed(b";80Rb");
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('b'))));
        assert_eq!(p.take_cursor_position(), Some((24, 80)));
    }

    #[test]
    fn other_input_while_a_report_is_expected_is_unaffected() {
        let mut p = Parser::new();
        p.expect_cursor_position();
        p.feed(b"\x1b[A\x1b[<0;1;1M");
        assert_eq!(p.next_event(), Some(key(KeyCode::Up)));
        assert!(matches!(p.next_event(), Some(Event::Mouse(_))));
        assert_eq!(p.take_cursor_position(), None);
    }

    #[test]
    fn a_large_paste_parses_every_event_in_order() {
        let mut input = Vec::new();
        for i in 0..50_000usize {
            input.extend_from_slice(format!("\x1b[<32;{};{}M", 1 + i % 200, 1 + i % 60).as_bytes());
            input.push(b'a' + (i % 26) as u8);
        }
        let events = parse(&input);
        assert_eq!(events.len(), 100_000);
        assert_eq!(events[0], mouse(MouseKind::Drag(MouseButton::Left), 0, 0));
        assert_eq!(events[1], key(KeyCode::Char('a')));
        assert_eq!(
            events[99_998],
            mouse(
                MouseKind::Drag(MouseButton::Left),
                49_999 % 200,
                49_999 % 60
            )
        );
    }

    #[test]
    fn events_after_a_partial_sequence_in_a_later_feed_are_not_lost() {
        let mut p = Parser::new();
        p.feed(b"ab\x1b[<0;1");
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('a'))));
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('b'))));
        assert_eq!(p.next_event(), None);
        p.feed(b";1Mc");
        assert_eq!(
            p.next_event(),
            Some(mouse(MouseKind::Down(MouseButton::Left), 0, 0))
        );
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('c'))));
        assert_eq!(p.next_event(), None);
    }

    #[test]
    fn garbage_never_panics() {
        for len in 0..4 {
            for a in 0..=255u8 {
                for b in [0u8, b'[', b'<', b';', b'M', 0x1b, 0xff] {
                    let mut input = vec![0x1b, b'['];
                    input.extend(std::iter::repeat(a).take(len));
                    input.push(b);
                    let mut p = Parser::new();
                    p.feed(&input);
                    while p.next_event().is_some() {}
                    p.flush();
                }
            }
        }
    }
}

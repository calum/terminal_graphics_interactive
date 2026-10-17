//! Turns the bytes a terminal sends on stdin into [`Event`]s.

use crate::event::{Event, Key, KeyCode, Modifiers, Mouse, MouseButton, MouseKind};

const ESC: u8 = 0x1b;

/// Sent by a terminal around pasted text once bracketed paste is turned on.
const PASTE_START: &[u8] = b"[200~";
const PASTE_END: &[u8] = b"[201~";

/// The most pasted text kept. Anything beyond is dropped, so pasting a huge
/// file cannot use unlimited memory.
const MAX_PASTE: usize = 1 << 20;

/// Longest CSI sequence we wait for before giving up on it as garbage.
const MAX_CSI_LEN: usize = 64;

/// The result of looking at the front of the buffer.
enum Step {
    /// An event, made from this many bytes.
    Event(Event, usize),
    /// This many bytes are not an event; drop them.
    Skip(usize),
    /// The terminal answered one of our queries; this is not an event.
    Reply(Reply, usize),
    /// More bytes are needed to decide.
    Incomplete,
}

/// A terminal's answer to a query the program sent.
enum Reply {
    /// The kitty keyboard flags in force (`ESC [ ? flags u`).
    KeyboardFlags(u32),
    /// A device attributes reply (`ESC [ ? ... c`).
    Attributes,
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
    /// The text of a paste whose end has not arrived yet.
    paste: Option<Vec<u8>>,
    /// The keyboard flags from the terminal's last reply, until taken.
    keyboard_flags: Option<u32>,
    /// Whether a device attributes reply has arrived, until taken.
    attributes_reply: bool,
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

            if let Some(collected) = &mut self.paste {
                match find(front, PASTE_END) {
                    Some(at) => {
                        keep(collected, &front[..at]);
                        self.start += at + PASTE_END.len();
                        let text = String::from_utf8_lossy(collected).into_owned();
                        self.paste = None;
                        return Some(Event::Paste(text));
                    }
                    None => {
                        // the end marker may be split across reads, so hold
                        // back as many bytes as it could have started in
                        let take = front.len().saturating_sub(PASTE_END.len() - 1);
                        keep(collected, &front[..take]);
                        self.start += take;
                        return None;
                    }
                }
            }
            if front.starts_with(PASTE_START) {
                self.start += PASTE_START.len();
                self.paste = Some(Vec::new());
                continue;
            }

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
                Step::Reply(reply, used) => {
                    self.start += used;
                    match reply {
                        Reply::KeyboardFlags(flags) => self.keyboard_flags = Some(flags),
                        Reply::Attributes => self.attributes_reply = true,
                    }
                }
                Step::Skip(used) => self.start += used,
                Step::Incomplete => return None,
            }
        }
    }

    /// The keyboard flags from the terminal's reply to a kitty keyboard
    /// protocol query (`ESC [ ? u`), or `None` if none has arrived since this
    /// was last called. The reply is never returned as an event.
    pub(crate) fn take_keyboard_flags(&mut self) -> Option<u32> {
        self.keyboard_flags.take()
    }

    /// Whether a device attributes reply (`ESC [ ? ... c`) has arrived since
    /// this was last called. The reply is never returned as an event.
    pub(crate) fn take_attributes_reply(&mut self) -> bool {
        std::mem::take(&mut self.attributes_reply)
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
        self.start < self.buffer.len() || self.paste.is_some()
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

/// Where `needle` first occurs in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Adds `bytes` to the pasted text, up to the limit.
fn keep(collected: &mut Vec<u8>, bytes: &[u8]) {
    let room = MAX_PASTE.saturating_sub(collected.len());
    collected.extend_from_slice(&bytes[..bytes.len().min(room)]);
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
            Step::Reply(reply, used) => Step::Reply(reply, used + 1),
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

    // a reply to one of our queries, not a key: `ESC [ ? flags u` or `ESC [ ? ... c`
    if params.first() == Some(&b'?') {
        return parse_reply(&params[1..], final_byte, used);
    }

    if final_byte == b'u' {
        return parse_kitty(params, used);
    }

    // only the first two parameters matter for keys: the key, then the modifiers
    // and event type (`modifiers:event`)
    let mut numbers = [(None, None); 2];
    for (slot, part) in numbers.iter_mut().zip(params.split(|&b| b == b';')) {
        *slot = values(part);
    }
    let first = numbers[0].0;
    let modifiers = |index: usize| modifiers_from_param(numbers[index].0);
    let event = released(numbers[1].1);

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
    match (step, event) {
        (Some((code, modifiers)), Some(released)) => key_step(code, modifiers, released, used),
        _ => Step::Skip(used),
    }
}

/// `ESC [ ? ...` replies. `params` is what follows the `?`.
fn parse_reply(params: &[u8], final_byte: u8, used: usize) -> Step {
    match (final_byte, number(params)) {
        (b'u', Some(flags)) => Step::Reply(Reply::KeyboardFlags(flags), used),
        (b'c', _) => Step::Reply(Reply::Attributes, used),
        _ => Step::Skip(used),
    }
}

/// `ESC [ code ; modifiers:event u`, the kitty keyboard protocol. `params` is
/// what comes before the `u`.
fn parse_kitty(params: &[u8], used: usize) -> Step {
    let mut fields = params.split(|&b| b == b';');
    let code = fields.next().and_then(|field| values(field).0);
    // the modifiers field is absent or `modifiers:event`; a missing number is malformed
    let (modifiers, event) = match fields.next().map(values) {
        None => (None, None),
        Some((None, _)) => return Step::Skip(used),
        Some(pair) => pair,
    };
    // one more field may follow, the text, which is ignored
    if fields.count() > 1 {
        return Step::Skip(used);
    }
    match (code.and_then(kitty_code), released(event)) {
        (Some(code), Some(released)) => {
            key_step(code, modifiers_from_param(modifiers), released, used)
        }
        _ => Step::Skip(used),
    }
}

/// The key for a kitty code point. Keypad and media keys live in the
/// private-use range and are not supported, so they give `None`.
fn kitty_code(code: u32) -> Option<KeyCode> {
    Some(match code {
        27 => KeyCode::Esc,
        13 => KeyCode::Enter,
        9 => KeyCode::Tab,
        127 => KeyCode::Backspace,
        57344..=63743 => return None,
        _ => KeyCode::Char(char::from_u32(code)?),
    })
}

/// Whether a key sequence's event type (`1` press, `2` repeat, `3` release)
/// means the key was released. An absent event type is a press.
fn released(event: Option<u32>) -> Option<bool> {
    match event {
        None | Some(1) | Some(2) => Some(false),
        Some(3) => Some(true),
        _ => None,
    }
}

/// A key press, repeat or release, as its event.
fn key_step(code: KeyCode, modifiers: Modifiers, released: bool, used: usize) -> Step {
    let key = Key { code, modifiers };
    let event = if released {
        Event::KeyUp(key)
    } else {
        Event::Key(key)
    };
    Step::Event(event, used)
}

/// The number before and the number after the first `:` in a parameter such
/// as `1:3`. A part that is not a number is `None`.
fn values(part: &[u8]) -> (Option<u32>, Option<u32>) {
    let mut subs = part.split(|&b| b == b':').map(number);
    (subs.next().flatten(), subs.next().flatten())
}

fn number(part: &[u8]) -> Option<u32> {
    std::str::from_utf8(part).ok()?.parse().ok()
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

    fn key_up(code: KeyCode) -> Event {
        Event::KeyUp(Key::new(code))
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

    fn paste(text: &str) -> Event {
        Event::Paste(text.to_string())
    }

    #[test]
    fn bracketed_paste_is_one_paste_event_not_key_presses() {
        assert_eq!(
            parse(b"\x1b[200~cargo run\x1b[201~"),
            vec![paste("cargo run")]
        );
    }

    #[test]
    fn a_paste_keeps_newlines_escapes_and_unicode_exactly() {
        let text = "a\x1b[Ab\r\nc€\t\x1b";
        let mut input = b"\x1b[200~".to_vec();
        input.extend_from_slice(text.as_bytes());
        input.extend_from_slice(b"\x1b[201~");
        assert_eq!(parse(&input), vec![paste(text)]);
    }

    #[test]
    fn an_empty_paste_is_an_empty_paste_event() {
        assert_eq!(parse(b"\x1b[200~\x1b[201~"), vec![paste("")]);
    }

    #[test]
    fn keys_around_a_paste_are_still_keys_and_stay_in_order() {
        assert_eq!(
            parse(b"a\x1b[200~xyz\x1b[201~b"),
            vec![
                key(KeyCode::Char('a')),
                paste("xyz"),
                key(KeyCode::Char('b'))
            ]
        );
    }

    #[test]
    fn a_paste_split_across_feeds_waits_for_its_end() {
        let mut p = Parser::new();
        p.feed(b"\x1b[20");
        assert_eq!(p.next_event(), None);
        p.feed(b"0~hel");
        assert_eq!(p.next_event(), None);
        assert!(p.has_pending(), "inside a paste");
        p.feed(b"lo\x1b[2");
        assert_eq!(p.next_event(), None);
        p.feed(b"01~!");
        assert_eq!(p.next_event(), Some(paste("hello")));
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('!'))));
        assert!(!p.has_pending());
    }

    #[test]
    fn the_end_marker_split_at_every_possible_point_still_ends_the_paste() {
        let end = b"\x1b[201~";
        for split in 1..end.len() {
            let mut p = Parser::new();
            p.feed(b"\x1b[200~ab");
            p.feed(&end[..split]);
            assert_eq!(p.next_event(), None, "split at {split}");
            p.feed(&end[split..]);
            assert_eq!(p.next_event(), Some(paste("ab")), "split at {split}");
        }
    }

    #[test]
    fn invalid_utf8_in_a_paste_is_replaced_not_fatal() {
        assert_eq!(
            parse(b"\x1b[200~a\xffb\x1b[201~"),
            vec![paste("a\u{fffd}b")]
        );
    }

    #[test]
    fn a_huge_paste_is_cut_to_a_limit_instead_of_eating_memory() {
        let mut p = Parser::new();
        p.feed(b"\x1b[200~");
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..64 {
            p.feed(&chunk); // 4 MiB in all
            assert_eq!(p.next_event(), None);
        }
        p.feed(b"\x1b[201~z");
        match p.next_event() {
            Some(Event::Paste(text)) => {
                assert!(text.len() <= 1 << 20, "kept {} bytes", text.len());
                assert!(text.len() >= 1 << 19, "kept far too little");
            }
            other => panic!("expected a paste, got {other:?}"),
        }
        assert_eq!(p.next_event(), Some(key(KeyCode::Char('z'))));
    }

    #[test]
    fn a_stray_paste_end_marker_is_ignored() {
        assert_eq!(parse(b"\x1b[201~a"), vec![key(KeyCode::Char('a'))]);
    }

    #[test]
    fn garbage_inside_a_paste_never_panics() {
        for a in 0..=255u8 {
            let mut p = Parser::new();
            p.feed(b"\x1b[200~");
            p.feed(&[a, 0x1b, a, b'[', a]);
            p.feed(b"\x1b[201~");
            while p.next_event().is_some() {}
            p.flush();
        }
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
    fn kitty_plain_key_is_a_press_with_or_without_the_event_type() {
        assert_eq!(parse(b"\x1b[97u"), vec![key(KeyCode::Char('a'))]);
        assert_eq!(parse(b"\x1b[97;1:1u"), vec![key(KeyCode::Char('a'))]);
    }

    #[test]
    fn kitty_repeat_is_a_key_press_so_existing_code_keeps_working() {
        assert_eq!(parse(b"\x1b[97;1:2u"), vec![key(KeyCode::Char('a'))]);
    }

    #[test]
    fn kitty_release_is_a_key_up() {
        assert_eq!(parse(b"\x1b[97;1:3u"), vec![key_up(KeyCode::Char('a'))]);
    }

    #[test]
    fn kitty_modifiers_are_one_plus_shift_alt_ctrl_bits() {
        assert_eq!(
            parse(b"\x1b[97;5u"),
            vec![key_mod(KeyCode::Char('a'), false, false, true)]
        );
        assert_eq!(
            parse(b"\x1b[97;2u"),
            vec![key_mod(KeyCode::Char('a'), true, false, false)]
        );
        assert_eq!(
            parse(b"\x1b[97;3u"),
            vec![key_mod(KeyCode::Char('a'), false, true, false)]
        );
    }

    #[test]
    fn kitty_release_keeps_its_modifiers() {
        assert_eq!(
            parse(b"\x1b[97;5:3u"),
            vec![Event::KeyUp(Key {
                code: KeyCode::Char('a'),
                modifiers: Modifiers {
                    shift: false,
                    alt: false,
                    ctrl: true
                },
            })]
        );
    }

    #[test]
    fn kitty_modifier_bits_above_ctrl_are_ignored() {
        // 1 + 4 (ctrl) + 64 (super) + 128 (caps lock)
        assert_eq!(
            parse(b"\x1b[97;197u"),
            vec![key_mod(KeyCode::Char('a'), false, false, true)]
        );
    }

    #[test]
    fn kitty_special_codes_are_the_named_keys() {
        assert_eq!(parse(b"\x1b[27u"), vec![key(KeyCode::Esc)]);
        assert_eq!(parse(b"\x1b[13u"), vec![key(KeyCode::Enter)]);
        assert_eq!(parse(b"\x1b[9u"), vec![key(KeyCode::Tab)]);
        assert_eq!(parse(b"\x1b[127u"), vec![key(KeyCode::Backspace)]);
        assert_eq!(parse(b"\x1b[13;1:3u"), vec![key_up(KeyCode::Enter)]);
    }

    #[test]
    fn kitty_other_code_points_are_characters() {
        assert_eq!(parse(b"\x1b[233u"), vec![key(KeyCode::Char('é'))]);
        assert_eq!(parse(b"\x1b[8364u"), vec![key(KeyCode::Char('€'))]);
    }

    #[test]
    fn kitty_private_use_codes_are_consumed_and_ignored() {
        // keypad and media keys live in 57344..=63743
        assert_eq!(parse(b"\x1b[57399uq"), vec![key(KeyCode::Char('q'))]);
        assert_eq!(parse(b"\x1b[57344;1:3uq"), vec![key(KeyCode::Char('q'))]);
        assert_eq!(parse(b"\x1b[63743uq"), vec![key(KeyCode::Char('q'))]);
    }

    #[test]
    fn kitty_invalid_code_points_are_consumed_and_ignored() {
        // a surrogate, and past the last code point
        assert_eq!(parse(b"\x1b[55296uq"), vec![key(KeyCode::Char('q'))]);
        assert_eq!(parse(b"\x1b[1114112uq"), vec![key(KeyCode::Char('q'))]);
    }

    #[test]
    fn kitty_alternate_keys_and_text_fields_are_ignored() {
        // shifted key 65 (A) and base key 97 after the first ':', text "A" last
        assert_eq!(
            parse(b"\x1b[97:65:97;2u"),
            vec![key_mod(KeyCode::Char('a'), true, false, false)]
        );
        assert_eq!(
            parse(b"\x1b[97;2;65u"),
            vec![key_mod(KeyCode::Char('a'), true, false, false)]
        );
    }

    #[test]
    fn kitty_truncated_sequence_waits_for_the_rest() {
        let mut p = Parser::new();
        p.feed(b"\x1b[97;1:3");
        assert_eq!(p.next_event(), None);
        assert!(p.has_pending());
        p.feed(b"u");
        assert_eq!(p.next_event(), Some(key_up(KeyCode::Char('a'))));
    }

    #[test]
    fn malformed_kitty_sequences_are_consumed_and_ignored() {
        let malformed: [&[u8]; 6] = [
            b"\x1b[97;;;;u",
            b"\x1b[:u",
            b"\x1b[u",
            b"\x1b[97;1:9u",
            b"\x1b[99999999999u",
            b"\x1b[97;99999999999u",
        ];
        for bytes in malformed {
            let mut input = bytes.to_vec();
            input.push(b'q');
            assert_eq!(
                parse(&input),
                vec![key(KeyCode::Char('q'))],
                "input {:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    #[test]
    fn arrows_and_navigation_keys_with_an_event_type() {
        assert_eq!(parse(b"\x1b[1;1:3A"), vec![key_up(KeyCode::Up)]);
        assert_eq!(parse(b"\x1b[1;1:1B"), vec![key(KeyCode::Down)]);
        assert_eq!(
            parse(b"\x1b[1;5:3C"),
            vec![Event::KeyUp(Key {
                code: KeyCode::Right,
                modifiers: Modifiers {
                    shift: false,
                    alt: false,
                    ctrl: true
                },
            })]
        );
        assert_eq!(parse(b"\x1b[1;1:3H"), vec![key_up(KeyCode::Home)]);
        assert_eq!(parse(b"\x1b[1;1:2F"), vec![key(KeyCode::End)]);
    }

    #[test]
    fn tilde_and_function_keys_with_an_event_type() {
        assert_eq!(parse(b"\x1b[3;1:3~"), vec![key_up(KeyCode::Delete)]);
        assert_eq!(parse(b"\x1b[5;1:1~"), vec![key(KeyCode::PageUp)]);
        assert_eq!(parse(b"\x1b[15;1:3~"), vec![key_up(KeyCode::F(5))]);
    }

    #[test]
    fn legacy_key_with_an_unknown_event_type_is_ignored() {
        assert_eq!(parse(b"\x1b[1;1:9Aq"), vec![key(KeyCode::Char('q'))]);
    }

    /// Every event that comes out of the parser after `input` is fed.
    fn drain(parser: &mut Parser) -> Vec<Event> {
        std::iter::from_fn(|| parser.next_event()).collect()
    }

    #[test]
    fn keyboard_flags_reply_is_recorded_and_is_not_a_key() {
        let mut p = Parser::new();
        p.feed(b"\x1b[?11u");
        assert_eq!(drain(&mut p), vec![]);
        assert_eq!(p.take_keyboard_flags(), Some(11));
        assert_eq!(p.take_keyboard_flags(), None, "taking clears it");
    }

    #[test]
    fn keyboard_flags_reply_with_no_flags_is_zero_and_not_a_key_a() {
        // `?97u` would read as the key 'a' without the `?`
        let mut p = Parser::new();
        p.feed(b"\x1b[?0u\x1b[?97u");
        assert_eq!(drain(&mut p), vec![]);
        assert_eq!(p.take_keyboard_flags(), Some(97));
    }

    #[test]
    fn keyboard_flags_reply_split_at_every_byte_boundary() {
        let reply = b"\x1b[?11ub";
        for split in 1..reply.len() {
            let mut p = Parser::new();
            p.feed(&reply[..split]);
            let mut events = drain(&mut p);
            p.feed(&reply[split..]);
            events.extend(drain(&mut p));
            assert_eq!(events, vec![key(KeyCode::Char('b'))], "split at {split}");
            assert_eq!(p.take_keyboard_flags(), Some(11), "split at {split}");
        }
    }

    #[test]
    fn a_keyboard_flags_reply_with_a_huge_number_is_consumed_and_ignored() {
        let mut p = Parser::new();
        p.feed(b"\x1b[?99999999999uq");
        assert_eq!(drain(&mut p), vec![key(KeyCode::Char('q'))]);
        assert_eq!(p.take_keyboard_flags(), None);
    }

    #[test]
    fn attributes_reply_is_recorded_and_is_not_a_key() {
        let mut p = Parser::new();
        p.feed(b"\x1b[?62;22c");
        assert_eq!(drain(&mut p), vec![]);
        assert!(p.take_attributes_reply());
        assert!(!p.take_attributes_reply(), "taking clears it");
    }

    #[test]
    fn attributes_reply_split_at_every_byte_boundary() {
        let reply = b"\x1b[?62;22cb";
        for split in 1..reply.len() {
            let mut p = Parser::new();
            p.feed(&reply[..split]);
            let mut events = drain(&mut p);
            p.feed(&reply[split..]);
            events.extend(drain(&mut p));
            assert_eq!(events, vec![key(KeyCode::Char('b'))], "split at {split}");
            assert!(p.take_attributes_reply(), "split at {split}");
        }
    }

    #[test]
    fn random_sequences_from_the_protocol_alphabet_terminate_and_never_panic() {
        // a tiny xorshift generator, so the test needs no crates
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let alphabet = b"\x1b[;:?0123456789uABCDHF~a";
        for _ in 0..200 {
            let len = 1 + (next() % 40) as usize;
            let input: Vec<u8> = (0..len)
                .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
                .collect();
            let mut p = Parser::new();
            // one byte at a time exercises every point where a sequence can pause
            let mut events = 0;
            for &byte in &input {
                p.feed(&[byte]);
                while p.next_event().is_some() {
                    events += 1;
                }
            }
            // every event uses at least one byte, so this bounds the loop
            assert!(events <= input.len(), "input {input:?}");
            p.flush();
        }
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

//! Shared by the end-to-end tests (`e2e_windows.rs`, `e2e_unix.rs`).
//!
//! Each of those files defines a `Pty` (a child process attached to a pseudo
//! terminal) with `spawn`, `send`, `text`, `expect` and `exit_code`, then calls
//! `e2e_scenarios!()` so Windows and Unix run exactly the same tests.
#![allow(dead_code)]

/// Removes ANSI escape sequences so assertions see only the text on screen.
pub fn strip_escapes(bytes: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        match bytes.get(i) {
            Some(b'[') => {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i += 1;
            }
            Some(b']') => {
                while i < bytes.len()
                    && bytes[i] != 0x07
                    && !(bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\'))
                {
                    i += 1;
                }
                i += if bytes.get(i) == Some(&0x07) { 1 } else { 2 };
            }
            _ => i += 1,
        }
    }
    String::from_utf8_lossy(&out).replace("\r\n", "\n")
}

/// The same scenarios on every platform. Needs `Pty` and `events_example()` in scope.
macro_rules! e2e_scenarios {
    () => {
        fn started() -> Pty {
            let pty = Pty::spawn(&events_example());
            pty.expect("q or Ctrl+C quits");
            pty
        }

        #[test]
        fn the_example_starts_in_a_real_console() {
            // RawMode::enable succeeded, or the banner would never print
            started();
        }

        #[test]
        fn a_key_arrives_immediately_without_pressing_enter() {
            let pty = started();
            pty.send(b"a");
            pty.expect("Char('a')");
        }

        #[test]
        fn input_is_not_echoed_by_the_console() {
            let pty = started();
            pty.send(b"zzz");
            pty.expect("Char('z')");
            // the events are printed, but the typed text must not appear on its own
            assert!(!pty.text().contains("zzz"), "{:?}", pty.text());
        }

        #[test]
        fn a_mouse_click_is_parsed_with_zero_based_coordinates() {
            let pty = started();
            pty.send(b"\x1b[<0;12;5M");
            pty.expect("kind: Down(Left), x: 11, y: 4");
        }

        #[test]
        fn a_mouse_drag_and_release_are_reported() {
            let pty = started();
            pty.send(b"\x1b[<32;3;4M\x1b[<0;3;4m");
            pty.expect("kind: Drag(Left), x: 2, y: 3");
            pty.expect("kind: Up(Left), x: 2, y: 3");
        }

        #[test]
        fn arrow_keys_arrive_as_arrow_events() {
            let pty = started();
            pty.send(b"\x1b[A");
            pty.expect("code: Up");
        }

        #[test]
        fn q_quits_with_a_zero_exit_code() {
            let pty = started();
            pty.send(b"q");
            assert_eq!(pty.exit_code(), 0);
        }

        #[test]
        fn ctrl_c_is_delivered_as_a_key_and_the_program_exits_cleanly() {
            let pty = started();
            pty.send(&[0x03]);
            pty.expect("ctrl: true");
            assert_eq!(pty.exit_code(), 0);
        }
    };
}

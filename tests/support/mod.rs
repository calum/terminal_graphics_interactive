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
            let pty = Pty::spawn(&example("events"));
            pty.expect("q or Ctrl+C quits");
            pty
        }

        #[test]
        fn the_example_starts_in_a_real_console() {
            // RawMode::enable succeeded, or the banner would never print
            started();
        }

        #[test]
        fn paint_draws_its_screen_with_terminal_draw_then_quits_cleanly() {
            // the paint example shows a status line on the last row of its
            // display; seeing it proves `Terminal::draw` output reaches a real
            // terminal, and exit code 0 after `q` proves cleanup ran
            let pty = Pty::spawn(&example("paint"));
            pty.expect("q: quit");
            // a click and a drag, which must not break anything
            pty.send(b"\x1b[<0;10;5M\x1b[<32;20;5M\x1b[<0;20;5m");
            pty.send(b"q");
            assert_eq!(pty.exit_code(), 0);
        }

        #[test]
        fn the_screensaver_animates_by_itself_and_any_key_quits() {
            // nothing is sent until the first block is on screen, so a block
            // appearing proves the frame loop draws without any input; a key
            // then proves input is handled while animating
            let pty = Pty::spawn(&example("screensaver"));
            pty.expect("█");
            pty.send(b"x");
            assert_eq!(pty.exit_code(), 0);
        }

        #[test]
        fn the_quickstart_from_llms_txt_marks_a_clicked_cell_and_quits() {
            // this is the program an AI agent will copy from llms.txt
            let pty = Pty::spawn(&example("quickstart"));
            // The screen is blank until a click, so an X proves the whole path:
            // click parsed, mapped to a cell, drawn. The program prints nothing
            // that says it is ready, so click until it responds; a click sent
            // too early is harmless.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !pty.text().contains('X') {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no X appeared; the program printed {:?}",
                    pty.text()
                );
                pty.send(b"\x1b[<0;10;5M");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            pty.send(b"q");
            assert_eq!(pty.exit_code(), 0);
        }

        #[test]
        fn pasted_text_arrives_as_one_paste_event_not_key_presses() {
            let pty = started();
            // what a terminal sends when you paste, once bracketed paste is on
            pty.send(b"\x1b[200~cargo run\x1b[201~");
            pty.expect("Paste(\"cargo run\")");
            assert!(
                !pty.text().contains("Char('"),
                "the pasted letters must not arrive as key presses: {:?}",
                pty.text()
            );
        }

        #[test]
        fn pasting_a_q_does_not_quit_paint_but_typing_one_does() {
            let pty = Pty::spawn(&example("paint"));
            pty.expect("q: quit");
            pty.send(b"\x1b[200~cargo run q 7 c\x1b[201~");
            std::thread::sleep(std::time::Duration::from_millis(400));
            assert!(!pty.has_exited(), "a pasted q quit the program");
            pty.send(b"q");
            assert_eq!(pty.exit_code(), 0);
        }

        #[test]
        fn the_window_size_is_found_by_asking_the_terminal() {
            // Terminal::start moves the cursor to the far corner, asks where it
            // is, and reads the answer; the harness answers like a terminal does
            let pty = started();
            pty.expect(&format!("size: {PTY_COLUMNS}x{PTY_ROWS}"));
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

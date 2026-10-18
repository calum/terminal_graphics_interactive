//! SAIL TOGETHER: `sail` for two to four players over the network. Everyone
//! sails the same sea and dodges the same fireballs, each in a boat of their
//! own. Only the players' key presses cross the network (lockstep over TCP), so
//! every machine runs the whole game and the sea is the same everywhere.
//!
//! One player hosts and the others join, each in their own terminal:
//!
//! ```text
//! cargo run --example sail_together -- host 2        (waits for 1 more player)
//! cargo run --example sail_together -- join 192.168.1.5:7777
//! ```
//!
//! The sea is as big as the host's window, so the host should start in a
//! window that every player's window can match; a smaller window is told how
//! big it must be. A and D (or the arrows) steer and q leaves. Over the
//! internet, the host forwards port 7777.

#[allow(dead_code)]
#[path = "sail.rs"]
mod sail;

use std::io;
use std::time::Duration;
use terminal_graphics_interactive::game::*;

/// The port a host listens on unless told another.
const DEFAULT_PORT: u16 = 7777;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match parse(&args) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Err(e) = play(mode) {
        eprintln!("sail_together: {e}");
        std::process::exit(1);
    }
}

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Host { players: usize, port: u16 },
    Join { addr: String },
}

const USAGE: &str = "usage:\n  sail_together host [PLAYERS] [PORT]   host a match for 2 to 4 players (default 2, port 7777)\n  sail_together join HOST:PORT          join a match, e.g. join 192.168.1.5:7777";

fn parse(args: &[String]) -> Result<Mode, String> {
    match args.first().map(String::as_str) {
        Some("host") => {
            let players = match args.get(1) {
                Some(text) => text
                    .parse::<usize>()
                    .ok()
                    .filter(|n| (2..=4).contains(n))
                    .ok_or_else(|| format!("PLAYERS must be 2, 3 or 4, not {text:?}\n{USAGE}"))?,
                None => 2,
            };
            let port = match args.get(2) {
                Some(text) => text.parse::<u16>().map_err(|_| {
                    format!("PORT must be a number from 1 to 65535, not {text:?}\n{USAGE}")
                })?,
                None => DEFAULT_PORT,
            };
            Ok(Mode::Host { players, port })
        }
        Some("join") => match args.get(1) {
            Some(addr) if addr.contains(':') => Ok(Mode::Join { addr: addr.clone() }),
            Some(addr) => Err(format!(
                "the address needs a port, like {addr}:{DEFAULT_PORT}\n{USAGE}"
            )),
            None => Err(USAGE.to_string()),
        },
        _ => Err(USAGE.to_string()),
    }
}

fn play(mode: Mode) -> io::Result<()> {
    let (tcp, settings) = match mode {
        Mode::Host { players, port } => {
            let host = Host::bind(("0.0.0.0", port))?;
            println!(
                "waiting for {} more player(s) on port {port}.\nThey join with: cargo run --example sail_together -- join <this machine's address>:{port}",
                players - 1
            );
            host_session(host, players, seed_from_clock(), window_size()?)?
        }
        Mode::Join { addr } => {
            println!("joining {addr} ...");
            join_session(&addr)?
        }
    };
    let config = Config::new(tcp.players(), PlayerId(tcp.me().0));
    run(sail::together(tcp, config, &settings))
}

/// The size of this terminal window, asked by starting a session for a moment.
/// The host's sea is as big as its window.
fn window_size() -> io::Result<(u32, u32)> {
    Ok(Terminal::start()?.size())
}

/// Waits for the other players, then tells them the seed and the size of the
/// sea. Returns the host's connection and the settings it announced.
fn host_session(
    host: Host,
    players: usize,
    seed: u64,
    window: (u32, u32),
) -> io::Result<(Tcp, sail::Settings)> {
    let mut tcp = host.accept(players, Duration::from_secs(300))?;
    let settings = sail::Settings::sized(seed, window.0, window.1);
    announce(&mut tcp, players, &settings)?;
    Ok((tcp, settings))
}

/// Joins a host and reads the settings it announces.
fn join_session(addr: &str) -> io::Result<(Tcp, sail::Settings)> {
    let mut tcp = Tcp::join(addr, Duration::from_secs(30))?;
    let settings = await_message(&mut tcp, PeerId(0), Duration::from_secs(30))?;
    Ok((tcp, settings))
}

/// A seed from the clock, so every match is a different sea.
fn seed_from_clock() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_nanos() as u64,
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Instant;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn a_hosts_window_sets_the_sea_leaving_the_terminals_spare_row() {
        let settings = sail::Settings::sized(1, 120, 40);
        assert_eq!((settings.width, settings.rows), (120, 39));
        let tiny = sail::Settings::sized(1, 0, 0);
        assert_eq!((tiny.width, tiny.rows), (1, 1), "never an empty sea");
    }
    #[test]
    fn the_command_line_chooses_host_or_join() {
        assert_eq!(
            parse(&words("host")),
            Ok(Mode::Host {
                players: 2,
                port: 7777
            })
        );
        assert_eq!(
            parse(&words("host 4 9000")),
            Ok(Mode::Host {
                players: 4,
                port: 9000
            })
        );
        assert_eq!(
            parse(&words("join 10.0.0.2:7777")),
            Ok(Mode::Join {
                addr: "10.0.0.2:7777".to_string()
            })
        );
    }

    #[test]
    fn a_bad_command_line_says_what_to_type_instead() {
        for bad in [
            "",
            "host 1",
            "host 5",
            "host x",
            "host 2 99999",
            "join",
            "play",
        ] {
            let message = parse(&words(bad)).unwrap_err();
            assert!(message.contains("usage:"), "{bad:?}: {message}");
        }
        let message = parse(&words("join 10.0.0.2")).unwrap_err();
        assert!(message.contains("10.0.0.2:7777"), "{message}");
    }

    /// The leagues sailed, read back from the HUD.
    fn leagues(text: &str) -> f32 {
        let row = text.lines().next().unwrap_or("");
        let end = row.find(" leagues").expect("the HUD shows the leagues");
        let start = row[..end].rfind(' ').map_or(0, |i| i + 1);
        row[start..end].parse().expect("a number of leagues")
    }

    #[test]
    fn two_players_in_two_threads_sail_together_over_real_tcp() {
        let host = Host::bind("127.0.0.1:0").unwrap();
        let addr = host.local_addr().unwrap().to_string();
        let done = Arc::new(AtomicUsize::new(0));
        // nobody drops their connection until both have finished: a machine
        // that is gone is (correctly) reported as "left the game"
        let leave_together = Arc::new(Barrier::new(2));

        let play_until_done =
            |tcp: Tcp, settings: sail::Settings, done: Arc<AtomicUsize>, gate: Arc<Barrier>| {
                let config = Config::new(tcp.players(), PlayerId(tcp.me().0));
                let mut game = Headless::new(sail::together(tcp, config, &settings)).size(100, 29);
                let started = Instant::now();
                let mut finished = false;
                loop {
                    game.step(1);
                    let text = game.text();
                    assert!(!text.contains("drifted apart"), "desync:\n{text}");
                    assert!(!text.contains("left the game"), "lost a player:\n{text}");
                    assert!(!text.contains("connection failed"), "{text}");
                    if !finished && leagues(&text) >= 2.0 {
                        finished = true;
                        done.fetch_add(1, Ordering::SeqCst);
                    }
                    if done.load(Ordering::SeqCst) == 2 {
                        gate.wait();
                        return game.text();
                    }
                    assert!(
                        started.elapsed() < Duration::from_secs(60),
                        "too slow:\n{text}"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
            };

        let host_done = Arc::clone(&done);
        let host_gate = Arc::clone(&leave_together);
        let host_thread = thread::spawn(move || {
            let (tcp, settings) = host_session(host, 2, 77, (100, 30)).unwrap();
            play_until_done(tcp, settings, host_done, host_gate)
        });
        let (tcp, settings) = join_session(&addr).unwrap();
        assert_eq!(
            settings,
            sail::Settings::sized(77, 100, 30),
            "the client got the host's settings, with a sea as big as the host's window"
        );
        let client_text = play_until_done(tcp, settings, Arc::clone(&done), leave_together);
        let host_text = host_thread.join().unwrap();

        for text in [&host_text, &client_text] {
            let crew = text.lines().nth(1).unwrap();
            assert!(crew.contains("P1") && crew.contains("P2"), "{crew}");
        }
    }
}

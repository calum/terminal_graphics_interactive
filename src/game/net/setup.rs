//! [`announce`] and [`await_message`]: sharing the start-of-match settings.

use super::{Decode, Encode, PeerId, Transport};
use std::io;
use std::thread;
use std::time::{Duration, Instant};

/// Sends `message` from the host (player 0) to every other player, once. Use
/// it to share what everyone must agree on before the match starts: the seed,
/// the size of the world, the knobs. Clients call [`await_message`].
///
/// ```
/// use std::time::Duration;
/// use terminal_graphics_interactive::game::*;
///
/// let mut ends = Local::network(2).into_iter();
/// let (mut host, mut client) = (ends.next().unwrap(), ends.next().unwrap());
///
/// // the host picks the seed and tells everyone
/// announce(&mut host, 2, &(12345u64, (80u32, 24u32)))?;
/// let (seed, size): (u64, (u32, u32)) =
///     await_message(&mut client, PeerId(0), Duration::from_secs(5))?;
/// assert_eq!((seed, size), (12345, (80, 24)));
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn announce<T: Transport, M: Encode>(
    transport: &mut T,
    players: usize,
    message: &M,
) -> io::Result<()> {
    let bytes = message.to_bytes();
    for peer in 1..players.min(256) {
        transport.send(PeerId(peer as u8), &bytes)?;
    }
    Ok(())
}

/// Waits up to `timeout` for the message that [`announce`] sends. Messages
/// from anyone but `from` are skipped. Fails with `TimedOut` if nothing
/// comes, and `InvalidData` if it is not the expected shape (is the other
/// side running the same version of the game?).
pub fn await_message<T: Transport, M: Decode>(
    transport: &mut T,
    from: PeerId,
    timeout: Duration,
) -> io::Result<M> {
    let deadline = Instant::now() + timeout;
    loop {
        match transport.poll()? {
            Some((peer, bytes)) if peer == from => {
                return M::from_bytes(&bytes).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("the host's start message was not understood ({e}); is it the same version of the game?"),
                    )
                });
            }
            Some(_) => {}
            None => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("no start message from the host within {timeout:?}; is the host running?"),
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Local, PeerId};

    #[test]
    fn the_host_announces_and_every_client_receives_the_message() {
        let mut ends = Local::network(3).into_iter();
        let (mut host, mut one, mut two) = (
            ends.next().unwrap(),
            ends.next().unwrap(),
            ends.next().unwrap(),
        );
        announce(&mut host, 3, &(42u64, vec![1i32, 2, 3])).unwrap();
        let wait = Duration::from_secs(1);
        let got: (u64, Vec<i32>) = await_message(&mut one, PeerId(0), wait).unwrap();
        assert_eq!(got, (42, vec![1, 2, 3]));
        let got: (u64, Vec<i32>) = await_message(&mut two, PeerId(0), wait).unwrap();
        assert_eq!(got, (42, vec![1, 2, 3]));
    }

    #[test]
    fn waiting_for_a_message_that_never_comes_times_out() {
        let mut ends = Local::network(2).into_iter();
        let _host = ends.next().unwrap();
        let mut client = ends.next().unwrap();
        let err =
            await_message::<_, u32>(&mut client, PeerId(0), Duration::from_millis(30)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(err.to_string().contains("host"), "{err}");
    }

    #[test]
    fn a_message_of_the_wrong_shape_is_invalid_data_not_a_panic() {
        let mut ends = Local::network(2).into_iter();
        let (mut host, mut client) = (ends.next().unwrap(), ends.next().unwrap());
        announce(&mut host, 2, &7u8).unwrap();
        let err =
            await_message::<_, u64>(&mut client, PeerId(0), Duration::from_secs(1)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_message_from_someone_else_is_skipped() {
        let mut ends = Local::network(3).into_iter();
        let (mut host, mut other, mut client) = (
            ends.next().unwrap(),
            ends.next().unwrap(),
            ends.next().unwrap(),
        );
        other.send(PeerId(2), &9u32.to_bytes()).unwrap();
        announce(&mut host, 3, &5u32).unwrap();
        let got: u32 = await_message(&mut client, PeerId(0), Duration::from_secs(1)).unwrap();
        assert_eq!(got, 5);
    }
}

//! [`Host`] and [`Tcp`]: real players over TCP. The host is `PeerId(0)` and
//! every client connects to it, so the network is a star.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use super::{PeerId, Transport};

/// The largest message payload, in bytes (1 MiB).
const MAX_FRAME: usize = 1024 * 1024;

/// Waits for the other players to connect. Bind it, tell the players the
/// address, then call [`Host::accept`].
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let host = Host::bind("127.0.0.1:0")?; // port 0: let the system pick one
/// println!("players join at {}", host.local_addr()?);
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct Host {
    listener: TcpListener,
}

impl Host {
    /// Listens on `addr`. Use port `0` to let the system pick a free port, then
    /// read it back with [`Host::local_addr`].
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Host> {
        Ok(Host {
            listener: TcpListener::bind(addr)?,
        })
    }

    /// The address the host is listening on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Waits until `players - 1` clients have joined, giving each its player
    /// number, then returns the connected [`Tcp`] for the host. Fails with
    /// `TimedOut` if they do not all arrive within `timeout`.
    pub fn accept(self, players: usize, timeout: Duration) -> io::Result<Tcp> {
        check_players(players)?;
        self.listener.set_nonblocking(true)?;
        let deadline = Instant::now() + timeout;
        let mut links: Vec<Option<Link>> = (0..players).map(|_| None).collect();
        let mut joined = 0;
        while joined + 1 < players {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let id = joined + 1;
                    let stream = hello_client(stream, id, players, deadline, timeout)?;
                    links[id] = Some(Link::new(stream)?);
                    joined += 1;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(not_enough_players(joined, players, timeout));
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Tcp {
            me: PeerId(0),
            players,
            links,
            next: 0,
        })
    }
}

/// Sends the two-byte hello `[player id, players]` to a new client.
fn hello_client(
    mut stream: TcpStream,
    id: usize,
    players: usize,
    deadline: Instant,
    timeout: Duration,
) -> io::Result<TcpStream> {
    stream.set_nonblocking(false)?;
    let left = deadline.saturating_duration_since(Instant::now());
    stream.set_write_timeout(Some(left.max(Duration::from_millis(1))))?;
    stream.write_all(&[id as u8, players as u8]).map_err(|e| {
        if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("player {id} connected but did not answer the hello within {timeout:?}"),
            )
        } else {
            e
        }
    })?;
    Ok(stream)
}

fn not_enough_players(joined: usize, players: usize, timeout: Duration) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "only {} of {players} players joined within {timeout:?}; start the others with Tcp::join(addr, timeout)",
            joined + 1
        ),
    )
}

fn check_players(players: usize) -> io::Result<()> {
    if (1..=255).contains(&players) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a game needs 1 to 255 players, not {players}"),
        ))
    }
}

/// Bytes waiting to be read or written, with a read position so consuming
/// from the front is cheap.
#[derive(Default)]
struct Buf {
    data: Vec<u8>,
    start: usize,
}

impl Buf {
    fn push(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn unread(&self) -> &[u8] {
        &self.data[self.start..]
    }

    fn consume(&mut self, n: usize) {
        self.start += n;
        if self.start == self.data.len() {
            self.data.clear();
            self.start = 0;
        } else if self.start >= 64 * 1024 && self.start * 2 >= self.data.len() {
            self.data.drain(..self.start);
            self.start = 0;
        }
    }
}

/// One connection to a peer, with the bytes waiting in each direction.
struct Link {
    stream: TcpStream,
    /// `false` once the peer has closed or the connection has failed.
    open: bool,
    /// Framed bytes not yet accepted by the socket.
    out: Buf,
    /// Bytes received but not yet returned as a whole message.
    inbuf: Buf,
}

impl Link {
    fn new(stream: TcpStream) -> io::Result<Link> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Link {
            stream,
            open: true,
            out: Buf::default(),
            inbuf: Buf::default(),
        })
    }

    /// Writes as much of `out` as the socket takes now. A real error closes the link.
    fn flush(&mut self) -> io::Result<()> {
        while !self.out.unread().is_empty() {
            match self.stream.write(self.out.unread()) {
                Ok(0) => {
                    self.open = false;
                    return Err(io::ErrorKind::WriteZero.into());
                }
                Ok(n) => self.out.consume(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.open = false;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Reads everything the socket has right now into `inbuf`.
    fn fill(&mut self) {
        let mut chunk = [0u8; 16 * 1024];
        while self.open {
            match self.stream.read(&mut chunk) {
                Ok(0) => self.open = false,
                Ok(n) => self.inbuf.push(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => self.open = false,
            }
        }
    }

    /// Takes one whole message from `inbuf` if one has fully arrived.
    fn next_frame(&mut self, peer: usize) -> io::Result<Option<Vec<u8>>> {
        let data = self.inbuf.unread();
        if data.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if len > MAX_FRAME {
            self.open = false;
            self.inbuf = Buf::default();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "peer {peer} sent a {len}-byte message, over the 1 MiB limit; \
                     closed that connection (is the other side speaking this protocol?)"
                ),
            ));
        }
        if data.len() < 4 + len {
            return Ok(None);
        }
        let payload = data[4..4 + len].to_vec();
        self.inbuf.consume(4 + len);
        Ok(Some(payload))
    }
}

impl Transport for Tcp {
    fn send(&mut self, to: PeerId, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "message is {} bytes, over the 1 MiB limit; split it into smaller messages",
                    bytes.len()
                ),
            ));
        }
        let Some(link) = self.links.get_mut(to.0 as usize).and_then(Option::as_mut) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "not connected to {to:?}: a client only talks to the host (PeerId(0)), \
                     and the host talks to each client"
                ),
            ));
        };
        if !link.open {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("{to:?} has left the game; stop sending to it"),
            ));
        }
        link.out.push(&(bytes.len() as u32).to_le_bytes());
        link.out.push(bytes);
        link.flush()
    }

    fn poll(&mut self) -> io::Result<Option<(PeerId, Vec<u8>)>> {
        for link in self.links.iter_mut().flatten() {
            // A failed flush closes the link, which `is_open` reports.
            let _ = link.flush();
            link.fill();
        }
        let n = self.links.len();
        for step in 0..n {
            let i = (self.next + step) % n;
            let Some(link) = self.links[i].as_mut() else {
                continue;
            };
            if let Some(payload) = link.next_frame(i)? {
                self.next = (i + 1) % n;
                return Ok(Some((PeerId(i as u8), payload)));
            }
        }
        Ok(None)
    }

    fn is_open(&self, peer: PeerId) -> bool {
        self.links
            .get(peer.0 as usize)
            .is_some_and(|link| link.as_ref().is_some_and(|l| l.open))
    }
}

#[cfg(test)]
impl Tcp {
    /// Reads what has arrived on every link without returning a message.
    fn poll_fill_for_test(&mut self) {
        for link in self.links.iter_mut().flatten() {
            link.fill();
        }
    }
}

/// A player's connections to the other players, with the host as peer 0.
/// Implements [`Transport`]; `send` and `poll` never block.
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use terminal_graphics_interactive::game::*;
///
/// let host = Host::bind("127.0.0.1:0")?;
/// let addr = host.local_addr()?;
/// let waiting = thread::spawn(move || host.accept(2, Duration::from_secs(5)));
///
/// let mut client = Tcp::join(addr, Duration::from_secs(5))?;
/// let mut host = waiting.join().unwrap()?;
/// assert_eq!((client.me(), client.players()), (PeerId(1), 2));
///
/// client.send(PeerId(0), b"hello host")?;
/// let (from, bytes) = loop {
///     if let Some(message) = host.poll()? {
///         break message; // poll never blocks, so ask again until it arrives
///     }
///     thread::sleep(Duration::from_millis(1));
/// };
/// assert_eq!((from, bytes.as_slice()), (PeerId(1), &b"hello host"[..]));
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct Tcp {
    me: PeerId,
    players: usize,
    /// Indexed by peer id; `None` for ourselves and for peers we are not
    /// connected to.
    links: Vec<Option<Link>>,
    /// The peer the next `poll` starts from, so no peer is starved.
    next: usize,
}

impl Tcp {
    /// Connects to a host at `addr` and receives this player's number and the
    /// number of players. Fails within `timeout` if no host answers.
    pub fn join(addr: impl ToSocketAddrs, timeout: Duration) -> io::Result<Tcp> {
        let deadline = Instant::now() + timeout;
        let mut stream = None;
        let mut last_error = None;
        for candidate in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&candidate, timeout) {
                Ok(connected) => {
                    stream = Some(connected);
                    break;
                }
                Err(e) => last_error = Some(e),
            }
        }
        let Some(mut stream) = stream else {
            let reason = last_error.map_or_else(
                || "the address resolved to nothing".to_string(),
                |e| e.to_string(),
            );
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("could not reach a host: {reason}; is the host running with Host::accept?"),
            ));
        };

        let mut hello = [0u8; 2];
        let mut got = 0;
        while got < hello.len() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("the host did not send its hello within {timeout:?}"),
                ));
            }
            stream.set_read_timeout(Some(left))?;
            match stream.read(&mut hello[got..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the host closed the connection before saying hello",
                    ))
                }
                Ok(n) => got += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        let (id, players) = (hello[0] as usize, hello[1] as usize);
        if id == 0 || id >= players {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the host gave player number {id} of {players}, which is not a client"),
            ));
        }
        let mut links: Vec<Option<Link>> = (0..players).map(|_| None).collect();
        links[0] = Some(Link::new(stream)?);
        Ok(Tcp {
            me: PeerId(id as u8),
            players,
            links,
            next: 0,
        })
    }

    /// This player's id: `PeerId(0)` for the host, `1`, `2`… for clients.
    pub fn me(&self) -> PeerId {
        self.me
    }

    /// How many players are in the game, the host included.
    pub fn players(&self) -> usize {
        self.players
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Instant;

    const LOCAL: &str = "127.0.0.1:0";

    /// Polls `step` until it yields a value; fails with `what` after 5 seconds.
    fn wait_for<T>(what: &str, mut step: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(value) = step() {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// A host waiting for `players - 1` clients on another thread.
    fn host_for(players: usize) -> (SocketAddr, thread::JoinHandle<io::Result<Tcp>>) {
        let host = Host::bind(LOCAL).unwrap();
        let addr = host.local_addr().unwrap();
        let handle = thread::spawn(move || host.accept(players, Duration::from_secs(5)));
        (addr, handle)
    }

    #[test]
    fn host_and_two_clients_join_with_ids_one_and_two() {
        let (addr, host) = host_for(3);
        let c1 = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let c2 = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let h = host.join().unwrap().unwrap();
        assert_eq!(h.me(), PeerId(0));
        assert_eq!(c1.me(), PeerId(1));
        assert_eq!(c2.me(), PeerId(2));
        assert_eq!(h.players(), 3);
        assert_eq!(c1.players(), 3);
        assert_eq!(c2.players(), 3);
    }

    /// A bare socket that speaks the wire format by hand, after reading the hello.
    fn raw_client(addr: SocketAddr) -> TcpStream {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut hello = [0u8; 2];
        stream.read_exact(&mut hello).unwrap();
        assert_eq!(hello, [1, 2], "player 1 of 2");
        stream
    }

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn a_message_dribbled_in_one_byte_at_a_time_arrives_whole() {
        let (addr, host) = host_for(2);
        let mut raw = raw_client(addr);
        let mut host = host.join().unwrap().unwrap();
        let bytes = frame(b"dribbled, one byte at a time");
        let (last, body) = bytes.split_last().unwrap();
        for (i, byte) in body.iter().enumerate() {
            raw.write_all(&[*byte]).unwrap();
            raw.flush().unwrap();
            thread::sleep(Duration::from_millis(1));
            assert_eq!(
                host.poll().unwrap(),
                None,
                "a partial frame must not be returned (after {} bytes)",
                i + 1
            );
        }
        raw.write_all(&[*last]).unwrap();
        let got = wait_for("the dribbled message", || host.poll().unwrap());
        assert_eq!(got, (PeerId(1), b"dribbled, one byte at a time".to_vec()));
        assert_eq!(host.poll().unwrap(), None);
    }

    #[test]
    fn two_frames_in_one_write_come_out_as_two_polls() {
        let (addr, host) = host_for(2);
        let mut raw = raw_client(addr);
        let mut host = host.join().unwrap().unwrap();
        let mut both = frame(b"first");
        both.extend(frame(b"second"));
        raw.write_all(&both).unwrap();
        let first = wait_for("the first frame", || host.poll().unwrap());
        assert_eq!(first, (PeerId(1), b"first".to_vec()));
        let second = wait_for("the second frame", || host.poll().unwrap());
        assert_eq!(second, (PeerId(1), b"second".to_vec()));
    }

    #[test]
    fn megabytes_sent_while_the_other_side_is_not_reading_arrive_intact() {
        let (addr, host) = host_for(2);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        let mb = 1024 * 1024;
        let messages: Vec<Vec<u8>> = (0..3u8)
            .map(|i| (0..mb).map(|j| (j % 251) as u8 ^ i).collect())
            .collect();
        for message in &messages {
            // The host is not reading yet, so the socket fills and the rest waits in `out`.
            client.send(PeerId(0), message).unwrap();
        }
        let mut got = Vec::new();
        wait_for("all three megabyte messages", || {
            client.poll().unwrap();
            while let Some((_, bytes)) = host.poll().unwrap() {
                got.push(bytes);
            }
            (got.len() == messages.len()).then_some(())
        });
        assert!(got == messages, "the bytes changed in transit");
    }

    #[test]
    fn an_oversize_length_is_invalid_data_once_and_closes_that_peer() {
        let (addr, host) = host_for(2);
        let mut raw = raw_client(addr);
        let mut host = host.join().unwrap().unwrap();
        raw.write_all(&0x7fff_ffffu32.to_le_bytes()).unwrap();
        let err = wait_for("the bad length to be noticed", || host.poll().err());
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("peer 1"), "message: {err}");
        assert_eq!(
            host.poll().unwrap(),
            None,
            "the error is reported only once"
        );
        assert!(!host.is_open(PeerId(1)));
    }

    #[test]
    fn a_closed_client_is_not_open_but_its_earlier_messages_still_arrive() {
        let (addr, host) = host_for(2);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        client.send(PeerId(0), b"bye").unwrap();
        drop(client);
        let got = wait_for("the message sent before the close", || host.poll().unwrap());
        assert_eq!(got, (PeerId(1), b"bye".to_vec()));
        wait_for("the host to notice the close", || {
            host.poll().unwrap();
            (!host.is_open(PeerId(1))).then_some(())
        });
        let err = host.send(PeerId(1), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn joining_with_nothing_listening_fails_quickly() {
        let port = TcpListener::bind(LOCAL).unwrap().local_addr().unwrap();
        // Windows can take about 2 s to refuse a loopback connection, so the bound
        // is "well before the 10 s timeout", not "instant".
        let started = Instant::now();
        let err = Tcp::join(port, Duration::from_secs(10)).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionRefused);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
        assert!(err.to_string().contains("Host::accept"), "message: {err}");
    }

    #[test]
    fn accept_with_no_joiners_times_out_and_says_how_many_joined() {
        let host = Host::bind(LOCAL).unwrap();
        let err = host.accept(2, Duration::from_millis(50)).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(err.to_string().contains("only 1 of 2"), "message: {err}");
    }

    #[test]
    fn the_empty_message_round_trips() {
        let (addr, host) = host_for(2);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        client.send(PeerId(0), b"").unwrap();
        let got = wait_for("the empty message", || host.poll().unwrap());
        assert_eq!(got, (PeerId(1), Vec::new()));
    }

    #[test]
    fn sending_to_a_peer_you_are_not_connected_to_is_invalid_input() {
        let (addr, host) = host_for(3);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let _other = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        let err = client.send(PeerId(2), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("PeerId(0)"), "message: {err}");
        let err = host.send(PeerId(9), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn a_payload_over_one_mebibyte_is_refused_before_anything_is_sent() {
        let (addr, host) = host_for(2);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let _host = host.join().unwrap().unwrap();
        let err = client
            .send(PeerId(0), &vec![0u8; 1024 * 1024 + 1])
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn a_message_round_trips_between_host_and_client() {
        let (addr, host) = host_for(2);
        let mut client = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        client.send(PeerId(0), b"hi host").unwrap();
        let got = wait_for("the client's message", || host.poll().unwrap());
        assert_eq!(got, (PeerId(1), b"hi host".to_vec()));
        host.send(PeerId(1), b"hi client").unwrap();
        let got = wait_for("the host's reply", || client.poll().unwrap());
        assert_eq!(got, (PeerId(0), b"hi client".to_vec()));
    }

    #[test]
    fn a_chatty_client_does_not_starve_another() {
        let (addr, host) = host_for(3);
        let mut a = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut b = Tcp::join(addr, Duration::from_secs(5)).unwrap();
        let mut host = host.join().unwrap().unwrap();
        for i in 0..5u8 {
            a.send(PeerId(0), &[i]).unwrap();
            b.send(PeerId(0), &[i]).unwrap();
        }
        // wait until all ten messages (5 bytes each with the length) are buffered
        wait_for("all ten messages", || {
            let _ = host.poll_fill_for_test();
            let buffered = |p: usize| host.links[p].as_ref().unwrap().inbuf.unread().len();
            (buffered(1) >= 25 && buffered(2) >= 25).then_some(())
        });
        let order: Vec<u8> = (0..10)
            .map(|_| host.poll().unwrap().expect("a message").0 .0)
            .collect();
        assert!(
            order.windows(2).all(|pair| pair[0] != pair[1]),
            "the host should take turns between the clients, got {order:?}"
        );
    }
}

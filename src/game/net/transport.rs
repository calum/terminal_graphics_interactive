//! [`Transport`]: how peers send each other bytes, and [`Local`], an in-process
//! network for tests and for playing against yourself.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use super::PeerId;

/// Moves whole messages between peers. `send` queues bytes for a peer, `poll`
/// hands back the next message that has arrived, and neither blocks.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let mut net = Local::network(2);
/// let (mut a, mut b) = (net.remove(0), net.remove(0));
/// a.send(b.me(), b"hello").unwrap();
/// assert_eq!(b.poll().unwrap(), Some((a.me(), b"hello".to_vec())));
/// assert_eq!(b.poll().unwrap(), None); // nothing now; poll never blocks
/// ```
pub trait Transport {
    /// Queues `bytes` for `to`. Returns at once.
    fn send(&mut self, to: PeerId, bytes: &[u8]) -> io::Result<()>;

    /// The next message that has arrived, with its sender. `None` means there is
    /// nothing right now; call again later.
    fn poll(&mut self) -> io::Result<Option<(PeerId, Vec<u8>)>>;

    /// `false` once `peer` has gone away.
    fn is_open(&self, peer: PeerId) -> bool {
        let _ = peer;
        true
    }
}

/// Shared state of a [`Local`] network: one inbox per endpoint.
#[derive(Debug)]
struct Hub {
    inboxes: Vec<VecDeque<(PeerId, Vec<u8>)>>,
    open: Vec<bool>,
}

/// An in-process network: endpoints that talk through memory. Endpoint *i* is
/// `PeerId(i)`.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let net = Local::network(3);
/// assert_eq!(net[2].me(), PeerId(2));
/// ```
#[derive(Debug)]
pub struct Local {
    me: PeerId,
    hub: Arc<Mutex<Hub>>,
}

impl Local {
    /// `n` connected endpoints; endpoint `i` is `PeerId(i)`.
    pub fn network(n: usize) -> Vec<Local> {
        let hub = Arc::new(Mutex::new(Hub {
            inboxes: (0..n).map(|_| VecDeque::new()).collect(),
            open: vec![true; n],
        }));
        (0..n)
            .map(|i| Local {
                me: PeerId(i as u8),
                hub: Arc::clone(&hub),
            })
            .collect()
    }

    /// This endpoint's id.
    pub fn me(&self) -> PeerId {
        self.me
    }

    fn lock(&self) -> MutexGuard<'_, Hub> {
        // A panic elsewhere while holding the lock must not take the network down.
        self.hub.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Transport for Local {
    fn send(&mut self, to: PeerId, bytes: &[u8]) -> io::Result<()> {
        let from = self.me;
        let mut hub = self.lock();
        if to == from {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cannot send to yourself ({to:?}); a player keeps its own state"),
            ));
        }
        let Some(&open) = hub.open.get(to.0 as usize) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "no endpoint {to:?} in this network; use a PeerId below {}",
                    hub.open.len()
                ),
            ));
        };
        if !open {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("{to:?} has gone; stop sending to it and treat it as left the game"),
            ));
        }
        let inbox = &mut hub.inboxes[to.0 as usize];
        inbox.push_back((from, bytes.to_vec()));
        Ok(())
    }

    fn poll(&mut self) -> io::Result<Option<(PeerId, Vec<u8>)>> {
        let me = self.me.0 as usize;
        Ok(self.lock().inboxes[me].pop_front())
    }

    fn is_open(&self, peer: PeerId) -> bool {
        self.lock()
            .open
            .get(peer.0 as usize)
            .copied()
            .unwrap_or(false)
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        let me = self.me.0 as usize;
        self.lock().open[me] = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sending_to_yourself_or_to_an_unknown_peer_is_invalid_input() {
        let mut net = Local::network(2);
        let err = net[0].send(PeerId(0), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("yourself"), "message: {err}");
        let err = net[0].send(PeerId(9), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("PeerId(9)"), "message: {err}");
    }

    #[test]
    fn a_dropped_endpoint_is_closed_and_sending_to_it_is_broken_pipe() {
        let mut net = Local::network(2);
        let b = net.remove(1);
        let mut a = net.remove(0);
        assert!(a.is_open(PeerId(1)));
        drop(b);
        assert!(!a.is_open(PeerId(1)));
        let err = a.send(PeerId(1), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn messages_arrive_in_send_order_with_the_sender() {
        let mut net = Local::network(3);
        let mut c = net.remove(2);
        let mut b = net.remove(1);
        let mut a = net.remove(0);
        a.send(PeerId(2), b"a1").unwrap();
        b.send(PeerId(2), b"b1").unwrap();
        a.send(PeerId(2), b"a2").unwrap();
        assert_eq!(c.poll().unwrap(), Some((PeerId(0), b"a1".to_vec())));
        assert_eq!(c.poll().unwrap(), Some((PeerId(1), b"b1".to_vec())));
        assert_eq!(c.poll().unwrap(), Some((PeerId(0), b"a2".to_vec())));
        assert_eq!(c.poll().unwrap(), None);
    }

    #[test]
    fn a_message_is_copied_so_later_changes_to_the_source_do_not_show() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut a = net.remove(0);
        let mut buf = *b"abc";
        a.send(PeerId(1), &buf).unwrap();
        buf[0] = b'z';
        assert_eq!(buf[0], b'z');
        assert_eq!(b.poll().unwrap(), Some((PeerId(0), b"abc".to_vec())));
    }

    #[test]
    fn messages_queued_before_the_sender_left_are_still_delivered() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut a = net.remove(0);
        a.send(PeerId(1), b"last words").unwrap();
        drop(a);
        assert_eq!(b.poll().unwrap(), Some((PeerId(0), b"last words".to_vec())));
    }

    #[test]
    fn local_endpoints_can_move_to_other_threads() {
        fn assert_send<T: Send>() {}
        assert_send::<Local>();
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut a = net.remove(0);
        let handle = std::thread::spawn(move || a.send(PeerId(1), b"hi").unwrap());
        handle.join().unwrap();
        assert_eq!(b.poll().unwrap(), Some((PeerId(0), b"hi".to_vec())));
    }

    #[test]
    fn a_fresh_network_has_numbered_endpoints_and_nothing_to_read() {
        let mut net = Local::network(3);
        assert_eq!(net.len(), 3);
        for (i, endpoint) in net.iter_mut().enumerate() {
            assert_eq!(endpoint.me(), PeerId(i as u8));
            assert_eq!(endpoint.poll().unwrap(), None);
        }
    }
}

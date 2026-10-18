//! [`Lossy`]: a fake bad link for tests. It drops, duplicates and delays the
//! messages you send, deterministically from a seed, so the code above it can be
//! tested against loss and reordering without a real network.

use std::io;

use super::{PeerId, Transport};
use crate::game::Rng;

/// A copy of a message waiting for its release time.
struct Queued {
    due: u64,
    seq: u64,
    to: PeerId,
    bytes: Vec<u8>,
}

/// Wraps a transport and misbehaves on the send side.
pub(crate) struct Lossy<T: Transport> {
    inner: T,
    rng: Rng,
    drop_p: f32,
    dup_p: f32,
    max_delay: u32,
    now: u64,
    seq: u64,
    queue: Vec<Queued>,
    sent: usize,
    dropped: usize,
}

impl<T: Transport> Lossy<T> {
    /// Wraps `inner` with no faults; the seed decides every random choice.
    pub(crate) fn new(inner: T, seed: u64) -> Lossy<T> {
        Lossy {
            inner,
            rng: Rng::new(seed),
            drop_p: 0.0,
            dup_p: 0.0,
            max_delay: 0,
            now: 0,
            seq: 0,
            queue: Vec::new(),
            sent: 0,
            dropped: 0,
        }
    }

    /// Each sent message is lost with probability `p` (clamped to 0..=1).
    #[allow(clippy::should_implement_trait)]
    pub(crate) fn drop(mut self, p: f32) -> Lossy<T> {
        self.drop_p = p.clamp(0.0, 1.0);
        self
    }

    /// Each sent message is delivered twice with probability `p` (clamped to 0..=1).
    pub(crate) fn duplicate(mut self, p: f32) -> Lossy<T> {
        self.dup_p = p.clamp(0.0, 1.0);
        self
    }

    /// Each copy of a message is held for a random number of polls in
    /// `0..=max_polls`, so messages can overtake each other.
    pub(crate) fn delay(mut self, max_polls: u32) -> Lossy<T> {
        self.max_delay = max_polls;
        self
    }

    /// Forwards every queued message whose release time has come, earliest first.
    fn flush_due(&mut self) -> io::Result<()> {
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.queue.len() {
            if self.queue[i].due <= self.now {
                ready.push(self.queue.swap_remove(i));
            } else {
                i += 1;
            }
        }
        ready.sort_by_key(|q| (q.due, q.seq));
        for q in ready {
            self.inner.send(q.to, &q.bytes)?;
        }
        Ok(())
    }

    /// Releases every held message now, whatever its release time.
    pub(crate) fn flush_all(&mut self) -> io::Result<()> {
        let mut all = std::mem::take(&mut self.queue);
        all.sort_by_key(|q| (q.due, q.seq));
        for q in all {
            self.inner.send(q.to, &q.bytes)?;
        }
        Ok(())
    }

    /// The wrapped transport, for assertions.
    pub(crate) fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Messages sent so far, including dropped ones.
    pub(crate) fn sent_count(&self) -> usize {
        self.sent
    }

    /// Messages lost so far.
    pub(crate) fn dropped_count(&self) -> usize {
        self.dropped
    }
}

impl<T: Transport> Transport for Lossy<T> {
    fn send(&mut self, to: PeerId, bytes: &[u8]) -> io::Result<()> {
        self.sent += 1;
        if self.rng.chance(self.drop_p) {
            self.dropped += 1;
            return Ok(());
        }
        let copies = if self.rng.chance(self.dup_p) { 2 } else { 1 };
        let span = i32::try_from(self.max_delay).unwrap_or(i32::MAX - 1) + 1;
        for _ in 0..copies {
            let wait = self.rng.range_i32(0, span) as u64;
            self.seq += 1;
            self.queue.push(Queued {
                due: self.now + wait,
                seq: self.seq,
                to,
                bytes: bytes.to_vec(),
            });
        }
        self.flush_due()
    }

    fn poll(&mut self) -> io::Result<Option<(PeerId, Vec<u8>)>> {
        self.now += 1;
        self.flush_due()?;
        self.inner.poll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::net::{Local, PeerId, Transport};

    #[test]
    fn with_no_faults_it_is_transparent() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let a = net.remove(0);
        let mut lossy = Lossy::new(a, 1);
        for i in 0..5u8 {
            lossy.send(PeerId(1), &[i]).unwrap();
        }
        for i in 0..5u8 {
            assert_eq!(b.poll().unwrap(), Some((PeerId(0), vec![i])));
        }
        assert_eq!(b.poll().unwrap(), None);
    }

    #[test]
    fn duplicating_with_probability_one_delivers_each_twice() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut lossy = Lossy::new(net.remove(0), 1).duplicate(1.0);
        lossy.send(PeerId(1), b"a").unwrap();
        lossy.send(PeerId(1), b"b").unwrap();
        let mut got = Vec::new();
        while let Some((_, bytes)) = b.poll().unwrap() {
            got.push(bytes);
        }
        assert_eq!(
            got,
            vec![b"a".to_vec(), b"a".to_vec(), b"b".to_vec(), b"b".to_vec()]
        );
    }

    /// Sends 200 numbered messages through a delayed link and returns the order
    /// in which they arrived at the far end.
    fn arrival_order(seed: u64) -> Vec<u8> {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut lossy = Lossy::new(net.remove(0), seed).delay(5);
        for i in 0..200u8 {
            lossy.send(PeerId(1), &[i]).unwrap();
        }
        let mut arrived = Vec::new();
        for _ in 0..1000 {
            lossy.poll().unwrap();
            while let Some((_, bytes)) = b.poll().unwrap() {
                arrived.push(bytes[0]);
            }
            if arrived.len() == 200 {
                return arrived;
            }
        }
        panic!("only {} of 200 arrived after 1000 polls", arrived.len());
    }

    #[test]
    fn delayed_messages_all_arrive_but_can_reorder_deterministically() {
        let order = arrival_order(7);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..200u8).collect::<Vec<_>>(),
            "every message arrives once"
        );
        assert_ne!(order, sorted, "some pair arrives out of send order");
        assert_eq!(
            arrival_order(7),
            order,
            "the same seed gives the same arrivals"
        );
    }

    #[test]
    fn flush_all_releases_every_held_message_now() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        // Release times are random in 0..=1000, so most copies are still held.
        let mut lossy = Lossy::new(net.remove(0), 3).delay(1000);
        for i in 0..50u8 {
            lossy.send(PeerId(1), &[i]).unwrap();
        }
        let mut arrived = Vec::new();
        while let Some((_, bytes)) = b.poll().unwrap() {
            arrived.push(bytes[0]);
        }
        assert!(
            arrived.len() < 50,
            "some copies must still be held before the flush"
        );
        lossy.flush_all().unwrap();
        while let Some((_, bytes)) = b.poll().unwrap() {
            arrived.push(bytes[0]);
        }
        arrived.sort_unstable();
        assert_eq!(arrived, (0..50u8).collect::<Vec<_>>());
    }

    #[test]
    fn inner_mut_gives_back_the_wrapped_transport() {
        let mut net = Local::network(2);
        let mut lossy = Lossy::new(net.remove(0), 1);
        assert_eq!(lossy.inner_mut().me(), PeerId(0));
    }

    #[test]
    fn half_the_messages_are_dropped_at_probability_one_half() {
        let mut net = Local::network(2);
        let _b = net.remove(1);
        let mut lossy = Lossy::new(net.remove(0), 42).drop(0.5);
        for _ in 0..1000 {
            lossy.send(PeerId(1), b"x").unwrap();
        }
        assert_eq!(lossy.sent_count(), 1000);
        let dropped = lossy.dropped_count();
        assert!((400..=600).contains(&dropped), "dropped {dropped} of 1000");
    }

    #[test]
    fn dropping_with_probability_one_delivers_nothing() {
        let mut net = Local::network(2);
        let mut b = net.remove(1);
        let mut lossy = Lossy::new(net.remove(0), 1).drop(1.0);
        for _ in 0..10 {
            lossy.send(PeerId(1), b"x").unwrap();
        }
        assert_eq!(lossy.sent_count(), 10);
        assert_eq!(lossy.dropped_count(), 10);
        assert_eq!(b.poll().unwrap(), None);
    }
}

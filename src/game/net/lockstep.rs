//! [`Lockstep`]: several machines play one [`Sim`] by sharing only inputs.

use super::{Decode, Encode, Hasher, PeerId, PlayerId, Reader, Sim, Transport};
use crate::game::Rng;
use std::collections::VecDeque;
use std::io;

/// Message kinds on the wire: a window of recent inputs, a state hash, goodbye.
const KIND_INPUTS: u8 = 1;
const KIND_HASH: u8 = 2;
const KIND_BYE: u8 = 3;

/// How many of the latest inputs go in every packet. Sending the same input
/// several times lets a link that loses or reorders packets heal itself.
const RESEND: usize = 8;

/// How many steps beyond the input delay a machine may produce inputs ahead
/// of what has run.
const MAX_AHEAD: u32 = 8;

/// The most steps one `tick` runs, so catching up after a stall is bounded.
const CATCH_UP: u32 = 4;

/// Inputs for steps further ahead than this are ignored.
const WINDOW: u32 = 512;

/// How many hashes, ours and theirs, are kept for comparing.
const HISTORY: usize = 64;

/// How a lockstep match is set up. Every machine in a match must use the same
/// `players`, `seed`, `delay`, `hash_every` and `star`; only `me` differs.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let config = Config::new(2, PlayerId(1)).seed(7).delay(4);
/// assert_eq!(config.players(), 2);
/// assert_eq!(config.me(), PlayerId(1));
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    players: usize,
    me: PlayerId,
    seed: u64,
    delay: u32,
    hash_every: u32,
    stall_limit: u32,
    star: bool,
}

impl Config {
    /// A match of `players` in which this machine is player `me`. The seed is
    /// 1, the input delay 3 steps (50 ms), hashes are compared every 60 steps,
    /// 600 steps (10 s) of waiting means a player is lost, and the network is
    /// a star around player 0 (what [`Tcp`](super::Tcp) makes).
    pub fn new(players: usize, me: PlayerId) -> Config {
        Config {
            players: players.clamp(1, 255),
            me,
            seed: 1,
            delay: 3,
            hash_every: 60,
            stall_limit: 600,
            star: true,
        }
    }

    /// The seed of the shared random numbers. The host chooses it and tells
    /// the others.
    pub fn seed(mut self, seed: u64) -> Config {
        self.seed = seed;
        self
    }

    /// How many steps an input waits before it takes effect. More delay hides
    /// a slower network and makes the controls feel later.
    pub fn delay(mut self, steps: u32) -> Config {
        self.delay = steps.min(MAX_AHEAD * 4);
        self
    }

    /// How often, in steps, machines compare the hash of their state. 0 turns
    /// the comparison off.
    pub fn hash_every(mut self, steps: u32) -> Config {
        self.hash_every = steps;
        self
    }

    /// How many `tick` calls in a row may wait for a player before they are
    /// reported lost.
    pub fn stall_limit(mut self, ticks: u32) -> Config {
        self.stall_limit = ticks;
        self
    }

    /// `true` (the default) when the clients only talk to player 0, which
    /// passes their messages on; `false` when every machine talks to every
    /// other, as with [`Local`](super::Local).
    pub fn star(mut self, star: bool) -> Config {
        self.star = star;
        self
    }

    /// How many players are in the match.
    pub fn players(&self) -> usize {
        self.players
    }

    /// Which player this machine is.
    pub fn me(&self) -> PlayerId {
        self.me
    }
}

/// Two machines computed different states for the same step.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let desync = Desync { tick: 180, last_agreed: 120, player: PlayerId(1), mine: 1, theirs: 2 };
/// assert!(desync.to_string().contains("between step 120 and step 180"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Desync {
    /// The checkpoint at which the hashes differed: after this many steps.
    pub tick: u32,
    /// The checkpoint before it, at which they still agreed (0 is the start).
    pub last_agreed: u32,
    /// The player whose hash differed from ours.
    pub player: PlayerId,
    /// Our hash.
    pub mine: u64,
    /// Their hash.
    pub theirs: u64,
}

impl std::fmt::Display for Desync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "player {} and this machine disagree between step {} and step {} \
             (state hashes {:016x} and {:016x}); the simulation is not deterministic",
            self.player.0 + 1,
            self.last_agreed,
            self.tick,
            self.theirs,
            self.mine
        )
    }
}

/// What a [`Lockstep::tick`] achieved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// This many steps ran (1 to 4). More than one means catching up.
    Advanced(u32),
    /// No step could run because these players' inputs have not arrived. Show
    /// "waiting for player 2" and keep calling `tick`.
    Stalled {
        /// The players whose input for the next step is missing.
        waiting_for: Vec<PlayerId>,
    },
    /// The machines disagree. The match cannot continue; this stays the answer.
    Desync(Desync),
    /// A player left, or did not answer for too long.
    Lost(PlayerId),
}

/// Runs one [`Sim`] in step with other machines, over a [`Transport`], by
/// sharing only each player's input.
///
/// Every player's input for step *t* is decided *delay* steps earlier, so there
/// is time for it to cross the network. A step runs when all players' inputs
/// for it have arrived; if one is late, the match waits ([`Progress::Stalled`]).
/// Every `hash_every` steps the machines compare a hash of their state and
/// report a [`Desync`] if it differs.
///
/// Call [`tick`](Lockstep::tick) once per update (60 times a second) with the
/// local player's input.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// struct Walk { at: Vec<i32> }
/// impl Sim for Walk {
///     type Input = i32;
///     fn step(&mut self, inputs: &[i32], _rng: &mut Rng) {
///         for (at, input) in self.at.iter_mut().zip(inputs) { *at += input; }
///     }
///     fn hash(&self, h: &mut Hasher) { for at in &self.at { h.i32(*at); } }
/// }
///
/// let mut ends = Local::network(2).into_iter();
/// let (a, b) = (ends.next().unwrap(), ends.next().unwrap());
/// let mut one = Lockstep::new(Walk { at: vec![0; 2] }, a, Config::new(2, PlayerId(0)).star(false));
/// let mut two = Lockstep::new(Walk { at: vec![0; 2] }, b, Config::new(2, PlayerId(1)).star(false));
/// for _ in 0..30 {
///     one.tick(1, |_| {}).unwrap(); // player 1 walks right
///     two.tick(-1, |_| {}).unwrap(); // player 2 walks left
/// }
/// assert!(one.sim().at[0] > 0 && one.sim().at[1] < 0);
/// ```
pub struct Lockstep<S: Sim, T: Transport> {
    sim: S,
    transport: T,
    rng: Rng,
    config: Config,
    /// The next step to run.
    executed: u32,
    /// The next step to produce our own input for.
    produced: u32,
    /// `slots[i]` holds every player's input for step `executed + i`.
    slots: VecDeque<Vec<Option<S::Input>>>,
    /// Our latest inputs, resent in every packet.
    recent: VecDeque<(u32, S::Input)>,
    own_hashes: VecDeque<(u32, u64)>,
    pending_hashes: VecDeque<(PlayerId, u32, u64)>,
    gone: Vec<bool>,
    desync: Option<Desync>,
    stalled_for: u32,
    scratch: Vec<S::Input>,
    packet: Vec<u8>,
}

impl<S: Sim, T: Transport> Lockstep<S, T> {
    /// Starts a match with `sim` in its starting state, which every machine
    /// must build the same way.
    pub fn new(sim: S, transport: T, config: Config) -> Lockstep<S, T> {
        let players = config.players;
        let mut slots = VecDeque::new();
        for _ in 0..config.delay {
            slots.push_back(vec![Some(S::Input::default()); players]);
        }
        Lockstep {
            sim,
            transport,
            rng: Rng::new(config.seed),
            executed: 0,
            produced: config.delay,
            slots,
            recent: VecDeque::new(),
            own_hashes: VecDeque::new(),
            pending_hashes: VecDeque::new(),
            gone: vec![false; players],
            desync: None,
            stalled_for: 0,
            scratch: Vec::with_capacity(players),
            packet: Vec::new(),
            config,
        }
    }

    /// The simulation as it is now.
    pub fn sim(&self) -> &S {
        &self.sim
    }

    /// How many steps have run.
    pub fn executed(&self) -> u32 {
        self.executed
    }

    /// The state hashes of the checkpoints reached so far, newest last.
    #[cfg(test)]
    fn checkpoints(&self) -> &VecDeque<(u32, u64)> {
        &self.own_hashes
    }

    /// The match settings.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// A hash of the simulation, the shared random numbers and the step
    /// count: equal on two machines exactly when they agree.
    pub fn state_hash(&self) -> u64 {
        let mut h = Hasher::new();
        self.sim.hash(&mut h);
        h.u64(self.rng.state()).u32(self.executed);
        h.finish()
    }

    /// One update: receives what has arrived, sends `local` as this player's
    /// input for a step `delay` steps from now, and runs every step that is
    /// ready (at most 4). `after_step` is called with the simulation after each
    /// step that runs, for effects that react to what just happened.
    pub fn tick(
        &mut self,
        local: S::Input,
        mut after_step: impl FnMut(&S),
    ) -> io::Result<Progress> {
        if let Some(desync) = &self.desync {
            return Ok(Progress::Desync(desync.clone()));
        }
        self.receive()?;
        self.produce(local)?;
        let ran = self.run(&mut after_step)?;
        if let Some(desync) = &self.desync {
            return Ok(Progress::Desync(desync.clone()));
        }
        if ran > 0 {
            self.stalled_for = 0;
            return Ok(Progress::Advanced(ran));
        }
        self.stalled_for += 1;
        let waiting_for = self.waiting_for();
        for &player in &waiting_for {
            if self.is_gone(player) {
                return Ok(Progress::Lost(player));
            }
        }
        if self.stalled_for > self.config.stall_limit {
            if let Some(&first) = waiting_for.first() {
                return Ok(Progress::Lost(first));
            }
        }
        Ok(Progress::Stalled { waiting_for })
    }

    /// Tells the other players this one is leaving, so they stop waiting.
    pub fn leave(&mut self) -> io::Result<()> {
        let bytes = [KIND_BYE, self.config.me.0];
        self.send_all(&bytes, None)
    }

    /// The transport, for tests that need to poke at it.
    #[cfg(test)]
    fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Gives the transport back, for example to reuse the connection.
    pub fn into_transport(self) -> T {
        self.transport
    }

    fn is_gone(&self, player: PlayerId) -> bool {
        self.gone[player.index()] || !self.transport.is_open(self.route(player))
    }

    /// The peer to send to in order to reach `player`.
    fn route(&self, player: PlayerId) -> PeerId {
        if self.config.star && self.config.me.0 != 0 {
            PeerId(0)
        } else {
            PeerId::from(player)
        }
    }

    fn waiting_for(&self) -> Vec<PlayerId> {
        (0..self.config.players)
            .filter(|&p| p != self.config.me.index())
            .filter(|&p| match self.slots.front() {
                Some(row) => row[p].is_none(),
                None => true,
            })
            .filter_map(PlayerId::from_index)
            .collect()
    }

    /// Sends to every peer this machine talks to, except `except`.
    fn send_all(&mut self, bytes: &[u8], except: Option<PeerId>) -> io::Result<()> {
        let me = self.config.me.0;
        let hub_only = self.config.star && me != 0;
        for peer in 0..self.config.players {
            let peer = PeerId(peer as u8);
            if peer.0 == me || Some(peer) == except || (hub_only && peer.0 != 0) {
                continue;
            }
            match self.transport.send(peer, bytes) {
                Ok(()) => {}
                // a player who left is found out when their input is missed
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn receive(&mut self) -> io::Result<()> {
        while let Some((peer, bytes)) = self.transport.poll()? {
            self.handle(peer, &bytes);
        }
        Ok(())
    }

    /// Whether a message about `player` may arrive from `peer`.
    fn allowed(&self, player: usize, peer: PeerId) -> bool {
        player < self.config.players
            && player != self.config.me.index()
            && (player == peer.0 as usize || (self.config.star && peer.0 == 0))
    }

    /// Reads one message. Anything wrong with it is ignored: a stranger or a
    /// broken peer must not be able to stop the match with bad bytes.
    fn handle(&mut self, peer: PeerId, bytes: &[u8]) {
        let mut r = Reader::new(bytes);
        let Ok(kind) = r.u8() else { return };
        let Ok(player) = r.u8() else { return };
        let player = player as usize;
        if !self.allowed(player, peer) {
            return;
        }
        let ok = match kind {
            KIND_INPUTS => self.handle_inputs(player, &mut r),
            KIND_HASH => self.handle_hash(player, &mut r),
            KIND_BYE => {
                self.gone[player] = true;
                true
            }
            _ => false,
        };
        // the hub passes on what its clients say, so clients hear each other
        if ok && self.config.star && self.config.me.0 == 0 && player == peer.0 as usize {
            let _ = self.send_all(bytes, Some(peer));
        }
    }

    fn handle_inputs(&mut self, player: usize, r: &mut Reader<'_>) -> bool {
        let (Ok(from), Ok(count)) = (r.u32(), r.u8()) else {
            return false;
        };
        let mut inputs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            match S::Input::decode(r) {
                Ok(input) => inputs.push(input),
                Err(_) => return false,
            }
        }
        for (i, input) in inputs.into_iter().enumerate() {
            let tick = from.wrapping_add(i as u32);
            if tick < self.executed || tick - self.executed >= WINDOW {
                continue;
            }
            let row = self.row((tick - self.executed) as usize);
            if row[player].is_none() {
                row[player] = Some(input);
            }
        }
        true
    }

    fn handle_hash(&mut self, player: usize, r: &mut Reader<'_>) -> bool {
        let (Ok(tick), Ok(theirs)) = (r.u32(), r.u64()) else {
            return false;
        };
        let player = PlayerId(player as u8);
        if let Some(&(_, mine)) = self.own_hashes.iter().find(|(t, _)| *t == tick) {
            self.compare(tick, player, mine, theirs);
        } else if tick > self.executed {
            if self.pending_hashes.len() == HISTORY {
                self.pending_hashes.pop_front();
            }
            self.pending_hashes.push_back((player, tick, theirs));
        }
        true
    }

    fn compare(&mut self, tick: u32, player: PlayerId, mine: u64, theirs: u64) {
        if mine != theirs && self.desync.is_none() {
            self.desync = Some(Desync {
                tick,
                last_agreed: tick.saturating_sub(self.config.hash_every),
                player,
                mine,
                theirs,
            });
        }
    }

    /// The row of inputs for the step `ahead` steps from the next to run.
    fn row(&mut self, ahead: usize) -> &mut Vec<Option<S::Input>> {
        let players = self.config.players;
        while self.slots.len() <= ahead {
            self.slots.push_back(vec![None; players]);
        }
        &mut self.slots[ahead]
    }

    /// Records our input for a future step, and sends our latest inputs.
    fn produce(&mut self, local: S::Input) -> io::Result<()> {
        if self.produced < self.executed + self.config.delay + MAX_AHEAD {
            let ahead = (self.produced - self.executed) as usize;
            let me = self.config.me.index();
            self.row(ahead)[me] = Some(local.clone());
            self.recent.push_back((self.produced, local));
            if self.recent.len() > RESEND {
                self.recent.pop_front();
            }
            self.produced += 1;
        }
        if self.config.players > 1 {
            if let Some(&(first, _)) = self.recent.front() {
                let mut packet = std::mem::take(&mut self.packet);
                packet.clear();
                packet.extend_from_slice(&[KIND_INPUTS, self.config.me.0]);
                first.encode(&mut packet);
                packet.push(self.recent.len() as u8);
                for (_, input) in &self.recent {
                    input.encode(&mut packet);
                }
                let sent = self.send_all(&packet, None);
                self.packet = packet;
                sent?;
            }
        }
        Ok(())
    }

    /// Runs the steps whose inputs are all here, at most [`CATCH_UP`].
    fn run(&mut self, after_step: &mut impl FnMut(&S)) -> io::Result<u32> {
        let mut ran = 0;
        while ran < CATCH_UP && self.desync.is_none() {
            let ready = matches!(self.slots.front(), Some(row) if row.iter().all(Option::is_some));
            if !ready {
                break;
            }
            let row = self.slots.pop_front().unwrap_or_default();
            self.scratch.clear();
            self.scratch.extend(row.into_iter().flatten());
            self.sim.step(&self.scratch, &mut self.rng);
            self.executed += 1;
            ran += 1;
            after_step(&self.sim);
            let every = self.config.hash_every;
            if every > 0 && self.executed % every == 0 {
                self.checkpoint()?;
            }
        }
        Ok(ran)
    }

    /// Hashes our state, tells the others, and checks hashes that came early.
    fn checkpoint(&mut self) -> io::Result<()> {
        let tick = self.executed;
        let mine = self.state_hash();
        if self.own_hashes.len() == HISTORY {
            self.own_hashes.pop_front();
        }
        self.own_hashes.push_back((tick, mine));
        if self.config.players > 1 {
            let mut packet = vec![KIND_HASH, self.config.me.0];
            tick.encode(&mut packet);
            mine.encode(&mut packet);
            self.send_all(&packet, None)?;
        }
        let mut i = 0;
        while i < self.pending_hashes.len() {
            let (player, at, theirs) = self.pending_hashes[i];
            if at == tick {
                self.pending_hashes.remove(i);
                self.compare(tick, player, mine, theirs);
            } else {
                i += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::lossy::Lossy;
    use super::super::{Hasher, Local, PeerId, PlayerId, Sim, Transport};
    use super::*;
    use crate::game::Rng;

    /// Each player walks along a line; the random numbers add some noise.
    #[derive(Clone)]
    struct Walk {
        at: Vec<i32>,
        noise: u64,
        steps: u32,
        /// A bug on purpose: on this step, a player jumps by 1000.
        bug_at: Option<u32>,
    }

    impl Walk {
        fn new(players: usize) -> Walk {
            Walk {
                at: vec![0; players],
                noise: 0,
                steps: 0,
                bug_at: None,
            }
        }
    }

    impl Sim for Walk {
        type Input = i32;
        fn step(&mut self, inputs: &[i32], rng: &mut Rng) {
            for (at, input) in self.at.iter_mut().zip(inputs) {
                *at += input;
            }
            self.noise ^= rng.next_u64();
            if self.bug_at == Some(self.steps) {
                self.at[0] += 1000;
            }
            self.steps += 1;
        }
        fn hash(&self, h: &mut Hasher) {
            for at in &self.at {
                h.i32(*at);
            }
            h.u64(self.noise);
        }
    }

    fn pair(delay: u32) -> (Lockstep<Walk, Local>, Lockstep<Walk, Local>) {
        let mut ends = Local::network(2).into_iter();
        let config = |me| Config::new(2, PlayerId(me)).seed(42).delay(delay);
        (
            Lockstep::new(Walk::new(2), ends.next().unwrap(), config(0)),
            Lockstep::new(Walk::new(2), ends.next().unwrap(), config(1)),
        )
    }

    #[test]
    fn two_machines_that_swap_inputs_end_in_the_same_state() {
        let (mut a, mut b) = pair(3);
        for frame in 0..300 {
            a.tick((frame % 5) - 2, |_| {}).unwrap();
            b.tick((frame % 7) - 3, |_| {}).unwrap();
        }
        assert!(a.executed() >= 295, "executed {}", a.executed());
        assert!(a.executed().abs_diff(b.executed()) <= 1);
        while a.executed() < b.executed() {
            a.tick(0, |_| {}).unwrap();
        }
        assert_eq!(a.state_hash(), b.state_hash());
        // player 1's input of frame f takes effect at step f + 3
        let expected: i32 = (0..a.executed() as i32 - 3).map(|f| (f % 5) - 2).sum();
        assert_eq!(a.sim().at[0], expected);
        let expected: i32 = (0..a.executed() as i32 - 3).map(|f| (f % 7) - 3).sum();
        assert_eq!(a.sim().at[1], expected);
        assert_ne!(a.sim().noise, 0);
    }

    /// Runs `frames` updates on every machine in turn, feeding player `p` the
    /// input `input(p, frame)`. Returns the last result of each.
    fn run<T: Transport>(
        peers: &mut [Lockstep<Walk, T>],
        frames: i32,
        input: impl Fn(usize, i32) -> i32,
    ) -> Vec<Progress> {
        let mut last = Vec::new();
        for frame in 0..frames {
            last.clear();
            for (p, peer) in peers.iter_mut().enumerate() {
                last.push(peer.tick(input(p, frame), |_| {}).unwrap());
            }
        }
        last
    }

    /// Checks that the machines are within a few steps of each other and agree
    /// on the state at every checkpoint they have both reached.
    fn settle<T: Transport>(peers: &mut [Lockstep<Walk, T>]) {
        let most = peers.iter().map(|p| p.executed()).max().unwrap();
        let least = peers.iter().map(|p| p.executed()).min().unwrap();
        assert!(most - least <= 12, "steps {least} to {most}");
        let mut shared = 0;
        for &(tick, hash) in peers[0].checkpoints() {
            for other in peers.iter().skip(1) {
                if let Some(&(_, theirs)) = other.checkpoints().iter().find(|(t, _)| *t == tick) {
                    assert_eq!(hash, theirs, "the machines disagree at step {tick}");
                    shared += 1;
                }
            }
        }
        assert!(shared > 0, "no checkpoint was reached by both");
    }

    #[test]
    fn an_input_takes_effect_exactly_delay_steps_later() {
        for delay in [0, 1, 3, 6] {
            let (mut a, mut b) = pair(delay);
            let mut steps_seen = 0;
            let mut first_move = None;
            for frame in 0..40 {
                let press = if frame == 0 { 5 } else { 0 };
                a.tick(press, |sim| {
                    steps_seen += 1;
                    if sim.at[0] != 0 && first_move.is_none() {
                        first_move = Some(steps_seen);
                    }
                })
                .unwrap();
                b.tick(0, |_| {}).unwrap();
            }
            // the input of the first frame is step `delay` (counting from 0),
            // so it is the (delay + 1)th step to run
            assert_eq!(first_move, Some(delay + 1), "delay {delay}");
        }
    }

    #[test]
    fn a_silent_machine_stalls_the_other_and_the_match_resumes_when_it_returns() {
        let (mut a, b) = pair(3);
        let mut last = Progress::Advanced(0);
        for _ in 0..50 {
            last = a.tick(1, |_| {}).unwrap();
        }
        assert_eq!(
            last,
            Progress::Stalled {
                waiting_for: vec![PlayerId(1)]
            }
        );
        assert!(a.executed() <= 3, "ran {} steps alone", a.executed());
        let mut peers = [a, b];
        run(&mut peers, 60, |p, _| if p == 0 { 1 } else { -1 });
        settle(&mut peers);
        assert!(peers[0].executed() > 50);
        assert!(peers[0].sim().at[1] < 0, "player 2's inputs arrived");
    }

    #[test]
    fn catching_up_after_a_burst_of_late_messages_runs_at_most_four_steps_per_tick() {
        let mut ends = Local::network(2).into_iter();
        let config = |me| Config::new(2, PlayerId(me)).seed(1);
        // everything player 1 sends is held back by a link with a long delay
        let slow = Lossy::new(ends.next().unwrap(), 1).delay(100_000);
        let mut a = Lockstep::new(Walk::new(2), slow, config(0));
        let mut b = Lockstep::new(Walk::new(2), ends.next().unwrap(), config(1));
        for _ in 0..10 {
            a.tick(1, |_| {}).unwrap();
            b.tick(1, |_| {}).unwrap();
        }
        assert!(
            b.executed() <= 4,
            "b should be waiting, ran {}",
            b.executed()
        );
        a.transport_mut().flush_all().unwrap();
        let mut ran = Vec::new();
        for _ in 0..6 {
            if let Progress::Advanced(n) = b.tick(1, |_| {}).unwrap() {
                ran.push(n);
            }
        }
        assert!(ran.iter().all(|&n| (1..=4).contains(&n)), "{ran:?}");
        assert!(ran.iter().any(|&n| n > 1), "it should catch up: {ran:?}");
    }

    #[test]
    fn a_link_that_drops_duplicates_and_reorders_still_keeps_the_machines_in_step() {
        let mut ends = Local::network(2).into_iter();
        let lossy = |end, seed| Lossy::new(end, seed).drop(0.25).duplicate(0.2).delay(5);
        let config = |me| Config::new(2, PlayerId(me)).seed(9);
        let mut peers = [
            Lockstep::new(Walk::new(2), lossy(ends.next().unwrap(), 1), config(0)),
            Lockstep::new(Walk::new(2), lossy(ends.next().unwrap(), 2), config(1)),
        ];
        let last = run(&mut peers, 1500, |p, f| {
            ((f * (p as i32 + 2)) % 5) - 2 + p as i32
        });
        assert!(!matches!(last[0], Progress::Desync(_)), "{last:?}");
        settle(&mut peers);
        assert!(
            peers[0].executed() > 1000,
            "only {} steps ran in 1500 frames",
            peers[0].executed()
        );
        assert_ne!(peers[0].sim().at[0], peers[0].sim().at[1]);
    }

    #[test]
    fn a_simulation_that_drifts_is_reported_at_the_checkpoint_with_the_last_good_one() {
        let mut ends = Local::network(2).into_iter();
        let config = |me| Config::new(2, PlayerId(me)).seed(5).hash_every(60);
        let mut cheat = Walk::new(2);
        cheat.bug_at = Some(130);
        let mut a = Lockstep::new(Walk::new(2), ends.next().unwrap(), config(0));
        let mut b = Lockstep::new(cheat, ends.next().unwrap(), config(1));
        let mut found = None;
        for _ in 0..400 {
            let ra = a.tick(1, |_| {}).unwrap();
            let rb = b.tick(1, |_| {}).unwrap();
            if let (Progress::Desync(da), Progress::Desync(db)) = (ra, rb) {
                found = Some((da, db));
                break;
            }
        }
        let (da, db) = found.expect("both machines should notice");
        assert_eq!((da.tick, da.last_agreed), (180, 120));
        assert_eq!((db.tick, db.last_agreed), (180, 120));
        assert_eq!(da.player, PlayerId(1));
        assert_ne!(da.mine, da.theirs);
        assert!(da.to_string().contains("between step 120 and step 180"));
        // it stays the answer, and nothing more runs
        let steps = a.executed();
        assert!(matches!(a.tick(1, |_| {}).unwrap(), Progress::Desync(_)));
        assert_eq!(a.executed(), steps);
    }

    #[test]
    fn machines_that_agree_report_no_desync_over_many_checkpoints() {
        let (mut a, mut b) = pair(3);
        for frame in 0..1000 {
            for lock in [&mut a, &mut b] {
                let p = lock.tick(frame % 3, |_| {}).unwrap();
                assert!(!matches!(p, Progress::Desync(_)), "{p:?}");
            }
        }
    }

    #[test]
    fn a_player_who_leaves_is_reported_lost() {
        let (mut a, b) = pair(3);
        drop(b);
        let mut last = Progress::Advanced(0);
        for _ in 0..10 {
            last = a.tick(1, |_| {}).unwrap();
        }
        assert_eq!(last, Progress::Lost(PlayerId(1)));
    }

    #[test]
    fn a_player_who_says_goodbye_is_reported_lost() {
        let (mut a, mut b) = pair(3);
        a.tick(1, |_| {}).unwrap();
        b.tick(1, |_| {}).unwrap();
        b.leave().unwrap();
        let mut last = Progress::Advanced(0);
        for _ in 0..30 {
            last = a.tick(1, |_| {}).unwrap();
        }
        assert_eq!(last, Progress::Lost(PlayerId(1)));
    }

    #[test]
    fn waiting_too_long_for_a_silent_player_is_reported_lost() {
        let mut ends = Local::network(2).into_iter();
        let config = Config::new(2, PlayerId(0)).stall_limit(100);
        let mut a = Lockstep::new(Walk::new(2), ends.next().unwrap(), config);
        let _quiet = ends.next().unwrap(); // connected, but never ticks
        let mut results = Vec::new();
        for _ in 0..105 {
            results.push(a.tick(1, |_| {}).unwrap());
        }
        assert!(matches!(results[99], Progress::Stalled { .. }));
        assert_eq!(results[104], Progress::Lost(PlayerId(1)));
    }

    #[test]
    fn three_machines_in_a_star_hear_each_other_through_the_host() {
        let mut ends = Local::network(3).into_iter();
        let config = |me| Config::new(3, PlayerId(me)).seed(3);
        let mut peers: Vec<_> = (0..3)
            .map(|me| Lockstep::new(Walk::new(3), ends.next().unwrap(), config(me)))
            .collect();
        run(&mut peers, 400, |p, f| ((f + p as i32) % 3) - 1 + p as i32);
        settle(&mut peers);
        assert!(peers[1].sim().at[2] != 0, "client 1 heard client 2");
        assert!(peers[2].sim().at[1] != 0, "client 2 heard client 1");
        assert_eq!(peers[1].sim().at, peers[2].sim().at);
    }

    #[test]
    fn a_single_player_match_runs_with_no_peers() {
        let mut ends = Local::network(1).into_iter();
        let mut solo = Lockstep::new(
            Walk::new(1),
            ends.next().unwrap(),
            Config::new(1, PlayerId(0)),
        );
        // the three prefilled steps run at once on the first tick
        assert_eq!(solo.tick(2, |_| {}).unwrap(), Progress::Advanced(4));
        for _ in 1..100 {
            assert_eq!(solo.tick(2, |_| {}).unwrap(), Progress::Advanced(1));
        }
        assert_eq!(solo.executed(), 103);
        assert_eq!(solo.sim().at[0], 200);
    }

    #[test]
    fn garbage_and_forged_messages_change_nothing_and_do_not_panic() {
        let mut ends = Local::network(3).into_iter();
        let mut host = Lockstep::new(
            Walk::new(3),
            ends.next().unwrap(),
            Config::new(3, PlayerId(0)),
        );
        let (mut e1, _e2) = (ends.next().unwrap(), ends.next().unwrap());
        let junk: [&[u8]; 9] = [
            &[],
            &[1],
            &[1, 1],
            &[9, 1, 0, 0, 0, 0, 0],
            &[1, 1, 0, 0, 0, 0, 255],
            &[2, 1, 1, 2],
            &[3],
            &[1, 7, 0, 0, 0, 0, 1, 5, 0, 0, 0],
            &[1, 1, 255, 255, 255, 255, 1, 5, 0, 0, 0],
        ];
        for message in junk {
            e1.send(PeerId(0), message).unwrap();
        }
        // well formed, but peer 1 claims to be player 2
        let mut forged = vec![1u8, 2];
        3u32.encode(&mut forged);
        forged.push(1);
        99i32.encode(&mut forged);
        e1.send(PeerId(0), &forged).unwrap();
        let mut last = None;
        for _ in 0..5 {
            last = Some(host.tick(0, |_| {}).unwrap());
        }
        assert_eq!(
            last,
            Some(Progress::Stalled {
                waiting_for: vec![PlayerId(1), PlayerId(2)]
            })
        );
        assert_eq!(host.sim().at, [0, 0, 0]);
    }
}

//! [`PlayerId`] and [`PeerId`]: who is who in a multiplayer game.

/// Which player of a game: `PlayerId(0)` is the first. Local play has one
/// player, `PlayerId(0)`.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// let me = PlayerId(1);
/// assert_eq!(me.index(), 1); // index into the `inputs` slice of a `Sim`
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// The position of this player in a list of per-player values.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The player at list position `index`, or `None` past 255.
    pub fn from_index(index: usize) -> Option<PlayerId> {
        u8::try_from(index).ok().map(PlayerId)
    }
}

/// Who is at the other end of a `Transport`. Peer *n* is
/// the machine playing player *n*.
///
/// ```
/// use terminal_graphics_interactive::game::*;
///
/// assert_eq!(PeerId::from(PlayerId(2)), PeerId(2));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub u8);

impl From<PlayerId> for PeerId {
    fn from(player: PlayerId) -> PeerId {
        PeerId(player.0)
    }
}

impl From<PeerId> for PlayerId {
    fn from(peer: PeerId) -> PlayerId {
        PlayerId(peer.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_i_is_player_i_and_back() {
        assert_eq!(PeerId::from(PlayerId(3)), PeerId(3));
        assert_eq!(PlayerId::from(PeerId(5)), PlayerId(5));
    }

    #[test]
    fn a_player_id_is_an_index() {
        assert_eq!(PlayerId(2).index(), 2);
        assert_eq!(PlayerId::from_index(7), Some(PlayerId(7)));
        assert_eq!(PlayerId::from_index(256), None);
    }

    #[test]
    fn the_first_player_is_the_default() {
        assert_eq!(PlayerId::default(), PlayerId(0));
        assert_eq!(PeerId::default(), PeerId(0));
    }
}

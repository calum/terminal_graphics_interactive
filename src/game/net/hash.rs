//! A small, fixed hash for checking that two machines agree on a game state.
//!
//! It is 64-bit FNV-1a over the little-endian bytes of each value, so the same
//! values give the same hash on every machine. It is not a cryptographic hash.

const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// A 64-bit FNV-1a hash of values fed in order.
///
/// Each method feeds one value and returns the hasher, so calls chain. Two
/// hashers that were fed the same values in the same order give the same
/// [`finish`](Hasher::finish).
///
/// ```
/// use terminal_graphics_interactive::game::Hasher;
///
/// let a = Hasher::new().u32(7).str("tick").finish();
/// let b = Hasher::new().u32(7).str("tick").finish();
/// assert_eq!(a, b);
/// ```
#[derive(Clone, Debug)]
pub struct Hasher {
    state: u64,
}

impl Default for Hasher {
    /// The same as [`Hasher::new`].
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::default().finish(), Hasher::new().finish());
    /// ```
    fn default() -> Hasher {
        Hasher::new()
    }
}

impl Hasher {
    /// A hasher with nothing fed yet. Its [`finish`](Hasher::finish) is the
    /// FNV offset basis.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().finish(), 0xcbf2_9ce4_8422_2325);
    /// ```
    pub fn new() -> Hasher {
        Hasher {
            state: OFFSET_BASIS,
        }
    }

    /// Feeds raw bytes, one at a time.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().bytes(b"a").finish(), 0xaf63_dc4c_8601_ec8c);
    /// ```
    pub fn bytes(&mut self, data: &[u8]) -> &mut Hasher {
        for &byte in data {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(PRIME);
        }
        self
    }

    /// Feeds a `u8`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().u8(7).finish(), Hasher::new().bytes(&[7]).finish());
    /// ```
    pub fn u8(&mut self, value: u8) -> &mut Hasher {
        self.bytes(&[value])
    }

    /// Feeds a `u16` as its two little-endian bytes.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().u16(0x0102).finish(), Hasher::new().bytes(&[2, 1]).finish());
    /// ```
    pub fn u16(&mut self, value: u16) -> &mut Hasher {
        self.bytes(&value.to_le_bytes())
    }

    /// Feeds a `u32` as its four little-endian bytes.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().u32(1).finish(), Hasher::new().bytes(&[1, 0, 0, 0]).finish());
    /// ```
    pub fn u32(&mut self, value: u32) -> &mut Hasher {
        self.bytes(&value.to_le_bytes())
    }

    /// Feeds a `u64` as its eight little-endian bytes.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// let a = Hasher::new().u64(1).finish();
    /// assert_eq!(a, Hasher::new().bytes(&[1, 0, 0, 0, 0, 0, 0, 0]).finish());
    /// ```
    pub fn u64(&mut self, value: u64) -> &mut Hasher {
        self.bytes(&value.to_le_bytes())
    }

    /// Feeds an `i32` as its four little-endian bytes.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().i32(-1).finish(), Hasher::new().bytes(&[0xFF; 4]).finish());
    /// ```
    pub fn i32(&mut self, value: i32) -> &mut Hasher {
        self.bytes(&value.to_le_bytes())
    }

    /// Feeds a `bool` as one byte: `1` for true, `0` for false.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// assert_eq!(Hasher::new().bool(true).finish(), Hasher::new().u8(1).finish());
    /// ```
    pub fn bool(&mut self, value: bool) -> &mut Hasher {
        self.u8(u8::from(value))
    }

    /// Feeds an `f32` by its bits. Every NaN counts as one value (it is hashed
    /// as `0x7fc00000`). `0.0` and `-0.0` are different values, because their
    /// bits differ.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// let nan = f32::from_bits(0x7fc0_0001);
    /// assert_eq!(Hasher::new().f32(nan).finish(), Hasher::new().f32(f32::NAN).finish());
    /// assert_ne!(Hasher::new().f32(0.0).finish(), Hasher::new().f32(-0.0).finish());
    /// ```
    pub fn f32(&mut self, value: f32) -> &mut Hasher {
        let bits = if value.is_nan() {
            0x7fc0_0000
        } else {
            value.to_bits()
        };
        self.u32(bits)
    }

    /// Feeds a `str` as a `u32` byte length, then its UTF-8 bytes. The length
    /// keeps `"ab"` then `"c"` apart from `"a"` then `"bc"`. Lengths above
    /// `u32::MAX` bytes are not supported.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// let split_early = Hasher::new().str("ab").str("c").finish();
    /// let split_late = Hasher::new().str("a").str("bc").finish();
    /// assert_ne!(split_early, split_late);
    /// ```
    pub fn str(&mut self, text: &str) -> &mut Hasher {
        self.u32(text.len() as u32).bytes(text.as_bytes())
    }

    /// The hash of everything fed so far. Does not change the hasher, so it
    /// can be called again after feeding more values.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Hasher;
    ///
    /// let mut h = Hasher::new();
    /// h.u8(1);
    /// assert_eq!(h.finish(), h.finish());
    /// ```
    pub fn finish(&self) -> u64 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_new() {
        assert_eq!(Hasher::default().finish(), Hasher::new().finish());
    }

    #[test]
    fn empty_input_is_the_offset_basis() {
        assert_eq!(Hasher::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn u32_feeds_its_four_little_endian_bytes() {
        let value = 0x0403_0201u32;
        assert_eq!(
            Hasher::new().u32(value).finish(),
            Hasher::new().bytes(&[1, 2, 3, 4]).finish()
        );
    }

    #[test]
    fn every_nan_hashes_the_same_and_zero_and_negative_zero_differ() {
        let nan_a = f32::from_bits(0x7fc0_0000);
        let nan_b = f32::from_bits(0x7fc0_0001);
        assert_eq!(
            Hasher::new().f32(nan_a).finish(),
            Hasher::new().f32(nan_b).finish()
        );
        assert_ne!(
            Hasher::new().f32(0.0).finish(),
            Hasher::new().f32(-0.0).finish()
        );
    }

    #[test]
    fn nan_is_hashed_as_the_canonical_bits_7fc00000() {
        assert_eq!(
            Hasher::new().f32(f32::from_bits(0x7fc0_0001)).finish(),
            Hasher::new().u32(0x7fc0_0000).finish()
        );
    }

    #[test]
    fn strings_are_length_prefixed_so_splits_do_not_collide() {
        let split_early = Hasher::new().str("ab").str("c").finish();
        let split_late = Hasher::new().str("a").str("bc").finish();
        assert_ne!(split_early, split_late);
        assert_eq!(
            Hasher::new().str("ab").finish(),
            Hasher::new().str("ab").finish()
        );
    }

    #[test]
    fn small_integers_and_bools_feed_their_little_endian_bytes() {
        assert_eq!(
            Hasher::new().u8(7).finish(),
            Hasher::new().bytes(&[7]).finish()
        );
        assert_eq!(
            Hasher::new().u16(0x0102).finish(),
            Hasher::new().bytes(&[2, 1]).finish()
        );
        assert_eq!(
            Hasher::new().i32(-1).finish(),
            Hasher::new().bytes(&[0xFF; 4]).finish()
        );
        assert_eq!(
            Hasher::new().u64(1).finish(),
            Hasher::new().bytes(&[1, 0, 0, 0, 0, 0, 0, 0]).finish()
        );
        assert_eq!(
            Hasher::new().bool(true).finish(),
            Hasher::new().bytes(&[1]).finish()
        );
        assert_eq!(
            Hasher::new().bool(false).finish(),
            Hasher::new().bytes(&[0]).finish()
        );
    }

    #[test]
    fn matches_the_published_fnv1a_vectors() {
        assert_eq!(Hasher::new().bytes(b"a").finish(), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(
            Hasher::new().bytes(b"foobar").finish(),
            0x8594_4171_f739_67e8
        );
    }

    #[test]
    fn order_matters() {
        assert_ne!(
            Hasher::new().u8(1).u8(2).finish(),
            Hasher::new().u8(2).u8(1).finish()
        );
    }

    #[test]
    fn finish_does_not_consume_or_reset() {
        let mut h = Hasher::new();
        h.u32(9);
        let first = h.finish();
        assert_eq!(h.finish(), first);
        h.u8(0);
        assert_ne!(h.finish(), first);
    }
}

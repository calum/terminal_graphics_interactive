/// A value that can be written as bytes. Integers are little endian, `f32` is
/// its exact bits, and a `Vec` or `String` has a `u32` length prefix.
///
/// ```
/// use terminal_graphics_interactive::game::Encode;
///
/// assert_eq!(1u32.to_bytes(), vec![1, 0, 0, 0]);
/// assert_eq!("hi".to_string().to_bytes(), vec![2, 0, 0, 0, b'h', b'i']);
/// ```
pub trait Encode {
    /// Appends the encoding of `self` to `out`, so one buffer can hold many values.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Encode;
    ///
    /// let mut out = Vec::new();
    /// 1u8.encode(&mut out);
    /// true.encode(&mut out);
    /// assert_eq!(out, vec![1, 1]);
    /// ```
    fn encode(&self, out: &mut Vec<u8>);

    /// The encoding of `self` as a new `Vec`.
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }
}

/// Why a byte slice could not be decoded. Match with a `_` arm: more kinds may
/// be added.
///
/// ```
/// use terminal_graphics_interactive::game::{Decode, DecodeError};
///
/// match bool::from_bytes(&[5]) {
///     Err(DecodeError::Invalid(what)) => assert_eq!(what, "bool"),
///     other => panic!("unexpected {other:?}"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// The input ended before the value was complete.
    Truncated,
    /// The bytes are present but do not form a valid value. The text says which kind.
    Invalid(&'static str),
    /// A complete value was decoded but bytes were left over.
    Trailing,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Truncated => write!(f, "the bytes ended early; more were expected"),
            DecodeError::Invalid(what) => write!(f, "the bytes are not a valid {what}"),
            DecodeError::Trailing => write!(f, "bytes were left over after the value"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A cursor over bytes being decoded. Each read consumes bytes from the front.
///
/// ```
/// use terminal_graphics_interactive::game::Reader;
///
/// let mut r = Reader::new(&[7, 1, 0, 0, 0]);
/// assert_eq!(r.u8(), Ok(7));
/// assert_eq!(r.u32(), Ok(1));
/// assert!(r.is_empty());
/// ```
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// A reader over all of `bytes`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[1, 2]).remaining(), 2);
    /// ```
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes }
    }

    /// The next `n` bytes, which are consumed. `Truncated` if fewer than `n` remain.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::{DecodeError, Reader};
    ///
    /// let mut r = Reader::new(&[1, 2, 3]);
    /// assert_eq!(r.bytes(2), Ok(&[1, 2][..]));
    /// assert_eq!(r.bytes(5), Err(DecodeError::Truncated));
    /// ```
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if n > self.bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let (head, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        Ok(head)
    }

    /// Bytes not yet read.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// let mut r = Reader::new(&[1, 2]);
    /// r.u8().unwrap();
    /// assert_eq!(r.remaining(), 1);
    /// ```
    pub fn remaining(&self) -> usize {
        self.bytes.len()
    }

    /// True when every byte has been read.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert!(Reader::new(&[]).is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The next `N` bytes as an array, consumed.
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    /// A `u8`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[9]).u8(), Ok(9));
    /// ```
    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }

    /// A little-endian `u16`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[1, 2]).u16(), Ok(0x0201));
    /// ```
    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    /// A little-endian `u32`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[1, 0, 0, 0]).u32(), Ok(1));
    /// ```
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    /// A little-endian `u64`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[0xFF; 8]).u64(), Ok(u64::MAX));
    /// ```
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    /// A little-endian `i32`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Reader;
    ///
    /// assert_eq!(Reader::new(&[0xFF; 4]).i32(), Ok(-1));
    /// ```
    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take()?))
    }

    /// A `bool`: one byte, `0` or `1`. Any other byte is `Invalid("bool")`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::{DecodeError, Reader};
    ///
    /// assert_eq!(Reader::new(&[1]).bool(), Ok(true));
    /// assert_eq!(Reader::new(&[2]).bool(), Err(DecodeError::Invalid("bool")));
    /// ```
    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Invalid("bool")),
        }
    }

    /// An `f32` from its exact bits, so every value (NaN payloads, `-0.0`) survives.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::{Encode, Reader};
    ///
    /// let bytes = (-0.0f32).to_bytes();
    /// assert_eq!(Reader::new(&bytes).f32().unwrap().to_bits(), (-0.0f32).to_bits());
    /// ```
    pub fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(u32::from_le_bytes(self.take()?)))
    }
}

impl Encode for f32 {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_bits().to_le_bytes());
    }
}

impl Decode for f32 {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.f32()
    }
}

macro_rules! le_int {
    ($($t:ident => $read:ident),* $(,)?) => {$(
        impl Encode for $t {
            fn encode(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
        }

        impl Decode for $t {
            fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
                r.$read()
            }
        }
    )*};
}

le_int!(u8 => u8, u16 => u16, u32 => u32, u64 => u64, i32 => i32);

/// A value that can be read back from bytes written by [`Encode`]. A decode
/// never panics: bad input is an error, and decoding is strict (no bytes may
/// be left over, and `bool`, `Option` tags and UTF-8 are checked).
///
/// ```
/// use terminal_graphics_interactive::game::{Decode, DecodeError, Encode};
///
/// let bytes = (3u8, Some(-1i32)).to_bytes();
/// assert_eq!(<(u8, Option<i32>)>::from_bytes(&bytes), Ok((3, Some(-1))));
/// assert_eq!(u8::from_bytes(&[1, 2]), Err(DecodeError::Trailing));
/// ```
pub trait Decode: Sized {
    /// Reads one value from `r`, consuming exactly its bytes.
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;

    /// Decodes a value that must fill `bytes` exactly. Errors with `Trailing`
    /// if bytes are left over, and `Truncated` if the value does not fit.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::{Decode, DecodeError};
    ///
    /// assert_eq!(u16::from_bytes(&[1, 0]), Ok(1));
    /// assert_eq!(u16::from_bytes(&[1]), Err(DecodeError::Truncated));
    /// ```
    fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let value = Self::decode(&mut r)?;
        if r.is_empty() {
            Ok(value)
        } else {
            Err(DecodeError::Trailing)
        }
    }
}

impl<A: Encode, B: Encode> Encode for (A, B) {
    fn encode(&self, out: &mut Vec<u8>) {
        self.0.encode(out);
        self.1.encode(out);
    }
}

impl<A: Decode, B: Decode> Decode for (A, B) {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok((A::decode(r)?, B::decode(r)?))
    }
}

impl<A: Encode, B: Encode, C: Encode> Encode for (A, B, C) {
    fn encode(&self, out: &mut Vec<u8>) {
        self.0.encode(out);
        self.1.encode(out);
        self.2.encode(out);
    }
}

impl<A: Decode, B: Decode, C: Decode> Decode for (A, B, C) {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok((A::decode(r)?, B::decode(r)?, C::decode(r)?))
    }
}

impl<T: Encode, const N: usize> Encode for [T; N] {
    /// The items one after another, with no length: the length is in the type.
    fn encode(&self, out: &mut Vec<u8>) {
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Decode, const N: usize> Decode for [T; N] {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(T::decode(r)?);
        }
        // Exactly N items were pushed, so this conversion cannot fail.
        items.try_into().map_err(|_| DecodeError::Truncated)
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Vec<u8>) {
        (self.len() as u32).encode(out);
        out.extend_from_slice(self.as_bytes());
    }
}

impl Decode for String {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let len = u32::decode(r)? as usize;
        let bytes = r.bytes(len)?;
        match std::str::from_utf8(bytes) {
            Ok(text) => Ok(text.to_owned()),
            Err(_) => Err(DecodeError::Invalid("utf-8")),
        }
    }
}

impl<T: Encode> Encode for Vec<T> {
    /// A `u32` length, then each element. Lengths above `u32::MAX` are not representable.
    fn encode(&self, out: &mut Vec<u8>) {
        (self.len() as u32).encode(out);
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let len = u32::decode(r)? as usize;
        // Every element takes at least one byte, so a length longer than
        // the rest of the input cannot be satisfied: refuse before allocating.
        if len > r.remaining() {
            return Err(DecodeError::Truncated);
        }
        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(T::decode(r)?);
        }
        Ok(items)
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, out: &mut Vec<u8>) {
        match self {
            None => out.push(0),
            Some(value) => {
                out.push(1);
                value.encode(out);
            }
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match r.u8()? {
            0 => Ok(None),
            1 => Ok(Some(T::decode(r)?)),
            _ => Err(DecodeError::Invalid("option")),
        }
    }
}

impl Encode for bool {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }
}

impl Decode for bool {
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.bool()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_is_little_endian() {
        assert_eq!(1u32.to_bytes(), vec![1, 0, 0, 0]);
    }

    #[test]
    fn u32_round_trips() {
        for v in [0u32, 1, 0x1234_5678, u32::MAX] {
            assert_eq!(u32::from_bytes(&v.to_bytes()), Ok(v));
        }
    }

    #[test]
    fn i32_and_u64_are_little_endian_and_round_trip() {
        assert_eq!(i32::MIN.to_bytes(), vec![0, 0, 0, 0x80]);
        assert_eq!(u64::MAX.to_bytes(), vec![0xFF; 8]);
        assert_eq!(i32::from_bytes(&i32::MIN.to_bytes()), Ok(i32::MIN));
        assert_eq!(
            u64::from_bytes(&0x0102_0304_0506_0708u64.to_bytes()),
            Ok(0x0102_0304_0506_0708)
        );
    }

    #[test]
    fn f32_round_trips_exact_bits() {
        for bits in [0x7FC0_0001u32, 0x8000_0000, 0x0000_0000, 0x3F80_0000] {
            let v = f32::from_bits(bits);
            let back = f32::from_bytes(&v.to_bytes()).unwrap();
            assert_eq!(back.to_bits(), bits);
        }
    }

    #[test]
    fn vec_with_a_huge_length_prefix_is_truncated_without_allocating() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(Vec::<u8>::from_bytes(&bytes), Err(DecodeError::Truncated));
        assert_eq!(
            Vec::<u32>::from_bytes(&[1, 0, 0, 0]),
            Err(DecodeError::Truncated)
        );
        assert_eq!(Vec::<u8>::from_bytes(&[2, 0, 0, 0, 9, 8]), Ok(vec![9, 8]));
    }

    #[test]
    fn random_bytes_never_panic_and_successful_decodes_re_encode_exactly() {
        use crate::game::Rng;
        let mut rng = Rng::new(0xC0DE_C0DE);
        let mut vec_ok = 0;
        let mut string_ok = 0;
        for _ in 0..20_000 {
            let len = rng.range_i32(0, 40) as usize;
            let mut bytes: Vec<u8> = (0..len).map(|_| (rng.next_u32() >> 24) as u8).collect();
            // Random bytes almost never form a small length prefix, so half the
            // time write a count of 0 to 3 into the first four bytes. Otherwise
            // the successful-decode branches would never run.
            if len >= 4 && rng.chance(0.5) {
                let count = rng.range_i32(0, 4) as u32;
                bytes[..4].copy_from_slice(&count.to_le_bytes());
            }

            if let Ok(value) = <Vec<(u8, Option<i32>, bool)>>::from_bytes(&bytes) {
                assert_eq!(value.to_bytes(), bytes);
                vec_ok += 1;
            }
            if let Ok(text) = String::from_bytes(&bytes) {
                assert_eq!(text.to_bytes(), bytes);
                string_ok += 1;
            }
        }
        assert!(
            vec_ok > 20,
            "only {vec_ok} Vec decodes succeeded: the check is vacuous"
        );
        assert!(
            string_ok > 20,
            "only {string_ok} String decodes succeeded: the check is vacuous"
        );
    }

    #[test]
    fn every_strict_prefix_is_truncated_and_a_trailing_byte_is_trailing() {
        let value: Vec<(u8, Option<i32>, bool)> = vec![(1, Some(-5), true), (2, None, false)];
        let bytes = value.to_bytes();
        for cut in 0..bytes.len() {
            assert_eq!(
                <Vec<(u8, Option<i32>, bool)>>::from_bytes(&bytes[..cut]),
                Err(DecodeError::Truncated),
                "prefix of length {cut}"
            );
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert_eq!(
            <Vec<(u8, Option<i32>, bool)>>::from_bytes(&extra),
            Err(DecodeError::Trailing)
        );
    }

    #[test]
    fn reader_reads_bool_and_rejects_other_bytes() {
        assert_eq!(Reader::new(&[1]).bool(), Ok(true));
        assert_eq!(Reader::new(&[3]).bool(), Err(DecodeError::Invalid("bool")));
    }

    #[test]
    fn decode_errors_say_what_went_wrong() {
        assert!(DecodeError::Truncated.to_string().contains("ended early"));
        assert!(DecodeError::Invalid("bool").to_string().contains("bool"));
        assert!(DecodeError::Trailing.to_string().contains("left over"));
        let _: &dyn std::error::Error = &DecodeError::Trailing;
    }

    #[test]
    fn tuples_and_arrays_have_no_length_prefix() {
        assert_eq!((1u8, 2u16).to_bytes(), vec![1, 2, 0]);
        assert_eq!([7u8, 8, 9].to_bytes(), vec![7, 8, 9]);
        assert_eq!(
            <(u8, bool, i32)>::from_bytes(&(3u8, true, -1i32).to_bytes()),
            Ok((3, true, -1))
        );
        assert_eq!(<[u16; 2]>::from_bytes(&[1, 0, 2, 0]), Ok([1, 2]));
    }

    #[test]
    fn string_round_trips_and_rejects_bad_utf8() {
        let s = String::from("héllo, 世界");
        assert_eq!(String::from_bytes(&s.to_bytes()), Ok(s));
        assert_eq!(
            String::from_bytes(&[1, 0, 0, 0, 0xFF]),
            Err(DecodeError::Invalid("utf-8"))
        );
    }

    #[test]
    fn option_tag_2_is_invalid() {
        assert_eq!(
            Option::<u8>::from_bytes(&[2, 0]),
            Err(DecodeError::Invalid("option"))
        );
        assert_eq!(Option::<u8>::from_bytes(&[1, 7]), Ok(Some(7)));
        assert_eq!(None::<u8>.to_bytes(), vec![0]);
    }

    #[test]
    fn bool_rejects_any_byte_but_0_and_1() {
        assert_eq!(bool::from_bytes(&[2]), Err(DecodeError::Invalid("bool")));
    }
}

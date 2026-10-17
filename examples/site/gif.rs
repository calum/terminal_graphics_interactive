//! A small GIF89a encoder, so the README animations can be made by a Rust
//! program in this repository with no tools or crates.
//!
//! Every frame uses one shared palette of up to 256 colours. Pixels are
//! palette indexes, left to right then top to bottom.

/// One picture of the animation.
pub struct Frame {
    pub pixels: Vec<u8>,
    /// How long to show it, in hundredths of a second.
    pub delay: u16,
}

/// Encodes `frames` (each `width * height` palette indexes) as a looping GIF.
/// Panics if the palette is empty or has more than 256 colours, or a frame is
/// the wrong size: those are mistakes in the caller, not in any input.
pub fn encode(width: u16, height: u16, palette: &[[u8; 3]], frames: &[Frame]) -> Vec<u8> {
    assert!(
        !palette.is_empty() && palette.len() <= 256,
        "the palette must have 1 to 256 colours, not {}",
        palette.len()
    );
    let pixel_count = width as usize * height as usize;
    for frame in frames {
        assert_eq!(
            frame.pixels.len(),
            pixel_count,
            "a frame needs width * height pixels"
        );
    }

    // the colour table holds a power of two entries, at least 2
    let table_bits = (usize::BITS - (palette.len().max(2) - 1).leading_zeros()).max(1);
    let table_size = 1usize << table_bits;

    let mut out = Vec::new();
    out.extend_from_slice(b"GIF89a");
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    // a global colour table, 8 bits per colour, of 2^table_bits entries
    out.push(0x80 | 0x70 | (table_bits as u8 - 1));
    out.push(0); // background colour
    out.push(0); // square pixels
    for i in 0..table_size {
        out.extend_from_slice(&palette.get(i).copied().unwrap_or([0, 0, 0]));
    }

    // loop forever
    out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
    out.extend_from_slice(b"NETSCAPE2.0");
    out.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);

    let min_code_size = table_bits.max(2);
    let mut dictionary = Dictionary::new();
    for frame in frames {
        // graphic control: draw over the last frame, no transparency
        out.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04]);
        out.extend_from_slice(&frame.delay.to_le_bytes());
        out.extend_from_slice(&[0x00, 0x00]);

        // image descriptor: the whole screen, no local colour table
        out.push(0x2C);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.push(0x00);

        out.push(min_code_size as u8);
        let data = lzw(&frame.pixels, min_code_size, &mut dictionary);
        for chunk in data.chunks(255) {
            out.push(chunk.len() as u8);
            out.extend_from_slice(chunk);
        }
        out.push(0);
    }
    out.push(0x3B);
    out
}

/// The LZW dictionary: which code stands for "this code followed by this
/// byte". Entries are stamped with a generation, so emptying it is one
/// increment and not a 4 MB clear.
struct Dictionary {
    slots: Vec<u32>,
    generation: u32,
}

impl Dictionary {
    fn new() -> Dictionary {
        Dictionary {
            slots: vec![0; 4096 * 256],
            generation: 1,
        }
    }

    fn get(&self, prefix: u16, byte: u8) -> Option<u16> {
        let value = self.slots[(prefix as usize) << 8 | byte as usize];
        (value >> 12 == self.generation).then_some((value & 0xFFF) as u16)
    }

    fn insert(&mut self, prefix: u16, byte: u8, code: u16) {
        self.slots[(prefix as usize) << 8 | byte as usize] = self.generation << 12 | code as u32;
    }

    fn clear(&mut self) {
        self.generation += 1;
    }
}

/// Packs variable width codes into bytes, least significant bit first.
struct BitWriter {
    bytes: Vec<u8>,
    accumulator: u32,
    bits: u32,
}

impl BitWriter {
    fn put(&mut self, code: u16, size: u32) {
        self.accumulator |= (code as u32) << self.bits;
        self.bits += size;
        while self.bits >= 8 {
            self.bytes.push(self.accumulator as u8);
            self.accumulator >>= 8;
            self.bits -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.bytes.push(self.accumulator as u8);
        }
        self.bytes
    }
}

/// GIF's flavour of LZW. The code size grows as the dictionary fills, and the
/// dictionary is emptied (with a clear code) when it reaches 4096 entries.
fn lzw(pixels: &[u8], min_code_size: u32, dictionary: &mut Dictionary) -> Vec<u8> {
    let clear = 1u16 << min_code_size;
    let end = clear + 1;
    let mut writer = BitWriter {
        bytes: Vec::new(),
        accumulator: 0,
        bits: 0,
    };
    let mut size = min_code_size + 1;
    let mut next = end + 1;
    dictionary.clear();
    writer.put(clear, size);

    let mut pixels = pixels.iter();
    if let Some(&first) = pixels.next() {
        let mut prefix = first as u16;
        for &byte in pixels {
            if let Some(code) = dictionary.get(prefix, byte) {
                prefix = code;
                continue;
            }
            writer.put(prefix, size);
            if next < 4096 {
                dictionary.insert(prefix, byte, next);
                // a code that needs more bits may be used from the next one on
                if next == 1 << size && size < 12 {
                    size += 1;
                }
                next += 1;
            } else {
                writer.put(clear, size);
                dictionary.clear();
                size = min_code_size + 1;
                next = end + 1;
            }
            prefix = byte as u16;
        }
        writer.put(prefix, size);
        // A decoder adds a dictionary entry for every code it reads, including
        // this last one, so it may move to wider codes before it reads the end
        // code. Do the same here.
        if next == 1 << size && size < 12 {
            size += 1;
        }
    }
    writer.put(end, size);
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a GIF decodes to.
    struct Decoded {
        width: u16,
        height: u16,
        palette: Vec<[u8; 3]>,
        looping: bool,
        frames: Vec<(u16, Vec<u8>)>,
    }

    /// A minimal, independent GIF reader, so the encoder is checked against
    /// the file format and not against itself.
    fn decode(bytes: &[u8]) -> Decoded {
        assert_eq!(&bytes[..6], b"GIF89a", "header");
        let width = u16::from_le_bytes([bytes[6], bytes[7]]);
        let height = u16::from_le_bytes([bytes[8], bytes[9]]);
        let flags = bytes[10];
        assert!(flags & 0x80 != 0, "a global colour table");
        let size = 2usize << (flags & 7);
        let mut at = 13;
        let palette: Vec<[u8; 3]> = (0..size)
            .map(|i| {
                [
                    bytes[at + i * 3],
                    bytes[at + i * 3 + 1],
                    bytes[at + i * 3 + 2],
                ]
            })
            .collect();
        at += size * 3;

        let (mut looping, mut delay) = (false, 0u16);
        let mut frames = Vec::new();
        loop {
            match bytes[at] {
                0x21 => {
                    let label = bytes[at + 1];
                    at += 2;
                    if label == 0xF9 {
                        assert_eq!(bytes[at], 4, "graphic control block size");
                        delay = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]);
                    }
                    if label == 0xFF && &bytes[at + 1..at + 12] == b"NETSCAPE2.0" {
                        looping = true;
                    }
                    while bytes[at] != 0 {
                        at += 1 + bytes[at] as usize;
                    }
                    at += 1;
                }
                0x2C => {
                    let w = u16::from_le_bytes([bytes[at + 5], bytes[at + 6]]);
                    let h = u16::from_le_bytes([bytes[at + 7], bytes[at + 8]]);
                    assert_eq!((w, h), (width, height), "full-size frames");
                    assert_eq!(bytes[at + 9], 0, "no local colour table, not interlaced");
                    let min_code_size = bytes[at + 10] as u32;
                    at += 11;
                    let mut data = Vec::new();
                    while bytes[at] != 0 {
                        let length = bytes[at] as usize;
                        assert!(length <= 255);
                        data.extend_from_slice(&bytes[at + 1..at + 1 + length]);
                        at += 1 + length;
                    }
                    at += 1;
                    frames.push((
                        delay,
                        lzw_decode(&data, min_code_size, w as usize * h as usize),
                    ));
                }
                0x3B => break,
                other => panic!("unexpected block {other:#x} at {at}"),
            }
        }
        Decoded {
            width,
            height,
            palette,
            looping,
            frames,
        }
    }

    fn lzw_decode(data: &[u8], min_code_size: u32, expected: usize) -> Vec<u8> {
        let clear = 1u32 << min_code_size;
        let end = clear + 1;
        let mut table: Vec<Vec<u8>> = Vec::new();
        let reset = |table: &mut Vec<Vec<u8>>| {
            table.clear();
            for i in 0..clear {
                table.push(vec![i as u8]);
            }
            table.push(Vec::new()); // clear
            table.push(Vec::new()); // end
        };
        reset(&mut table);

        let mut out = Vec::new();
        let mut size = min_code_size + 1;
        let (mut bits, mut have) = (0u32, 0u32);
        let mut bytes = data.iter();
        let mut previous: Option<Vec<u8>> = None;
        loop {
            while have < size {
                match bytes.next() {
                    Some(&byte) => {
                        bits |= (byte as u32) << have;
                        have += 8;
                    }
                    None => panic!("ran out of data before the end code"),
                }
            }
            let code = bits & ((1 << size) - 1);
            bits >>= size;
            have -= size;

            if code == clear {
                reset(&mut table);
                size = min_code_size + 1;
                previous = None;
                continue;
            }
            if code == end {
                break;
            }
            let entry = if (code as usize) < table.len() {
                table[code as usize].clone()
            } else {
                // the one case where a code is used before it is defined
                let mut entry = previous.clone().expect("a previous entry");
                entry.push(entry[0]);
                entry
            };
            out.extend_from_slice(&entry);
            if let Some(previous) = previous {
                if table.len() < 4096 {
                    let mut added = previous;
                    added.push(entry[0]);
                    table.push(added);
                }
            }
            if table.len() == (1 << size) && size < 12 {
                size += 1;
            }
            previous = Some(entry);
        }
        assert_eq!(out.len(), expected, "pixel count");
        out
    }

    fn grey(count: usize) -> Vec<[u8; 3]> {
        (0..count).map(|i| [i as u8, i as u8, i as u8]).collect()
    }

    fn frame(pixels: Vec<u8>, delay: u16) -> Frame {
        Frame { pixels, delay }
    }

    #[test]
    fn a_tiny_two_colour_picture_round_trips() {
        let pixels = vec![0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0];
        let bytes = encode(4, 4, &grey(2), &[frame(pixels.clone(), 7)]);
        let decoded = decode(&bytes);
        assert_eq!((decoded.width, decoded.height), (4, 4));
        assert_eq!(decoded.frames, vec![(7, pixels)]);
        assert_eq!(decoded.palette, grey(2));
        assert_eq!(bytes.last(), Some(&0x3B), "trailer");
    }

    #[test]
    fn the_palette_is_padded_to_a_power_of_two_and_keeps_its_colours() {
        for count in [1usize, 2, 3, 4, 5, 16, 17, 100, 255, 256] {
            let palette = grey(count);
            let pixels: Vec<u8> = (0..64).map(|i| (i % count) as u8).collect();
            let bytes = encode(8, 8, &palette, &[frame(pixels.clone(), 1)]);
            let decoded = decode(&bytes);
            assert!(decoded.palette.len().is_power_of_two(), "{count} colours");
            assert!(decoded.palette.len() >= count.max(2));
            assert_eq!(&decoded.palette[..count], &palette[..], "{count} colours");
            assert_eq!(decoded.frames[0].1, pixels, "{count} colours");
        }
    }

    #[test]
    fn a_picture_needing_many_codes_resets_the_dictionary_and_still_round_trips() {
        // pseudo random pixels fill the 4096 entry dictionary many times over
        let mut seed = 12345u32;
        let pixels: Vec<u8> = (0..200 * 200)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8 % 7
            })
            .collect();
        let bytes = encode(200, 200, &grey(7), &[frame(pixels.clone(), 3)]);
        assert_eq!(decode(&bytes).frames[0].1, pixels);
    }

    #[test]
    fn long_runs_of_one_colour_round_trip_and_compress() {
        let pixels = vec![5u8; 300 * 300];
        let bytes = encode(300, 300, &grey(8), &[frame(pixels.clone(), 1)]);
        assert_eq!(decode(&bytes).frames[0].1, pixels);
        assert!(bytes.len() < 3000, "{} bytes for a flat image", bytes.len());
    }

    #[test]
    fn several_frames_keep_their_order_and_their_delays() {
        let frames: Vec<Frame> = (0..5)
            .map(|i| frame(vec![(i % 3) as u8; 12], 10 + i as u16))
            .collect();
        let decoded = decode(&encode(4, 3, &grey(3), &frames));
        assert_eq!(decoded.frames.len(), 5);
        for (i, (delay, pixels)) in decoded.frames.iter().enumerate() {
            assert_eq!(*delay, 10 + i as u16);
            assert_eq!(pixels, &vec![(i % 3) as u8; 12]);
        }
    }

    #[test]
    fn the_animation_loops_forever() {
        let decoded = decode(&encode(2, 2, &grey(2), &[frame(vec![0, 1, 1, 0], 5)]));
        assert!(decoded.looping);
    }

    #[test]
    fn data_longer_than_one_sub_block_is_split_correctly() {
        // varied pixels so the compressed data is well over 255 bytes
        let pixels: Vec<u8> = (0..4000u32).map(|i| ((i * i + i / 3) % 13) as u8).collect();
        let bytes = encode(100, 40, &grey(13), &[frame(pixels.clone(), 1)]);
        assert_eq!(decode(&bytes).frames[0].1, pixels);
    }

    #[test]
    fn one_pixel_wide_and_tall_pictures_work() {
        for (w, h) in [(1u16, 1u16), (1, 50), (50, 1), (3, 1)] {
            let pixels: Vec<u8> = (0..w as usize * h as usize)
                .map(|i| (i % 2) as u8)
                .collect();
            let decoded = decode(&encode(w, h, &grey(2), &[frame(pixels.clone(), 1)]));
            assert_eq!(decoded.frames[0].1, pixels, "{w}x{h}");
        }
    }

    #[test]
    fn the_same_input_gives_the_same_bytes() {
        let pixels: Vec<u8> = (0..1000u32).map(|i| (i % 5) as u8).collect();
        let make = || encode(40, 25, &grey(5), &[frame(pixels.clone(), 2)]);
        assert_eq!(make(), make());
    }

    #[test]
    #[should_panic(expected = "palette")]
    fn an_empty_palette_is_a_mistake() {
        encode(1, 1, &[], &[frame(vec![0], 1)]);
    }

    #[test]
    #[should_panic(expected = "palette")]
    fn more_than_256_colours_is_a_mistake() {
        encode(1, 1, &grey(257), &[frame(vec![0], 1)]);
    }

    #[test]
    #[should_panic(expected = "pixels")]
    fn a_frame_of_the_wrong_size_is_a_mistake() {
        encode(2, 2, &grey(2), &[frame(vec![0, 1, 0], 1)]);
    }
}

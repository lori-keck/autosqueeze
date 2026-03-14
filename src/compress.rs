/// compress.rs — THE FILE THE AGENT EDITS
///
/// CURRENT ALGORITHM: LZ77 (sliding window)
///
/// Format (byte-level):
///   Flag byte: top bit indicates type
///   - 0xxxxxxx (0-127):  literal run of (x+1) bytes follows
///   - 1xxxxxxx xxxxxxxx: match. Length = (first byte & 0x7F) + 3, offset = next byte + 1
///     For long offsets: if first byte has bit 6 set, offset uses 2 bytes (big-endian) + 1
///
/// Simplified format for correctness:
///   0x00 <byte>                  = literal byte
///   0x01..=0xFF <offset_hi> <offset_lo> = match, length = tag + 2, offset = 16-bit BE

use std::io::{self, Read, Write};

const WINDOW_SIZE: usize = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 257; // tag 0xFF = 255 + 2 = 257

pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(input.len());
    let mut pos = 0;

    while pos < input.len() {
        let window_start = pos.saturating_sub(WINDOW_SIZE);
        let mut best_offset: u16 = 0;
        let mut best_length: usize = 0;

        // Search window for longest match
        for search_pos in window_start..pos {
            let mut length = 0usize;
            let max_len = MAX_MATCH.min(input.len() - pos);
            while length < max_len && input[search_pos + length] == input[pos + length] {
                length += 1;
            }
            if length > best_length && length >= MIN_MATCH {
                let offset = (pos - search_pos) as u16;
                if offset <= u16::MAX {
                    best_length = length;
                    best_offset = offset;
                    if length == max_len { break; }
                }
            }
        }

        if best_length >= MIN_MATCH {
            // Match: tag = length - 2 (so MIN_MATCH=3 → tag=1, MAX_MATCH=257 → tag=255)
            let tag = (best_length - 2) as u8;
            output.push(tag);
            output.push((best_offset >> 8) as u8);
            output.push((best_offset & 0xFF) as u8);
            pos += best_length;
        } else {
            // Literal: tag = 0, then the byte
            output.push(0x00);
            output.push(input[pos]);
            pos += 1;
        }
    }

    output
}

pub fn decompress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut i = 0;

    while i < input.len() {
        let tag = input[i];

        if tag == 0x00 {
            // Literal
            if i + 1 >= input.len() { break; }
            output.push(input[i + 1]);
            i += 2;
        } else {
            // Match
            if i + 2 >= input.len() { break; }
            let length = tag as usize + 2;
            let offset = ((input[i + 1] as usize) << 8) | (input[i + 2] as usize);
            i += 3;

            if offset == 0 || offset > output.len() {
                // Corrupted data, bail
                break;
            }

            let start = output.len() - offset;
            for j in 0..length {
                // Must index one at a time — match can overlap with output
                let byte = output[start + j];
                output.push(byte);
            }
        }
    }

    output
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let decompress_mode = args.iter().any(|a| a == "--decompress" || a == "-d");

    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).expect("Failed to read stdin");

    let output = if decompress_mode {
        decompress(&input)
    } else {
        compress(&input)
    };

    io::stdout().write_all(&output).expect("Failed to write stdout");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        assert_eq!(decompress(&compress(b"")), b"");
    }

    #[test]
    fn test_roundtrip_simple() {
        let input = b"aaabbbcccc";
        assert_eq!(decompress(&compress(input)), input);
    }

    #[test]
    fn test_roundtrip_no_runs() {
        let input = b"abcdefg";
        assert_eq!(decompress(&compress(input)), input);
    }

    #[test]
    fn test_roundtrip_single_byte() {
        let input = b"a";
        assert_eq!(decompress(&compress(input)), input);
    }

    #[test]
    fn test_roundtrip_binary() {
        let input: Vec<u8> = (0..=255).collect();
        assert_eq!(decompress(&compress(&input)), input);
    }

    #[test]
    fn test_roundtrip_long_run() {
        let input = vec![0x42; 1000];
        let compressed = compress(&input);
        let decompressed = decompress(&compressed);
        assert_eq!(decompressed.len(), input.len(), "Length mismatch");
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_roundtrip_random_ish() {
        let mut input = Vec::with_capacity(10000);
        let mut state: u32 = 12345;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            input.push((state >> 16) as u8);
        }
        assert_eq!(decompress(&compress(&input)), input);
    }

    #[test]
    fn test_roundtrip_repeated_pattern() {
        let pattern = b"hello world! ";
        let input: Vec<u8> = pattern.iter().cycle().take(5000).copied().collect();
        assert_eq!(decompress(&compress(&input)), input);
    }

    #[test]
    fn test_compression_ratio_improves() {
        let input = vec![0x42; 1000];
        let compressed = compress(&input);
        assert!(compressed.len() < input.len(), "Repeated data should compress");
    }
}

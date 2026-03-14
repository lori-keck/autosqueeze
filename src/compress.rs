/// compress.rs — THE FILE THE AGENT EDITS
///
/// This file contains the compression and decompression algorithms.
/// The agent modifies this file to try to achieve better compression ratios
/// and/or faster speeds. Everything is fair game: the algorithm, data structures,
/// bit manipulation, dictionary approaches, entropy coding, etc.
///
/// CONSTRAINTS:
/// - compress() must take a byte slice and return compressed bytes
/// - decompress() must take compressed bytes and return the original bytes EXACTLY
/// - Decompression must be lossless — decompress(compress(data)) == data, always
/// - No external crate dependencies (stdlib only)
///
/// CURRENT ALGORITHM: Run-Length Encoding (RLE) — the simplest possible baseline.
/// This is intentionally naive. Beat it.

use std::io::{self, Read, Write};

/// Compress the input bytes and return compressed output
pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    // Simple RLE: [count, byte] pairs
    // count is stored as a single byte (1-255), so max run length is 255
    let mut output = Vec::new();
    let mut i = 0;

    while i < input.len() {
        let byte = input[i];
        let mut count: u8 = 1;

        while i + (count as usize) < input.len()
            && input[i + (count as usize)] == byte
            && count < 255
        {
            count += 1;
        }

        output.push(count);
        output.push(byte);
        i += count as usize;
    }

    output
}

/// Decompress the compressed bytes back to the original
pub fn decompress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut i = 0;

    while i + 1 < input.len() {
        let count = input[i] as usize;
        let byte = input[i + 1];

        for _ in 0..count {
            output.push(byte);
        }

        i += 2;
    }

    output
}

/// Main entry point — reads stdin, compresses or decompresses, writes stdout
/// Usage: echo "data" | compress [--decompress]
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
        assert_eq!(decompress(&compress(&input)), input);
    }

    #[test]
    fn test_roundtrip_random_ish() {
        // Pseudo-random but deterministic
        let mut input = Vec::with_capacity(10000);
        let mut state: u32 = 12345;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            input.push((state >> 16) as u8);
        }
        assert_eq!(decompress(&compress(&input)), input);
    }
}

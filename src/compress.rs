/// compress.rs — THE FILE THE AGENT EDITS
///
/// CURRENT ALGORITHM: LZ77 with hash-chain matching + batched literals
///
/// Improvements over basic LZ77:
/// 1. Hash chains for O(1) match lookups instead of O(n) brute force
/// 2. Batched literal encoding: up to 128 literals per tag byte (was 1:1)
///
/// Format:
///   Tag byte with high bit:
///   - 0xxxxxxx: literal run of (tag+1) bytes follows (1-128 literals)
///   - 1xxxxxxx + offset_hi + offset_lo: match, length = (tag & 0x7F) + 3, offset = 16-bit BE
///
/// Window: 32KB, min match: 3, max match: 130

use std::io::{self, Read, Write};

const WINDOW_SIZE: usize = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 130; // (0x7F) + 3
const MAX_LITERAL_RUN: usize = 128; // 0x7F + 1
const HASH_CHAIN_LIMIT: usize = 64;

// ─── Token types ──────────────────────────────────────────────────────────

#[derive(Debug)]
enum Token {
    Literal(u8),
    Match { length: usize, offset: usize },
}

// ─── Hash Chain ───────────────────────────────────────────────────────────

struct HashChain {
    head: Vec<i32>,
    prev: Vec<i32>,
    mask: usize,
}

impl HashChain {
    fn new() -> Self {
        let size = 1 << 15;
        HashChain {
            head: vec![-1i32; size],
            prev: vec![-1i32; WINDOW_SIZE],
            mask: size - 1,
        }
    }

    fn hash3(data: &[u8], pos: usize) -> usize {
        if pos + 2 >= data.len() { return 0; }
        ((data[pos] as usize) << 10
         ^ (data[pos + 1] as usize) << 5
         ^ (data[pos + 2] as usize))
    }

    fn insert(&mut self, data: &[u8], pos: usize) {
        if pos + 2 >= data.len() { return; }
        let h = Self::hash3(data, pos) & self.mask;
        self.prev[pos % WINDOW_SIZE] = self.head[h];
        self.head[h] = pos as i32;
    }

    fn find_best_match(&self, data: &[u8], pos: usize) -> (usize, usize) {
        if pos + 2 >= data.len() { return (0, 0); }
        let h = Self::hash3(data, pos) & self.mask;
        let mut chain_pos = self.head[h];
        let mut best_len = 0usize;
        let mut best_offset = 0usize;
        let mut chain_count = 0;
        let min_pos = pos.saturating_sub(WINDOW_SIZE);

        while chain_pos >= 0 && (chain_pos as usize) >= min_pos && chain_count < HASH_CHAIN_LIMIT {
            let cp = chain_pos as usize;
            if cp >= pos {
                chain_pos = self.prev[cp % WINDOW_SIZE];
                chain_count += 1;
                continue;
            }

            let max_len = MAX_MATCH.min(data.len() - pos);
            let mut length = 0;
            while length < max_len && data[cp + length] == data[pos + length] {
                length += 1;
            }

            if length > best_len && length >= MIN_MATCH {
                best_len = length;
                best_offset = pos - cp;
                if length == max_len { break; }
            }

            chain_pos = self.prev[cp % WINDOW_SIZE];
            chain_count += 1;
        }

        (best_len, best_offset)
    }
}

// ─── Compress ─────────────────────────────────────────────────────────────

pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    // First pass: generate tokens
    let mut tokens: Vec<Token> = Vec::new();
    let mut chain = HashChain::new();
    let mut pos = 0;

    while pos < input.len() {
        let (match_len, match_offset) = chain.find_best_match(input, pos);

        if match_len >= MIN_MATCH && match_offset <= 65535 {
            tokens.push(Token::Match { length: match_len, offset: match_offset });
            for i in 0..match_len {
                chain.insert(input, pos + i);
            }
            pos += match_len;
        } else {
            tokens.push(Token::Literal(input[pos]));
            chain.insert(input, pos);
            pos += 1;
        }
    }

    // Second pass: encode tokens with batched literals
    let mut output = Vec::with_capacity(input.len());

    // Header: original size (4 bytes LE) for safety
    let size = input.len() as u32;
    output.extend_from_slice(&size.to_le_bytes());

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Match { length, offset } => {
                let tag = 0x80 | ((*length - MIN_MATCH) as u8);
                output.push(tag);
                output.push((*offset >> 8) as u8);
                output.push((*offset & 0xFF) as u8);
                i += 1;
            }
            Token::Literal(_) => {
                // Collect consecutive literals
                let start = i;
                let mut count = 0;
                while i < tokens.len() && count < MAX_LITERAL_RUN {
                    if let Token::Literal(_) = &tokens[i] {
                        count += 1;
                        i += 1;
                    } else {
                        break;
                    }
                }
                // Write literal run: tag = count - 1 (high bit clear)
                output.push((count - 1) as u8);
                for j in start..start + count {
                    if let Token::Literal(b) = &tokens[j] {
                        output.push(*b);
                    }
                }
            }
        }
    }

    output
}

// ─── Decompress ───────────────────────────────────────────────────────────

pub fn decompress(input: &[u8]) -> Vec<u8> {
    if input.len() < 4 {
        return Vec::new();
    }

    let orig_size = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let mut output = Vec::with_capacity(orig_size);
    let mut i = 4;

    while i < input.len() && output.len() < orig_size {
        let tag = input[i];
        i += 1;

        if tag & 0x80 != 0 {
            // Match
            if i + 1 >= input.len() { break; }
            let length = (tag & 0x7F) as usize + MIN_MATCH;
            let offset = ((input[i] as usize) << 8) | (input[i + 1] as usize);
            i += 2;

            if offset == 0 || offset > output.len() { break; }

            let start = output.len() - offset;
            for j in 0..length {
                let byte = output[start + j];
                output.push(byte);
            }
        } else {
            // Literal run
            let count = (tag as usize) + 1;
            if i + count > input.len() { break; }
            output.extend_from_slice(&input[i..i + count]);
            i += count;
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
        assert_eq!(decompress(&compress(&input)), input);
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
        assert!(compressed.len() < input.len(), "Repeated data should compress: {} vs {}", compressed.len(), input.len());
    }

    #[test]
    fn test_roundtrip_moby_start() {
        let input = b"Call me Ishmael. Some years ago--never mind how long precisely--having little or no money in my purse, and nothing particular to interest me on shore, I thought I would sail about a little and see the watery part of the world.";
        assert_eq!(decompress(&compress(input.as_slice())), input.as_slice());
    }

    #[test]
    fn test_random_data_no_blowup() {
        // Random data shouldn't expand by more than ~1%
        let mut input = Vec::with_capacity(10000);
        let mut state: u32 = 99999;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            input.push((state >> 16) as u8);
        }
        let compressed = compress(&input);
        // With batched literals, overhead is ~1 tag per 128 bytes + 4 byte header = ~82 bytes
        assert!(compressed.len() < input.len() + 200,
            "Random data expanded too much: {} -> {}", input.len(), compressed.len());
    }
}

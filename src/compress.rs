/// compress.rs — THE FILE THE AGENT EDITS
///
/// CURRENT ALGORITHM: LZ77 with hash chains, lazy matching, batched literals
///
/// Format (same as PR #3):
///   0xxxxxxx: literal run of (tag+1) bytes (1-128)
///   1xxxxxxx + offset_hi + offset_lo: match, length = (tag & 0x7F) + 3 (3-130)
///
/// New: lazy matching — before committing to a match, check if the next
/// position has a significantly better match. If so, emit a literal and
/// take the longer match instead.
///
/// New: better hash function (fewer collisions), quick-reject on chain walk.

use std::io::{self, Read, Write};

const WINDOW_SIZE: usize = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 130; // 0x7F + 3
const MAX_LITERAL_RUN: usize = 128;
const HASH_CHAIN_LIMIT: usize = 96;

struct HashChain {
    head: Vec<i32>,
    prev: Vec<i32>,
    mask: usize,
}

impl HashChain {
    fn new() -> Self {
        let size = 1 << 16; // 64K entries (was 32K — fewer collisions)
        HashChain {
            head: vec![-1i32; size],
            prev: vec![-1i32; WINDOW_SIZE],
            mask: size - 1,
        }
    }

    fn hash4(data: &[u8], pos: usize) -> usize {
        if pos + 3 >= data.len() {
            if pos + 2 >= data.len() { return 0; }
            return (data[pos] as usize).wrapping_mul(2654435761)
                ^ (data[pos + 1] as usize).wrapping_mul(40503)
                ^ (data[pos + 2] as usize);
        }
        // 4-byte hash for fewer collisions
        (data[pos] as usize).wrapping_mul(2654435761)
            ^ (data[pos + 1] as usize).wrapping_mul(2246822519)
            ^ (data[pos + 2] as usize).wrapping_mul(40503)
            ^ (data[pos + 3] as usize)
    }

    fn insert(&mut self, data: &[u8], pos: usize) {
        if pos + 2 >= data.len() { return; }
        let h = Self::hash4(data, pos) & self.mask;
        self.prev[pos % WINDOW_SIZE] = self.head[h];
        self.head[h] = pos as i32;
    }

    fn find_best_match(&self, data: &[u8], pos: usize) -> (usize, usize) {
        if pos + 2 >= data.len() { return (0, 0); }
        let h = Self::hash4(data, pos) & self.mask;
        let mut chain_pos = self.head[h];
        let mut best_len = MIN_MATCH - 1;
        let mut best_offset = 0usize;
        let mut chain_count = 0;
        let min_pos = pos.saturating_sub(WINDOW_SIZE);
        let max_len = MAX_MATCH.min(data.len() - pos);

        while chain_pos >= 0 && (chain_pos as usize) >= min_pos && chain_count < HASH_CHAIN_LIMIT {
            let cp = chain_pos as usize;
            if cp >= pos {
                chain_pos = self.prev[cp % WINDOW_SIZE];
                chain_count += 1;
                continue;
            }

            // Quick reject: check byte at current best length
            if data[cp + best_len] != data[pos + best_len] {
                chain_pos = self.prev[cp % WINDOW_SIZE];
                chain_count += 1;
                continue;
            }

            let mut length = 0;
            while length < max_len && data[cp + length] == data[pos + length] {
                length += 1;
            }

            if length > best_len {
                best_len = length;
                best_offset = pos - cp;
                if length == max_len { break; }
            }

            chain_pos = self.prev[cp % WINDOW_SIZE];
            chain_count += 1;
        }

        if best_len >= MIN_MATCH {
            (best_len, best_offset)
        } else {
            (0, 0)
        }
    }
}

#[derive(Debug)]
enum Token {
    Literal(u8),
    Match { length: usize, offset: usize },
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut tokens: Vec<Token> = Vec::new();
    let mut chain = HashChain::new();
    let mut pos = 0;

    while pos < input.len() {
        let (match_len, match_offset) = chain.find_best_match(input, pos);

        if match_len >= MIN_MATCH && match_offset <= 65535 {
            // Lazy matching: check next position
            let mut use_lazy = false;
            if match_len < MAX_MATCH && pos + 1 < input.len() {
                chain.insert(input, pos);
                let (next_len, next_offset) = chain.find_best_match(input, pos + 1);
                // Only take next match if it's meaningfully better
                // (longer by >= 2, since we pay 1 literal to defer)
                if next_len >= match_len + 2 && next_offset <= 65535 {
                    use_lazy = true;
                    tokens.push(Token::Literal(input[pos]));
                    pos += 1;
                    tokens.push(Token::Match { length: next_len, offset: next_offset });
                    for i in 0..next_len {
                        chain.insert(input, pos + i);
                    }
                    pos += next_len;
                }
            }

            if !use_lazy {
                tokens.push(Token::Match { length: match_len, offset: match_offset });
                // Insert was already done for pos if we checked lazy
                let start_insert = if match_len < MAX_MATCH && pos + 1 < input.len() { 1 } else { 0 };
                for i in start_insert..match_len {
                    chain.insert(input, pos + i);
                }
                if start_insert == 0 {
                    chain.insert(input, pos);
                }
                pos += match_len;
            }
        } else {
            tokens.push(Token::Literal(input[pos]));
            chain.insert(input, pos);
            pos += 1;
        }
    }

    // Encode
    let mut output = Vec::with_capacity(input.len());
    output.extend_from_slice(&(input.len() as u32).to_le_bytes());

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
    fn test_roundtrip_very_long_run() {
        let input = vec![0x42; 100000];
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
        let mut input = Vec::with_capacity(10000);
        let mut state: u32 = 99999;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            input.push((state >> 16) as u8);
        }
        let compressed = compress(&input);
        assert!(compressed.len() < input.len() + 200,
            "Random data expanded too much: {} -> {}", input.len(), compressed.len());
    }
}

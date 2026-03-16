/// compress.rs — THE FILE THE AGENT EDITS
///
/// ALGORITHM: LZ77 (optimal DP parsing) + Block Range Coding
///
/// Pipeline:
///   1. LZ77 with hash chains + DP optimal parsing → token stream
///   2. Split into blocks
///   3. Each block: build frequency tables, range-encode symbols
///
/// Range coding replaces Huffman for fractional-bit precision (~1-3% better).

use std::io::{self, Read, Write};

const WINDOW_SIZE: usize = 1048576; // 1MB window
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const HASH_CHAIN_LIMIT: usize = 512;
const BLOCK_SIZE: usize = 32768;

// ─── DEFLATE length/distance tables ──────────────────────────────────────

fn length_to_code(length: usize) -> (u16, u8, u16) {
    match length {
        3 => (257, 0, 0), 4 => (258, 0, 0), 5 => (259, 0, 0),
        6 => (260, 0, 0), 7 => (261, 0, 0), 8 => (262, 0, 0),
        9 => (263, 0, 0), 10 => (264, 0, 0),
        11..=12 => (265, 1, (length - 11) as u16),
        13..=14 => (266, 1, (length - 13) as u16),
        15..=16 => (267, 1, (length - 15) as u16),
        17..=18 => (268, 1, (length - 17) as u16),
        19..=22 => (269, 2, (length - 19) as u16),
        23..=26 => (270, 2, (length - 23) as u16),
        27..=30 => (271, 2, (length - 27) as u16),
        31..=34 => (272, 2, (length - 31) as u16),
        35..=42 => (273, 3, (length - 35) as u16),
        43..=50 => (274, 3, (length - 43) as u16),
        51..=58 => (275, 3, (length - 51) as u16),
        59..=66 => (276, 3, (length - 59) as u16),
        67..=82 => (277, 4, (length - 67) as u16),
        83..=98 => (278, 4, (length - 83) as u16),
        99..=114 => (279, 4, (length - 99) as u16),
        115..=130 => (280, 4, (length - 115) as u16),
        131..=162 => (281, 5, (length - 131) as u16),
        163..=194 => (282, 5, (length - 163) as u16),
        195..=226 => (283, 5, (length - 195) as u16),
        227..=257 => (284, 5, (length - 227) as u16),
        258 => (285, 0, 0),
        _ => (285, 0, 0),
    }
}

fn code_to_length_base(code: u16) -> (usize, u8) {
    match code {
        257 => (3, 0), 258 => (4, 0), 259 => (5, 0), 260 => (6, 0),
        261 => (7, 0), 262 => (8, 0), 263 => (9, 0), 264 => (10, 0),
        265 => (11, 1), 266 => (13, 1), 267 => (15, 1), 268 => (17, 1),
        269 => (19, 2), 270 => (23, 2), 271 => (27, 2), 272 => (31, 2),
        273 => (35, 3), 274 => (43, 3), 275 => (51, 3), 276 => (59, 3),
        277 => (67, 4), 278 => (83, 4), 279 => (99, 4), 280 => (115, 4),
        281 => (131, 5), 282 => (163, 5), 283 => (195, 5), 284 => (227, 5),
        285 => (258, 0),
        _ => (0, 0),
    }
}

fn offset_to_code(offset: usize) -> (u8, u8, u32) {
    match offset {
        1 => (0, 0, 0), 2 => (1, 0, 0), 3 => (2, 0, 0), 4 => (3, 0, 0),
        5..=6 => (4, 1, (offset - 5) as u32),
        7..=8 => (5, 1, (offset - 7) as u32),
        9..=12 => (6, 2, (offset - 9) as u32),
        13..=16 => (7, 2, (offset - 13) as u32),
        17..=24 => (8, 3, (offset - 17) as u32),
        25..=32 => (9, 3, (offset - 25) as u32),
        33..=48 => (10, 4, (offset - 33) as u32),
        49..=64 => (11, 4, (offset - 49) as u32),
        65..=96 => (12, 5, (offset - 65) as u32),
        97..=128 => (13, 5, (offset - 97) as u32),
        129..=192 => (14, 6, (offset - 129) as u32),
        193..=256 => (15, 6, (offset - 193) as u32),
        257..=384 => (16, 7, (offset - 257) as u32),
        385..=512 => (17, 7, (offset - 385) as u32),
        513..=768 => (18, 8, (offset - 513) as u32),
        769..=1024 => (19, 8, (offset - 769) as u32),
        1025..=1536 => (20, 9, (offset - 1025) as u32),
        1537..=2048 => (21, 9, (offset - 1537) as u32),
        2049..=3072 => (22, 10, (offset - 2049) as u32),
        3073..=4096 => (23, 10, (offset - 3073) as u32),
        4097..=6144 => (24, 11, (offset - 4097) as u32),
        6145..=8192 => (25, 11, (offset - 6145) as u32),
        8193..=12288 => (26, 12, (offset - 8193) as u32),
        12289..=16384 => (27, 12, (offset - 12289) as u32),
        16385..=24576 => (28, 13, (offset - 16385) as u32),
        24577..=32768 => (29, 13, (offset - 24577) as u32),
        32769..=49152 => (30, 14, (offset - 32769) as u32),
        49153..=65536 => (31, 14, (offset - 49153) as u32),
        65537..=98304 => (32, 15, (offset - 65537) as u32),
        98305..=131072 => (33, 15, (offset - 98305) as u32),
        131073..=196608 => (34, 16, (offset - 131073) as u32),
        196609..=262144 => (35, 16, (offset - 196609) as u32),
        262145..=393216 => (36, 17, (offset - 262145) as u32),
        393217..=524288 => (37, 17, (offset - 393217) as u32),
        524289..=786432 => (38, 18, (offset - 524289) as u32),
        786433..=1048576 => (39, 18, (offset - 786433) as u32),
        _ => (39, 18, 0),
    }
}

fn code_to_offset_base(code: u8) -> (usize, u8) {
    match code {
        0 => (1, 0), 1 => (2, 0), 2 => (3, 0), 3 => (4, 0),
        4 => (5, 1), 5 => (7, 1), 6 => (9, 2), 7 => (13, 2),
        8 => (17, 3), 9 => (25, 3), 10 => (33, 4), 11 => (49, 4),
        12 => (65, 5), 13 => (97, 5), 14 => (129, 6), 15 => (193, 6),
        16 => (257, 7), 17 => (385, 7), 18 => (513, 8), 19 => (769, 8),
        20 => (1025, 9), 21 => (1537, 9), 22 => (2049, 10), 23 => (3073, 10),
        24 => (4097, 11), 25 => (6145, 11), 26 => (8193, 12), 27 => (12289, 12),
        28 => (16385, 13), 29 => (24577, 13),
        30 => (32769, 14), 31 => (49153, 14),
        32 => (65537, 15), 33 => (98305, 15),
        34 => (131073, 16), 35 => (196609, 16),
        36 => (262145, 17), 37 => (393217, 17),
        38 => (524289, 18), 39 => (786433, 18),
        _ => (0, 0),
    }
}

// ─── Hash Chain ──────────────────────────────────────────────────────────

struct HashChain {
    head: Vec<i32>,
    prev: Vec<i32>,
    mask: usize,
}

impl HashChain {
    fn new() -> Self {
        let size = 1 << 16;
        HashChain { head: vec![-1i32; size], prev: vec![-1i32; WINDOW_SIZE], mask: size - 1 }
    }

    fn hash4(data: &[u8], pos: usize) -> usize {
        if pos + 3 >= data.len() {
            if pos + 2 >= data.len() { return 0; }
            return (data[pos] as usize).wrapping_mul(2654435761)
                ^ (data[pos + 1] as usize).wrapping_mul(40503)
                ^ (data[pos + 2] as usize);
        }
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

    fn find_matches(&self, data: &[u8], pos: usize) -> Vec<(usize, usize)> {
        let mut results = Vec::new();
        if pos + 2 >= data.len() { return results; }
        let h = Self::hash4(data, pos) & self.mask;
        let mut cp = self.head[h];
        let mut count = 0;
        let min_p = pos.saturating_sub(WINDOW_SIZE);
        let max_l = MAX_MATCH.min(data.len() - pos);
        let mut best_len = MIN_MATCH - 1;

        while cp >= 0 && (cp as usize) >= min_p && count < HASH_CHAIN_LIMIT {
            let c = cp as usize;
            if c < pos {
                let mut l = 0;
                while l < max_l && data[c + l] == data[pos + l] { l += 1; }
                if l > best_len {
                    results.push((l, pos - c));
                    best_len = l;
                    if l == max_l { break; }
                }
            }
            cp = self.prev[c % WINDOW_SIZE];
            count += 1;
        }
        results
    }
}

// ─── LZ77 Token + Optimal Parsing ───────────────────────────────────────

enum Token { Literal(u8), Match { length: usize, offset: usize } }

fn lz77_tokenize(input: &[u8]) -> Vec<Token> {
    if input.is_empty() { return Vec::new(); }
    let n = input.len();
    let mut chain = HashChain::new();

    let mut all_matches: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for pos in 0..n {
        all_matches[pos] = chain.find_matches(input, pos);
        chain.insert(input, pos);
    }

    // Iterative DP: converge cost estimates over 2 iterations
    let mut ll_lens = vec![8u8; 286];
    let mut d_lens = vec![5u8; 40];
    let mut choice: Vec<(usize, usize)> = vec![(1, 0); n + 1];
    for _iter in 0..2 {
        let mut cost = vec![u32::MAX / 2; n + 1];
        choice = vec![(1, 0); n + 1];
        cost[n] = 0;
        for pos in (0..n).rev() {
            let lb = ll_lens[input[pos] as usize];
            let lc = (if lb == 0 { 15 } else { lb as u32 }) + cost[pos + 1];
            if lc < cost[pos] { cost[pos] = lc; choice[pos] = (1, 0); }
            for &(len, off) in &all_matches[pos] {
                let (lcode, leb, _) = length_to_code(len);
                let ll = ll_lens[lcode as usize];
                let ll_c = if ll == 0 { 15 } else { ll as u32 };
                let (dcode, deb, _) = offset_to_code(off);
                let dl = d_lens[dcode as usize];
                let dl_c = if dl == 0 { 15 } else { dl as u32 };
                let mc = ll_c + leb as u32 + dl_c + deb as u32 + cost[pos + len];
                if mc < cost[pos] { cost[pos] = mc; choice[pos] = (len, off); }
                for &sl in &[3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,67,83,99,115,131] {
                    if sl >= MIN_MATCH && sl < len {
                        let (slc, sleb, _) = length_to_code(sl);
                        let sll = ll_lens[slc as usize];
                        let sll_c = if sll == 0 { 15 } else { sll as u32 };
                        let smc = sll_c + sleb as u32 + dl_c + deb as u32 + cost[pos + sl];
                        if smc < cost[pos] { cost[pos] = smc; choice[pos] = (sl, off); }
                    }
                }
            }
        }
        let mut ll_freq = vec![1u32; 286];
        let mut d_freq = vec![1u32; 40];
        let mut p = 0;
        while p < n {
            let (len, off) = choice[p];
            if off == 0 { ll_freq[input[p] as usize] += 1; p += 1; }
            else {
                let (lc, _, _) = length_to_code(len);
                ll_freq[lc as usize] += 1;
                let (dc, _, _) = offset_to_code(off);
                d_freq[dc as usize] += 1;
                p += len;
            }
        }
        ll_freq[256] += 1;
        ll_lens = build_code_lengths(&ll_freq, 15);
        d_lens = build_code_lengths(&d_freq, 15);
    }

    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < n {
        let (len, off) = choice[pos];
        if off == 0 { tokens.push(Token::Literal(input[pos])); pos += 1; }
        else { tokens.push(Token::Match { length: len, offset: off }); pos += len; }
    }
    tokens
}

// ─── Bit I/O ─────────────────────────────────────────────────────────────

struct BitWriter { bytes: Vec<u8>, buf: u64, nbits: u32 }

impl BitWriter {
    fn new() -> Self { BitWriter { bytes: Vec::new(), buf: 0, nbits: 0 } }
    fn write_bits(&mut self, val: u32, bits: u32) {
        self.buf |= (val as u64) << self.nbits;
        self.nbits += bits;
        while self.nbits >= 8 { self.bytes.push(self.buf as u8); self.buf >>= 8; self.nbits -= 8; }
    }
    fn flush(mut self) -> Vec<u8> { if self.nbits > 0 { self.bytes.push(self.buf as u8); } self.bytes }
}

struct BitReader<'a> { data: &'a [u8], byte_pos: usize, bit_pos: u32 }

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self { BitReader { data, byte_pos: 0, bit_pos: 0 } }
    fn read_bits(&mut self, bits: u32) -> u32 {
        let mut val = 0u32;
        let mut br = 0u32;
        while br < bits {
            if self.byte_pos >= self.data.len() { return val; }
            let byte = self.data[self.byte_pos];
            let avail = 8 - self.bit_pos;
            let take = (bits - br).min(avail);
            let mask = ((1u32 << take) - 1) as u8;
            val |= (((byte >> self.bit_pos) & mask) as u32) << br;
            br += take;
            self.bit_pos += take;
            if self.bit_pos >= 8 { self.bit_pos = 0; self.byte_pos += 1; }
        }
        val
    }
}

// ─── Huffman ─────────────────────────────────────────────────────────────

fn build_code_lengths(freqs: &[u32], max_bits: u8) -> Vec<u8> {
    let n = freqs.len();
    let mut lengths = vec![0u8; n];
    let active: Vec<usize> = (0..n).filter(|&i| freqs[i] > 0).collect();
    if active.is_empty() { return lengths; }
    if active.len() == 1 { lengths[active[0]] = 1; return lengths; }

    let na = active.len();
    let total = 2 * na - 1;
    let mut freq = vec![0u64; total];
    let mut left_child = vec![0usize; total];
    let mut right_child = vec![0usize; total];
    let mut avail = vec![false; total];
    for (idx, &sym) in active.iter().enumerate() { freq[idx] = freqs[sym] as u64; avail[idx] = true; }

    let mut next = na;
    for _ in 0..na - 1 {
        let (mut m1, mut m2) = (usize::MAX, usize::MAX);
        let (mut f1, mut f2) = (u64::MAX, u64::MAX);
        for i in 0..next {
            if !avail[i] { continue; }
            if freq[i] < f1 || (freq[i] == f1 && i < m1) { m2 = m1; f2 = f1; m1 = i; f1 = freq[i]; }
            else if freq[i] < f2 || (freq[i] == f2 && i < m2) { m2 = i; f2 = freq[i]; }
        }
        if m2 == usize::MAX { break; }
        freq[next] = f1 + f2;
        left_child[next] = m1; right_child[next] = m2;
        avail[m1] = false; avail[m2] = false; avail[next] = true;
        next += 1;
    }

    let root = next - 1;
    let mut depth = vec![0u8; total];
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node < na { lengths[active[node]] = depth[node]; }
        else {
            let l = left_child[node]; let r = right_child[node];
            depth[l] = depth[node] + 1; depth[r] = depth[node] + 1;
            stack.push(l); stack.push(r);
        }
    }

    if lengths.iter().any(|&l| l > max_bits) {
        let mut sorted: Vec<(usize, u32)> = active.iter().map(|&i| (i, freqs[i])).collect();
        sorted.sort_by_key(|&(_, f)| f);
        for &(sym, _) in &sorted { lengths[sym] = max_bits; }
        let target = 1u64 << max_bits;
        let mut kraft: u64 = sorted.len() as u64;
        for &(sym, _) in sorted.iter().rev() {
            let cur = lengths[sym];
            for new_len in 1..cur {
                let cost_change = (1u64 << (max_bits - new_len)) - (1u64 << (max_bits - cur));
                if kraft + cost_change <= target { kraft += cost_change; lengths[sym] = new_len; break; }
            }
        }
    }
    for &i in &active { if lengths[i] == 0 { lengths[i] = max_bits; } }
    lengths
}

fn canonical_codes(lengths: &[u8]) -> Vec<(u32, u8)> {
    let n = lengths.len();
    let mut codes = vec![(0u32, 0u8); n];
    let mut syms: Vec<(usize, u8)> = lengths.iter().enumerate().filter(|(_, &b)| b > 0).map(|(i, &b)| (i, b)).collect();
    syms.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut code: u32 = 0; let mut prev = 0u8;
    for &(sym, bits) in &syms {
        if prev > 0 { code += 1; }
        if bits > prev { code <<= bits - prev; }
        codes[sym] = (code, bits); prev = bits;
    }
    codes
}

fn rev_bits(val: u32, bits: u8) -> u32 {
    let mut r = 0u32; let mut v = val;
    for _ in 0..bits { r = (r << 1) | (v & 1); v >>= 1; } r
}

fn build_decode_table(lengths: &[u8]) -> (Vec<u16>, Vec<u8>, u8) {
    let max_bits = lengths.iter().copied().max().unwrap_or(1).max(1);
    let size = 1usize << max_bits;
    let mut sym_table = vec![0u16; size];
    let mut len_table = vec![0u8; size];
    let codes = canonical_codes(lengths);
    for (sym, &(code, bits)) in codes.iter().enumerate() {
        if bits == 0 { continue; }
        let rev = rev_bits(code, bits) as usize;
        let fill = 1usize << (max_bits - bits);
        for j in 0..fill { let idx = rev | (j << bits); sym_table[idx] = sym as u16; len_table[idx] = bits; }
    }
    (sym_table, len_table, max_bits)
}

fn decode_sym(reader: &mut BitReader, sym_table: &[u16], len_table: &[u8], max_bits: u8) -> u16 {
    let bits = reader.read_bits(max_bits as u32);
    let idx = bits as usize & (sym_table.len() - 1);
    let sym = sym_table[idx];
    let len = len_table[idx];
    if len < max_bits && len > 0 {
        let total_bit_pos = reader.byte_pos as u32 * 8 + reader.bit_pos;
        let new_total = total_bit_pos - (max_bits - len) as u32;
        reader.byte_pos = (new_total / 8) as usize;
        reader.bit_pos = new_total % 8;
    }
    sym
}

// ─── BWT (Burrows-Wheeler Transform) ─────────────────────────────────────

fn bwt_forward(data: &[u8]) -> (Vec<u8>, u32) {
    let n = data.len();
    if n == 0 { return (Vec::new(), 0); }
    // Sort rotation indices using cyclic comparison
    let mut indices: Vec<u32> = (0..n as u32).collect();
    // Use radix + merge sort approach for speed
    indices.sort_unstable_by(|&a, &b| {
        let a = a as usize;
        let b = b as usize;
        // Compare up to n bytes cyclically
        let mut i = 0;
        while i < n {
            // Compare in chunks for speed
            let ca = data[(a + i) % n];
            let cb = data[(b + i) % n];
            if ca != cb { return ca.cmp(&cb); }
            i += 1;
        }
        std::cmp::Ordering::Equal
    });
    let mut output = Vec::with_capacity(n);
    let mut orig_idx = 0u32;
    for (i, &s) in indices.iter().enumerate() {
        if s == 0 { orig_idx = i as u32; }
        output.push(data[(s as usize + n - 1) % n]);
    }
    (output, orig_idx)
}

fn bwt_inverse(bwt: &[u8], orig_idx: u32) -> Vec<u8> {
    let n = bwt.len();
    if n == 0 { return Vec::new(); }
    let mut count = [0usize; 256];
    for &b in bwt { count[b as usize] += 1; }
    let mut cumul = [0usize; 256];
    let mut sum = 0;
    for i in 0..256 { cumul[i] = sum; sum += count[i]; }
    let mut t = vec![0usize; n];
    let mut c = cumul;
    for i in 0..n { let b = bwt[i] as usize; t[i] = c[b]; c[b] += 1; }
    let mut output = vec![0u8; n];
    let mut idx = orig_idx as usize;
    for i in (0..n).rev() { output[i] = bwt[idx]; idx = t[idx]; }
    output
}

fn mtf_encode(data: &[u8]) -> Vec<u8> {
    let mut list = [0u8; 256];
    for i in 0..256 { list[i] = i as u8; }
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        let pos = list.iter().position(|&x| x == b).unwrap_or(0);
        out.push(pos as u8);
        for i in (1..=pos).rev() { list[i] = list[i - 1]; }
        list[0] = b;
    }
    out
}

fn mtf_decode(data: &[u8]) -> Vec<u8> {
    let mut list = [0u8; 256];
    for i in 0..256 { list[i] = i as u8; }
    let mut out = Vec::with_capacity(data.len());
    for &pos in data {
        let b = list[pos as usize];
        out.push(b);
        for i in (1..=pos as usize).rev() { list[i] = list[i - 1]; }
        list[0] = b;
    }
    out
}

/// RLE encode MTF output: replaces runs of zeros with run-length codes
/// Symbols: 0 = RUNA, 1 = RUNB (encode zero runs), 2-256 = MTF values 1-255 shifted
/// Run encoding: run of N zeros → encode (N) as binary digits using RUNA(=+1)/RUNB(=+2) in power-of-2 positions
fn rle_zero_encode(data: &[u8]) -> Vec<u16> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0 {
            let mut run = 0usize;
            while i < data.len() && data[i] == 0 { run += 1; i += 1; }
            // Encode run using bijective base-2: RUNA adds 1*power, RUNB adds 2*power
            while run > 0 {
                if run % 2 == 1 { out.push(0); } // RUNA
                else { out.push(1); } // RUNB
                run = (run - 1) / 2;
            }
        } else {
            out.push(data[i] as u16 + 1); // shift: MTF value k → symbol k+1
            i += 1;
        }
    }
    out
}

fn rle_zero_decode(data: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] <= 1 {
            // Decode zero run
            let mut run = 0usize;
            let mut power = 1usize;
            while i < data.len() && data[i] <= 1 {
                run += (data[i] as usize + 1) * power;
                power *= 2;
                i += 1;
            }
            for _ in 0..run { out.push(0); }
        } else {
            out.push((data[i] - 1) as u8);
            i += 1;
        }
    }
    out
}

// ─── Range coder for BWT pipeline ───────────────────────────────────────

const RC_TOP: u32 = 1 << 24;
const RC_BOT: u32 = 1 << 16;

struct RcModel {
    freq: Vec<u32>,
    cum: Vec<u32>,
    total: u32,
    nsym: usize,
}

impl RcModel {
    fn new(nsym: usize) -> Self {
        let mut m = RcModel { freq: vec![1; nsym], cum: vec![0; nsym + 1], total: nsym as u32, nsym };
        m.rebuild();
        m
    }
    fn rebuild(&mut self) {
        self.cum[0] = 0;
        for i in 0..self.nsym { self.cum[i+1] = self.cum[i] + self.freq[i]; }
        self.total = self.cum[self.nsym];
    }
    fn update(&mut self, sym: usize) {
        self.freq[sym] += 1;
        self.total += 1;
        if self.total > RC_BOT / 2 {
            self.total = 0;
            for i in 0..self.nsym { self.freq[i] = (self.freq[i] + 1) / 2; self.total += self.freq[i]; }
            self.rebuild();
        } else {
            for i in (sym+1)..=self.nsym { self.cum[i] += 1; }
        }
    }
}

struct RcEncoder { low: u32, range: u32, buf: Vec<u8> }

impl RcEncoder {
    fn new() -> Self { RcEncoder { low: 0, range: 0xFFFFFFFF, buf: Vec::new() } }
    fn encode(&mut self, model: &mut RcModel, sym: usize) {
        let r = self.range / model.total;
        self.low += r * model.cum[sym];
        if sym + 1 < model.nsym { self.range = r * (model.cum[sym+1] - model.cum[sym]); }
        else { self.range -= r * model.cum[sym]; }
        while (self.low ^ self.low.wrapping_add(self.range)) < RC_TOP || self.range < RC_BOT {
            if (self.low ^ self.low.wrapping_add(self.range)) >= RC_TOP {
                self.range = self.low.wrapping_neg() & (RC_BOT - 1);
            }
            self.buf.push((self.low >> 24) as u8);
            self.low <<= 8;
            self.range <<= 8;
        }
        model.update(sym);
    }
    fn encode_raw(&mut self, val: u32, nbits: u32) {
        for i in 0..nbits {
            let bit = (val >> i) & 1;
            let half = self.range >> 1;
            if bit != 0 {
                self.low = self.low.wrapping_add(half);
                self.range -= half;
            } else {
                self.range = half;
            }
            while (self.low ^ self.low.wrapping_add(self.range)) < RC_TOP || self.range < RC_BOT {
                if (self.low ^ self.low.wrapping_add(self.range)) >= RC_TOP {
                    self.range = self.low.wrapping_neg() & (RC_BOT - 1);
                }
                self.buf.push((self.low >> 24) as u8);
                self.low <<= 8;
                self.range <<= 8;
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        for _ in 0..4 { self.buf.push((self.low >> 24) as u8); self.low <<= 8; }
        self.buf
    }
}

struct RcDecoder<'a> { low: u32, range: u32, code: u32, data: &'a [u8], pos: usize }

impl<'a> RcDecoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut d = RcDecoder { low: 0, range: 0xFFFFFFFF, code: 0, data, pos: 0 };
        for _ in 0..4 { d.code = (d.code << 8) | d.byte() as u32; }
        d
    }
    fn byte(&mut self) -> u8 {
        if self.pos < self.data.len() { let b = self.data[self.pos]; self.pos += 1; b } else { 0 }
    }
    fn decode_raw(&mut self, nbits: u32) -> u32 {
        let mut val = 0u32;
        for i in 0..nbits {
            let half = self.range >> 1;
            if (self.code.wrapping_sub(self.low)) >= half {
                val |= 1 << i;
                self.low = self.low.wrapping_add(half);
                self.range -= half;
            } else {
                self.range = half;
            }
            while (self.low ^ self.low.wrapping_add(self.range)) < RC_TOP || self.range < RC_BOT {
                if (self.low ^ self.low.wrapping_add(self.range)) >= RC_TOP {
                    self.range = self.low.wrapping_neg() & (RC_BOT - 1);
                }
                self.code = (self.code << 8) | self.byte() as u32;
                self.low <<= 8;
                self.range <<= 8;
            }
        }
        val
    }
    fn decode(&mut self, model: &mut RcModel) -> usize {
        let r = self.range / model.total;
        let t = ((self.code - self.low) / r).min(model.total - 1);
        let mut lo = 0usize; let mut hi = model.nsym;
        while lo < hi { let mid = (lo+hi)/2; if model.cum[mid+1] <= t { lo = mid+1; } else { hi = mid; } }
        let sym = lo;
        self.low += r * model.cum[sym];
        if sym + 1 < model.nsym { self.range = r * (model.cum[sym+1] - model.cum[sym]); }
        else { self.range -= r * model.cum[sym]; }
        while (self.low ^ self.low.wrapping_add(self.range)) < RC_TOP || self.range < RC_BOT {
            if (self.low ^ self.low.wrapping_add(self.range)) >= RC_TOP {
                self.range = self.low.wrapping_neg() & (RC_BOT - 1);
            }
            self.code = (self.code << 8) | self.byte() as u32;
            self.low <<= 8;
            self.range <<= 8;
        }
        model.update(sym);
        sym
    }
}

/// BWT pipeline with two sub-modes:
/// Mode A (0): BWT → MTF → RLE → order-0 range coding (original — good for repetitive/structured)
/// Mode B (1): BWT → MTF → order-1 bit-level range coding (good for text)

fn mtf_ctx(val: u8) -> usize {
    match val {
        0 => 0,
        1 => 1,
        2..=3 => 2,
        4..=7 => 3,
        8..=15 => 4,
        16..=31 => 5,
        32..=63 => 6,
        _ => 7,
    }
}

const MTF_CTX_GROUPS: usize = 8;

fn bwt_compress_mode_a(mtf_data: &[u8]) -> Vec<u8> {
    let rle_data = rle_zero_encode(mtf_data);
    let mut model = RcModel::new(258);
    let mut enc = RcEncoder::new();
    for &s in &rle_data { enc.encode(&mut model, s as usize); }
    enc.encode(&mut model, 257); // EOB
    enc.finish()
}

fn bwt_compress_mode_b(mtf_data: &[u8]) -> Vec<u8> {
    let mut lit_model = BitModel::new(MTF_CTX_GROUPS * LIT_TREE_SIZE);
    let mut enc = RcEncoder::new();
    let mut prev_ctx: usize = 0;
    for &b in mtf_data {
        encode_literal_byte(&mut enc, &mut lit_model, prev_ctx, b);
        prev_ctx = mtf_ctx(b);
    }
    enc.finish()
}

/// Mode C: BWT → MTF → order-2 bit-level with quantized contexts
fn bwt_compress_mode_c(mtf_data: &[u8]) -> Vec<u8> {
    let ctx_count = MTF_CTX_GROUPS * MTF_CTX_GROUPS; // 64 contexts
    let mut lit_model = BitModel::new(ctx_count * LIT_TREE_SIZE);
    let mut enc = RcEncoder::new();
    let mut prev1: usize = 0;
    let mut prev2: usize = 0;
    for &b in mtf_data {
        let ctx = prev2 * MTF_CTX_GROUPS + prev1;
        encode_literal_byte(&mut enc, &mut lit_model, ctx, b);
        prev2 = prev1;
        prev1 = mtf_ctx(b);
    }
    enc.finish()
}

/// Mode E: BWT → MTF → order-3 bit-level with quantized contexts
fn bwt_compress_mode_e(mtf_data: &[u8]) -> Vec<u8> {
    let ctx_count = MTF_CTX_GROUPS * MTF_CTX_GROUPS * MTF_CTX_GROUPS; // 512 contexts
    let mut lit_model = BitModel::new(ctx_count * LIT_TREE_SIZE);
    let mut enc = RcEncoder::new();
    let mut prev1: usize = 0;
    let mut prev2: usize = 0;
    let mut prev3: usize = 0;
    for &b in mtf_data {
        let ctx = (prev3 * MTF_CTX_GROUPS + prev2) * MTF_CTX_GROUPS + prev1;
        encode_literal_byte(&mut enc, &mut lit_model, ctx, b);
        prev3 = prev2;
        prev2 = prev1;
        prev1 = mtf_ctx(b);
    }
    enc.finish()
}

/// Mode D: BWT → MTF → zero/nonzero split coding
/// For each byte: first bit = is_zero (contexted by prev MTF quantized)
/// If zero: count run length, encode with Golomb-like coding
/// If non-zero: encode value (1-255) with bit-tree (8 bits, ctx = prev quantized)
fn bwt_compress_mode_d(mtf_data: &[u8]) -> Vec<u8> {
    // Context: previous non-zero value quantized (8 groups)
    let ctx_groups = 8usize;
    // is_zero model: per context
    let mut zero_model = BitModel::new(ctx_groups);
    // run length model: encode run bits (up to 24 bits) with flat context
    let mut run_model = BitModel::new(2 * 32); // 2 contexts (continue/value) × 32 bit positions
    // non-zero value model: 8 contexts × 256 tree nodes (encode 0-254 → actual value 1-255)
    let mut val_model = BitModel::new(ctx_groups * LIT_TREE_SIZE);
    let mut enc = RcEncoder::new();
    
    let mut prev_ctx: usize = 0;
    let mut i = 0;
    while i < mtf_data.len() {
        if mtf_data[i] == 0 {
            rc_encode_bit(&mut enc, &mut zero_model, prev_ctx, 1); // is zero
            // Count run
            let mut run = 0usize;
            while i < mtf_data.len() && mtf_data[i] == 0 { run += 1; i += 1; }
            // Encode run length with unary-like coding:
            // For each bit position, encode whether run continues, then MSB first
            // Use Elias gamma-like: encode (run_bits - 1) in unary, then run value
            let bits_needed = if run == 0 { 1 } else { 32 - (run as u32).leading_zeros() };
            // Unary part: encode (bits_needed - 1) ones then a zero
            for b in 0..bits_needed - 1 {
                rc_encode_bit(&mut enc, &mut run_model, b as usize, 1); // continue
            }
            rc_encode_bit(&mut enc, &mut run_model, (bits_needed - 1).min(31) as usize, 0); // stop
            // Binary part: encode remaining bits (skip MSB which is always 1)
            for b in (0..bits_needed - 1).rev() {
                let bit = ((run >> b) & 1) as u32;
                enc.encode_raw(bit, 1);
            }
            // prev_ctx stays the same (zeros don't change context)
        } else {
            rc_encode_bit(&mut enc, &mut zero_model, prev_ctx, 0); // not zero
            let val = mtf_data[i] - 1; // encode 0-254 for values 1-255
            encode_literal_byte(&mut enc, &mut val_model, prev_ctx, val);
            prev_ctx = mtf_ctx(mtf_data[i]);
            i += 1;
        }
    }
    enc.finish()
}

fn bwt_compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let bwt_block_size = 1_500_000usize;
    let mut out = Vec::new();
    out.extend_from_slice(&(input.len() as u32).to_le_bytes());
    let num_blocks = (input.len() + bwt_block_size - 1) / bwt_block_size;
    out.extend_from_slice(&(num_blocks as u32).to_le_bytes());
    
    for chunk in input.chunks(bwt_block_size) {
        let (bwt_data, orig_idx) = bwt_forward(chunk);
        let mtf_data = mtf_encode(&bwt_data);
        
        let enc_a = bwt_compress_mode_a(&mtf_data);
        let enc_b = bwt_compress_mode_b(&mtf_data);
        let enc_c = bwt_compress_mode_c(&mtf_data);
        let enc_d = bwt_compress_mode_d(&mtf_data);
        let enc_e = bwt_compress_mode_e(&mtf_data);
        
        out.extend_from_slice(&orig_idx.to_le_bytes());
        
        // Pick smallest mode. Mode A has no block_len header, B/C/D/E do.
        let size_a = 1 + 4 + enc_a.len();
        let size_b = 1 + 4 + 4 + enc_b.len();
        let size_c = 1 + 4 + 4 + enc_c.len();
        let size_d = 1 + 4 + 4 + enc_d.len();
        let size_e = 1 + 4 + 4 + enc_e.len();
        
        let mut best_mode = 0u8;
        let mut best_size = size_a;
        if size_b < best_size { best_mode = 1; best_size = size_b; }
        if size_c < best_size { best_mode = 2; best_size = size_c; }
        if size_d < best_size { best_mode = 3; best_size = size_d; }
        if size_e < best_size { best_mode = 4; best_size = size_e; }
        
        match best_mode {
            0 => {
                out.push(0u8);
                out.extend_from_slice(&(enc_a.len() as u32).to_le_bytes());
                out.extend_from_slice(&enc_a);
            }
            1 => {
                out.push(1u8);
                out.extend_from_slice(&(mtf_data.len() as u32).to_le_bytes());
                out.extend_from_slice(&(enc_b.len() as u32).to_le_bytes());
                out.extend_from_slice(&enc_b);
            }
            2 => {
                out.push(2u8);
                out.extend_from_slice(&(mtf_data.len() as u32).to_le_bytes());
                out.extend_from_slice(&(enc_c.len() as u32).to_le_bytes());
                out.extend_from_slice(&enc_c);
            }
            3 => {
                out.push(3u8);
                out.extend_from_slice(&(mtf_data.len() as u32).to_le_bytes());
                out.extend_from_slice(&(enc_d.len() as u32).to_le_bytes());
                out.extend_from_slice(&enc_d);
            }
            _ => {
                out.push(4u8);
                out.extend_from_slice(&(mtf_data.len() as u32).to_le_bytes());
                out.extend_from_slice(&(enc_e.len() as u32).to_le_bytes());
                out.extend_from_slice(&enc_e);
            }
        }
    }
    out
}

fn bwt_decompress(input: &[u8]) -> Vec<u8> {
    if input.len() < 8 { return Vec::new(); }
    let orig_size = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let num_blocks = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    let mut pos = 8;
    let mut output = Vec::with_capacity(orig_size);
    
    for _ in 0..num_blocks {
        if pos + 5 > input.len() { break; }
        let orig_idx = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]); pos += 4;
        let block_mode = input[pos]; pos += 1;
        
        if block_mode == 0 {
            // Mode A: RLE + order-0 range coding
            if pos + 4 > input.len() { break; }
            let enc_size = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            if pos + enc_size > input.len() { break; }
            
            let mut model = RcModel::new(258);
            let mut dec = RcDecoder::new(&input[pos..pos+enc_size]); pos += enc_size;
            
            let mut rle_data: Vec<u16> = Vec::new();
            loop {
                let sym = dec.decode(&mut model);
                if sym == 257 { break; }
                if rle_data.len() > orig_size * 2 { break; }
                rle_data.push(sym as u16);
            }
            
            let mtf_data = rle_zero_decode(&rle_data);
            let bwt_data = mtf_decode(&mtf_data);
            let original = bwt_inverse(&bwt_data, orig_idx);
            output.extend_from_slice(&original);
        } else if block_mode == 1 {
            // Mode B: order-1 bit-level range coding (8 quantized contexts)
            if pos + 8 > input.len() { break; }
            let block_len = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            let enc_size = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            if pos + enc_size > input.len() { break; }
            
            let mut lit_model = BitModel::new(MTF_CTX_GROUPS * LIT_TREE_SIZE);
            let mut dec = RcDecoder::new(&input[pos..pos+enc_size]); pos += enc_size;
            
            let mut mtf_data = Vec::with_capacity(block_len);
            let mut prev_ctx: usize = 0;
            for _ in 0..block_len {
                let b = decode_literal_byte(&mut dec, &mut lit_model, prev_ctx);
                mtf_data.push(b);
                prev_ctx = mtf_ctx(b);
            }
            
            let bwt_data = mtf_decode(&mtf_data);
            let original = bwt_inverse(&bwt_data, orig_idx);
            output.extend_from_slice(&original);
        } else if block_mode == 2 {
            // Mode C: order-2 bit-level range coding (64 quantized contexts)
            if pos + 8 > input.len() { break; }
            let block_len = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            let enc_size = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            if pos + enc_size > input.len() { break; }
            
            let ctx_count = MTF_CTX_GROUPS * MTF_CTX_GROUPS;
            let mut lit_model = BitModel::new(ctx_count * LIT_TREE_SIZE);
            let mut dec = RcDecoder::new(&input[pos..pos+enc_size]); pos += enc_size;
            
            let mut mtf_data = Vec::with_capacity(block_len);
            let mut prev1: usize = 0;
            let mut prev2: usize = 0;
            for _ in 0..block_len {
                let ctx = prev2 * MTF_CTX_GROUPS + prev1;
                let b = decode_literal_byte(&mut dec, &mut lit_model, ctx);
                mtf_data.push(b);
                prev2 = prev1;
                prev1 = mtf_ctx(b);
            }
            
            let bwt_data = mtf_decode(&mtf_data);
            let original = bwt_inverse(&bwt_data, orig_idx);
            output.extend_from_slice(&original);
        } else if block_mode == 3 {
            // Mode D: zero/nonzero split coding
            if pos + 8 > input.len() { break; }
            let block_len = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            let enc_size = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            if pos + enc_size > input.len() { break; }
            
            let ctx_groups = 8usize;
            let mut zero_model = BitModel::new(ctx_groups);
            let mut run_model = BitModel::new(2 * 32);
            let mut val_model = BitModel::new(ctx_groups * LIT_TREE_SIZE);
            let mut dec = RcDecoder::new(&input[pos..pos+enc_size]); pos += enc_size;
            
            let mut mtf_data = Vec::with_capacity(block_len);
            let mut prev_ctx: usize = 0;
            while mtf_data.len() < block_len {
                let is_zero = rc_decode_bit(&mut dec, &mut zero_model, prev_ctx);
                if is_zero == 1 {
                    // Decode run length (Elias gamma)
                    let mut bits_needed = 1u32;
                    loop {
                        let cont = rc_decode_bit(&mut dec, &mut run_model, (bits_needed - 1).min(31) as usize);
                        if cont == 0 { break; }
                        bits_needed += 1;
                    }
                    let mut run = 1usize << (bits_needed - 1);
                    for b in (0..bits_needed - 1).rev() {
                        let bit = dec.decode_raw(1) as usize;
                        run |= bit << b;
                    }
                    for _ in 0..run.min(block_len - mtf_data.len()) { mtf_data.push(0); }
                } else {
                    let val = decode_literal_byte(&mut dec, &mut val_model, prev_ctx);
                    let byte = val + 1;
                    mtf_data.push(byte);
                    prev_ctx = mtf_ctx(byte);
                }
            }
            
            let bwt_data = mtf_decode(&mtf_data);
            let original = bwt_inverse(&bwt_data, orig_idx);
            output.extend_from_slice(&original);
        } else if block_mode == 4 {
            // Mode E: order-3 bit-level range coding (512 quantized contexts)
            if pos + 8 > input.len() { break; }
            let block_len = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            let enc_size = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
            if pos + enc_size > input.len() { break; }
            
            let ctx_count = MTF_CTX_GROUPS * MTF_CTX_GROUPS * MTF_CTX_GROUPS;
            let mut lit_model = BitModel::new(ctx_count * LIT_TREE_SIZE);
            let mut dec = RcDecoder::new(&input[pos..pos+enc_size]); pos += enc_size;
            
            let mut mtf_data = Vec::with_capacity(block_len);
            let mut prev1: usize = 0;
            let mut prev2: usize = 0;
            let mut prev3: usize = 0;
            for _ in 0..block_len {
                let ctx = (prev3 * MTF_CTX_GROUPS + prev2) * MTF_CTX_GROUPS + prev1;
                let b = decode_literal_byte(&mut dec, &mut lit_model, ctx);
                mtf_data.push(b);
                prev3 = prev2;
                prev2 = prev1;
                prev1 = mtf_ctx(b);
            }
            
            let bwt_data = mtf_decode(&mtf_data);
            let original = bwt_inverse(&bwt_data, orig_idx);
            output.extend_from_slice(&original);
        }
    }
    output.truncate(orig_size);
    output
}


// ─── Transforms ──────────────────────────────────────────────────────────

// Transform mode for literals within a block
#[derive(Clone, Copy, PartialEq)]
enum LitTransform { None, XorDelta, Mtf }

fn transform_literal(b: u8, prev: u8, mtf_list: &mut [u8; 256], mode: LitTransform) -> u8 {
    match mode {
        LitTransform::None => b,
        LitTransform::XorDelta => b ^ prev,
        LitTransform::Mtf => {
            let pos = mtf_list.iter().position(|&x| x == b).unwrap_or(0);
            // Move to front
            for i in (1..=pos).rev() { mtf_list[i] = mtf_list[i - 1]; }
            mtf_list[0] = b;
            pos as u8
        }
    }
}

fn init_mtf() -> [u8; 256] {
    let mut list = [0u8; 256];
    for i in 0..256 { list[i] = i as u8; }
    list
}

// ─── Bit-level range coder for literals with context ─────────────────────

/// Bit-level probability model: 2048-entry table indexed by context
/// Uses LZMA-style bit tree encoding for literals.
/// Context for each bit = (prev_byte_high_bits << bit_position) | already_decoded_bits
struct BitModel {
    probs: Vec<u16>,  // probability of 0 in 11-bit fixed point (0..2048)
}

const BIT_MODEL_INIT: u16 = 1024; // 50/50 initial probability
const BIT_MODEL_MOVE: u32 = 4;    // adaptation speed (lower = faster)

impl BitModel {
    fn new(size: usize) -> Self {
        BitModel { probs: vec![BIT_MODEL_INIT; size] }
    }
}

fn rc_encode_bit(enc: &mut RcEncoder, model: &mut BitModel, idx: usize, bit: u32) {
    let prob = model.probs[idx] as u32;
    let bound = (enc.range >> 11) * prob;
    if bit == 0 {
        enc.range = bound;
        model.probs[idx] += ((2048 - prob) >> BIT_MODEL_MOVE) as u16;
    } else {
        enc.low = enc.low.wrapping_add(bound);
        enc.range -= bound;
        model.probs[idx] -= (prob >> BIT_MODEL_MOVE) as u16;
    }
    // Normalize
    while (enc.low ^ enc.low.wrapping_add(enc.range)) < RC_TOP || enc.range < RC_BOT {
        if (enc.low ^ enc.low.wrapping_add(enc.range)) >= RC_TOP {
            enc.range = enc.low.wrapping_neg() & (RC_BOT - 1);
        }
        enc.buf.push((enc.low >> 24) as u8);
        enc.low <<= 8;
        enc.range <<= 8;
    }
}

fn rc_decode_bit(dec: &mut RcDecoder, model: &mut BitModel, idx: usize) -> u32 {
    let prob = model.probs[idx] as u32;
    let bound = (dec.range >> 11) * prob;
    let bit;
    if (dec.code.wrapping_sub(dec.low)) < bound {
        bit = 0;
        dec.range = bound;
        model.probs[idx] += ((2048 - prob) >> BIT_MODEL_MOVE) as u16;
    } else {
        bit = 1;
        dec.low = dec.low.wrapping_add(bound);
        dec.range -= bound;
        model.probs[idx] -= (prob >> BIT_MODEL_MOVE) as u16;
    }
    while (dec.low ^ dec.low.wrapping_add(dec.range)) < RC_TOP || dec.range < RC_BOT {
        if (dec.low ^ dec.low.wrapping_add(dec.range)) >= RC_TOP {
            dec.range = dec.low.wrapping_neg() & (RC_BOT - 1);
        }
        dec.code = (dec.code << 8) | dec.byte() as u32;
        dec.low <<= 8;
        dec.range <<= 8;
    }
    bit
}

/// Encode a byte using bit-tree coding with previous-byte context.
/// Context structure: for each bit position (MSB first), the context includes
/// the high bits of the previous byte and the already-encoded bits of current byte.
/// Total contexts per prev-byte-group: 255 (binary tree nodes for 8 bits)
/// We use prev_byte >> 4 as the context group (16 groups) to balance adaptation speed.
const LIT_CTX_BITS: usize = 8; // full prev byte = 256 context groups
const LIT_CTX_GROUPS: usize = 1 << LIT_CTX_BITS;
const LIT_TREE_SIZE: usize = 256; // 1 + 2 + 4 + ... + 128 = 255, but we index 1..255

fn encode_literal_byte(enc: &mut RcEncoder, model: &mut BitModel, ctx_group: usize, byte: u8) {
    let base = ctx_group * LIT_TREE_SIZE;
    let mut tree_idx: usize = 1;
    for i in (0..8).rev() {
        let bit = ((byte >> i) & 1) as u32;
        rc_encode_bit(enc, model, base + tree_idx, bit);
        tree_idx = (tree_idx << 1) | bit as usize;
    }
}

fn decode_literal_byte(dec: &mut RcDecoder, model: &mut BitModel, ctx_group: usize) -> u8 {
    let base = ctx_group * LIT_TREE_SIZE;
    let mut tree_idx: usize = 1;
    for _ in 0..8 {
        let bit = rc_decode_bit(dec, model, base + tree_idx);
        tree_idx = (tree_idx << 1) | bit as usize;
    }
    (tree_idx - 256) as u8
}

/// Encode a symbol (0..nsym-1) using a bit tree
fn encode_tree_sym(enc: &mut RcEncoder, model: &mut BitModel, base: usize, sym: usize, nbits: u32) {
    let mut idx: usize = 1;
    for i in (0..nbits).rev() {
        let bit = ((sym >> i) & 1) as u32;
        rc_encode_bit(enc, model, base + idx, bit);
        idx = (idx << 1) | bit as usize;
    }
}

fn decode_tree_sym(dec: &mut RcDecoder, model: &mut BitModel, base: usize, nbits: u32) -> usize {
    let mut idx: usize = 1;
    for _ in 0..nbits {
        let bit = rc_decode_bit(dec, model, base + idx);
        idx = (idx << 1) | bit as usize;
    }
    idx - (1 << nbits)
}

/// LZ77 stream models using bit-level range coding (LZMA-style)
struct Lz77RcModels {
    /// Normal literal model: 256 prev-byte contexts × 256 tree nodes
    lit_model: BitModel,
    /// Matched literal model: 256 prev-byte contexts × 256 tree nodes
    /// (used when literal follows a match — encodes XOR with match byte)
    match_lit_model: BitModel,
    /// Lit/match flag: 256 contexts (prev byte) × 1 probability each
    flag_model: BitModel,
    /// Length code: bit tree for 5 bits (32 entries, covers 0-28 = codes 257-285)
    len_model: BitModel,
    /// Distance code: bit tree for 6 bits (64 entries, covers 0-39)
    dist_model: BitModel,
}

impl Lz77RcModels {
    fn new() -> Self {
        Lz77RcModels {
            lit_model: BitModel::new(LIT_CTX_GROUPS * LIT_TREE_SIZE),
            match_lit_model: BitModel::new(LIT_CTX_GROUPS * LIT_TREE_SIZE),
            flag_model: BitModel::new(256),
            len_model: BitModel::new(64),   // bit tree for 5 bits
            dist_model: BitModel::new(128), // bit tree for 6 bits
        }
    }
}

// ─── Compress ────────────────────────────────────────────────────────────

fn lz77_compress(input: &[u8]) -> Vec<u8> {
    let tokens = lz77_tokenize(input);
    
    let mut out = Vec::new();
    out.extend_from_slice(&(input.len() as u32).to_le_bytes());
    
    let mut models = Lz77RcModels::new();
    let mut enc = RcEncoder::new();
    let mut prev_byte: u8 = 0;
    let mut pos: usize = 0;
    let mut last_offset: usize = 1; // last match offset for matched-literal context
    let mut after_match = false;    // is this literal right after a match?
    
    for t in &tokens {
        match t {
            Token::Literal(b) => {
                rc_encode_bit(&mut enc, &mut models.flag_model, prev_byte as usize, 0);
                if after_match && pos >= last_offset {
                    // LZMA-style: encode literal using matched byte from last offset as context
                    let match_byte = input[pos - last_offset];
                    // Encode the XOR delta through the matched model
                    let delta = *b ^ match_byte;
                    encode_literal_byte(&mut enc, &mut models.match_lit_model, prev_byte as usize, delta);
                } else {
                    encode_literal_byte(&mut enc, &mut models.lit_model, prev_byte as usize, *b);
                }
                prev_byte = *b;
                pos += 1;
                after_match = false;
            }
            Token::Match { length, offset } => {
                rc_encode_bit(&mut enc, &mut models.flag_model, prev_byte as usize, 1);
                
                let (lcode, leb, lev) = length_to_code(*length);
                encode_tree_sym(&mut enc, &mut models.len_model, 0, (lcode - 257) as usize, 5);
                if leb > 0 { enc.encode_raw(lev as u32, leb as u32); }
                
                let (dcode, deb, dev) = offset_to_code(*offset);
                encode_tree_sym(&mut enc, &mut models.dist_model, 0, dcode as usize, 6);
                if deb > 0 { enc.encode_raw(dev, deb as u32); }
                
                prev_byte = input[pos + length - 1];
                pos += length;
                last_offset = *offset;
                after_match = true;
            }
        }
    }
    
    let encoded = enc.finish();
    out.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    out.extend_from_slice(&encoded);
    out
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    
    // Try LZ77 pipeline
    let lz77_result = lz77_compress(input);
    
    // Try BWT pipeline (only for inputs that aren't too large — BWT is O(n log²n))
    let bwt_result = if input.len() <= 2_000_000 {
        bwt_compress(input)
    } else {
        Vec::new() // skip for very large inputs
    };
    
    // Pick the smaller result, prefixed with a mode byte
    let use_bwt = !bwt_result.is_empty() && bwt_result.len() < lz77_result.len();
    
    let mut out = Vec::new();
    if use_bwt {
        out.push(1u8); // BWT mode
        out.extend_from_slice(&bwt_result);
    } else {
        out.push(0u8); // LZ77 mode
        out.extend_from_slice(&lz77_result);
    }
    out
}

// ─── Decompress ──────────────────────────────────────────────────────────

fn lz77_decompress(input: &[u8]) -> Vec<u8> {
    if input.len() < 8 { return Vec::new(); }
    let orig_size = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let enc_size = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    if input.len() < 8 + enc_size { return Vec::new(); }
    
    let mut models = Lz77RcModels::new();
    let mut dec = RcDecoder::new(&input[8..8 + enc_size]);
    let mut output = Vec::with_capacity(orig_size);
    let mut prev_byte: u8 = 0;
    let mut last_offset: usize = 1;
    let mut after_match = false;
    
    while output.len() < orig_size {
        let flag = rc_decode_bit(&mut dec, &mut models.flag_model, prev_byte as usize);
        
        if flag == 0 {
            // Literal
            let cur_pos = output.len();
            if after_match && cur_pos >= last_offset {
                let match_byte = output[cur_pos - last_offset];
                let delta = decode_literal_byte(&mut dec, &mut models.match_lit_model, prev_byte as usize);
                let b = delta ^ match_byte;
                output.push(b);
                prev_byte = b;
            } else {
                let b = decode_literal_byte(&mut dec, &mut models.lit_model, prev_byte as usize);
                output.push(b);
                prev_byte = b;
            }
            after_match = false;
        } else {
            // Match
            let len_idx = decode_tree_sym(&mut dec, &mut models.len_model, 0, 5);
            let lcode = (len_idx + 257) as u16;
            let (bl, eb) = code_to_length_base(lcode);
            let extra = if eb > 0 { dec.decode_raw(eb as u32) as usize } else { 0 };
            let length = bl + extra;
            
            let dsym = decode_tree_sym(&mut dec, &mut models.dist_model, 0, 6);
            let (bd, deb) = code_to_offset_base(dsym as u8);
            let dextra = if deb > 0 { dec.decode_raw(deb as u32) as usize } else { 0 };
            let offset = bd + dextra;
            
            if offset == 0 || offset > output.len() { break; }
            let start = output.len() - offset;
            for j in 0..length {
                let b = output[start + j];
                output.push(b);
            }
            prev_byte = output[output.len() - 1];
            last_offset = offset;
            after_match = true;
        }
    }
    output.truncate(orig_size);
    output
}

pub fn decompress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let mode = input[0];
    let data = &input[1..];
    match mode {
        1 => bwt_decompress(data),
        _ => lz77_decompress(data),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let d = args.iter().any(|a| a == "--decompress" || a == "-d");
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).expect("Failed to read stdin");
    let output = if d { decompress(&input) } else { compress(&input) };
    io::stdout().write_all(&output).expect("Failed to write stdout");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_roundtrip_empty() { assert_eq!(decompress(&compress(b"")), b""); }
    #[test]
    fn test_bwt_roundtrip_basic() {
        let data = b"banana";
        let (bwt, idx) = bwt_forward(data);
        let orig = bwt_inverse(&bwt, idx);
        assert_eq!(orig, data);
    }
    #[test]
    fn test_mtf_roundtrip() {
        let data = b"Call me Ishmael. Some years ago.";
        let encoded = mtf_encode(data);
        let decoded = mtf_decode(&encoded);
        assert_eq!(decoded, data);
    }
    #[test]
    fn test_bwt_mtf_roundtrip() {
        let data = b"Call me Ishmael. Some years ago.";
        let (bwt, idx) = bwt_forward(data);
        let mtf = mtf_encode(&bwt);
        let mtf_dec = mtf_decode(&mtf);
        assert_eq!(mtf_dec, bwt);
        let orig = bwt_inverse(&mtf_dec, idx);
        assert_eq!(orig, data);
    }
    #[test]
    fn test_bwt_compress_roundtrip() {
        let data = b"Call me Ishmael. Some years ago.";
        let compressed = bwt_compress(data);
        let decompressed = bwt_decompress(&compressed);
        assert_eq!(decompressed, data);
    }
    #[test] fn test_roundtrip_simple() { let i = b"aaabbbcccc"; assert_eq!(decompress(&compress(i)), i); }
    #[test] fn test_roundtrip_no_runs() { let i = b"abcdefg"; assert_eq!(decompress(&compress(i)), i); }
    #[test] fn test_roundtrip_single() { let i = b"a"; assert_eq!(decompress(&compress(i)), i); }
    #[test] fn test_roundtrip_binary() { let i: Vec<u8> = (0..=255).collect(); assert_eq!(decompress(&compress(&i)), i); }
    #[test] fn test_roundtrip_long_run() { let i = vec![0x42; 1000]; assert_eq!(decompress(&compress(&i)), i); }
    #[test] fn test_roundtrip_very_long() { let i = vec![0x42; 100000]; assert_eq!(decompress(&compress(&i)), i); }
    
    #[test]
    fn test_roundtrip_random() {
        let mut i = Vec::with_capacity(10000);
        let mut s: u32 = 12345;
        for _ in 0..10000 { s = s.wrapping_mul(1103515245).wrapping_add(12345); i.push((s >> 16) as u8); }
        assert_eq!(decompress(&compress(&i)), i);
    }
    
    #[test]
    fn test_roundtrip_pattern() {
        let p = b"hello world! ";
        let i: Vec<u8> = p.iter().cycle().take(5000).copied().collect();
        assert_eq!(decompress(&compress(&i)), i);
    }
    
    #[test] fn test_compression() { let i = vec![0x42; 1000]; assert!(compress(&i).len() < i.len()); }
    
    #[test]
    fn test_roundtrip_moby() {
        let i = b"Call me Ishmael. Some years ago--never mind how long precisely--having little or no money in my purse, and nothing particular to interest me on shore, I thought I would sail about a little and see the watery part of the world.";
        assert_eq!(decompress(&compress(i.as_slice())), i.as_slice());
    }

    #[test]
    fn test_roundtrip_large_text() {
        let mut input = Vec::new();
        let words: Vec<&[u8]> = vec![b"the ", b"quick ", b"brown ", b"fox ", b"jumps ",
            b"over ", b"lazy ", b"dog ", b"and ", b"in ", b"to ", b"of ", b"is ",
            b"it ", b"for ", b"on ", b"was ", b"with ", b"that ", b"at "];
        let mut s: u32 = 42;
        while input.len() < 1_300_000 {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            input.extend_from_slice(words[(s >> 16) as usize % words.len()]);
        }
        let c = compress(&input);
        let d = decompress(&c);
        assert_eq!(d.len(), input.len());
        assert_eq!(d, input);
    }
}

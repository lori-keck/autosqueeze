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

    // DP backward pass
    let mut cost = vec![u32::MAX / 2; n + 1];
    let mut choice: Vec<(usize, usize)> = vec![(1, 0); n + 1];
    cost[n] = 0;

    for pos in (0..n).rev() {
        let c = 9u32 + cost[pos + 1];
        if c < cost[pos] { cost[pos] = c; choice[pos] = (1, 0); }
        for &(len, off) in &all_matches[pos] {
            let (_, leb, _) = length_to_code(len);
            let (_, deb, _) = offset_to_code(off);
            let mc = 7u32 + leb as u32 + 5 + deb as u32 + cost[pos + len];
            if mc < cost[pos] { cost[pos] = mc; choice[pos] = (len, off); }
            // Try some shorter lengths
            for &sl in &[MIN_MATCH, 4, 5, 6, 8, 12, 16, 24, 32, len/2] {
                if sl >= MIN_MATCH && sl < len {
                    let (_, leb, _) = length_to_code(sl);
                    let mc = 7u32 + leb as u32 + 5 + deb as u32 + cost[pos + sl];
                    if mc < cost[pos] { cost[pos] = mc; choice[pos] = (sl, off); }
                }
            }
        }
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

/// BWT pipeline: BWT → MTF → RLE → adaptive range coding (no headers needed)
fn bwt_compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let bwt_block_size = 900_000usize;
    let mut out = Vec::new();
    out.extend_from_slice(&(input.len() as u32).to_le_bytes());
    let num_blocks = (input.len() + bwt_block_size - 1) / bwt_block_size;
    out.extend_from_slice(&(num_blocks as u32).to_le_bytes());
    
    for chunk in input.chunks(bwt_block_size) {
        let (bwt_data, orig_idx) = bwt_forward(chunk);
        let mtf_data = mtf_encode(&bwt_data);
        let rle_data = rle_zero_encode(&mtf_data);
        
        out.extend_from_slice(&orig_idx.to_le_bytes());
        
        // Adaptive range coding: 258 symbols (0-256 values + EOB=257)
        let mut model = RcModel::new(258);
        let mut enc = RcEncoder::new();
        for &s in &rle_data { enc.encode(&mut model, s as usize); }
        enc.encode(&mut model, 257); // EOB
        let encoded = enc.finish();
        out.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        out.extend_from_slice(&encoded);
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
        if pos + 4 > input.len() { break; }
        let orig_idx = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]); pos += 4;
        
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

// ─── Block encoding ──────────────────────────────────────────────────────

fn estimate_block_size_mode(tokens: &[Token], mode: LitTransform) -> usize {
    let mut ll_freq = vec![0u32; 286];
    let mut d_freq = vec![0u32; 40];
    let mut prev_lit = 0u8;
    let mut mtf = init_mtf();
    for t in tokens {
        match t {
            Token::Literal(b) => {
                let val = transform_literal(*b, prev_lit, &mut mtf, mode);
                ll_freq[val as usize] += 1; prev_lit = *b;
            }
            Token::Match { length, offset } => {
                let (c, _, _) = length_to_code(*length); ll_freq[c as usize] += 1;
                let (dc, _, _) = offset_to_code(*offset); d_freq[dc as usize] += 1;
            }
        }
    }
    ll_freq[256] += 1;
    let ll_lens = build_code_lengths(&ll_freq, 15);
    let d_lens = build_code_lengths(&d_freq, 15);
    let mut bits = 0usize;
    prev_lit = 0;
    let mut mtf2 = init_mtf();
    for t in tokens {
        match t {
            Token::Literal(b) => {
                let val = transform_literal(*b, prev_lit, &mut mtf2, mode);
                bits += ll_lens[val as usize] as usize; prev_lit = *b;
            }
            Token::Match { length, offset } => {
                let (lc, leb, _) = length_to_code(*length);
                bits += ll_lens[lc as usize] as usize + leb as usize;
                let (dc, deb, _) = offset_to_code(*offset);
                bits += d_lens[dc as usize] as usize + deb as usize;
            }
        }
    }
    bits += ll_lens[256] as usize;
    (bits + 7) / 8
}

fn encode_block(tokens: &[Token], out: &mut Vec<u8>) {
    // Try all 3 modes, pick the smallest
    let sizes = [
        estimate_block_size_mode(tokens, LitTransform::None),
        estimate_block_size_mode(tokens, LitTransform::XorDelta),
        estimate_block_size_mode(tokens, LitTransform::Mtf),
    ];
    let best_mode_idx = sizes.iter().enumerate().min_by_key(|&(_, &s)| s).unwrap().0;
    let mode = [LitTransform::None, LitTransform::XorDelta, LitTransform::Mtf][best_mode_idx];
    out.push(best_mode_idx as u8);

    let mut ll_freq = vec![0u32; 286];
    let mut d_freq = vec![0u32; 40];
    let mut prev_lit = 0u8;
    let mut mtf = init_mtf();
    for t in tokens {
        match t {
            Token::Literal(b) => {
                let val = transform_literal(*b, prev_lit, &mut mtf, mode);
                ll_freq[val as usize] += 1; prev_lit = *b;
            }
            Token::Match { length, offset } => {
                let (c, _, _) = length_to_code(*length); ll_freq[c as usize] += 1;
                let (dc, _, _) = offset_to_code(*offset); d_freq[dc as usize] += 1;
            }
        }
    }
    ll_freq[256] += 1;

    let ll_lens = build_code_lengths(&ll_freq, 15);
    let d_lens = build_code_lengths(&d_freq, 15);
    let ll_codes = canonical_codes(&ll_lens);
    let d_codes = canonical_codes(&d_lens);

    let ll_count = ll_lens.iter().rposition(|&l| l > 0).map(|p| p + 1).unwrap_or(0);
    out.extend_from_slice(&(ll_count as u16).to_le_bytes());
    // Nibble-pack code lengths
    for ii in (0..ll_count).step_by(2) {
        let lo = ll_lens[ii] & 0x0F;
        let hi = if ii + 1 < ll_count { ll_lens[ii + 1] & 0x0F } else { 0 };
        out.push(lo | (hi << 4));
    }
    let d_count = d_lens.iter().rposition(|&l| l > 0).map(|p| p + 1).unwrap_or(0);
    out.push(d_count as u8);
    for ii in (0..d_count).step_by(2) {
        let lo = d_lens[ii] & 0x0F;
        let hi = if ii + 1 < d_count { d_lens[ii + 1] & 0x0F } else { 0 };
        out.push(lo | (hi << 4));
    }

    let mut bw = BitWriter::new();
    let mut prev_lit2 = 0u8;
    let mut mtf2 = init_mtf();
    for t in tokens {
        match t {
            Token::Literal(b) => {
                let val = transform_literal(*b, prev_lit2, &mut mtf2, mode);
                prev_lit2 = *b;
                let (code, bits) = ll_codes[val as usize];
                bw.write_bits(rev_bits(code, bits), bits as u32);
            }
            Token::Match { length, offset } => {
                let (lc, leb, lev) = length_to_code(*length);
                let (code, bits) = ll_codes[lc as usize];
                bw.write_bits(rev_bits(code, bits), bits as u32);
                if leb > 0 { bw.write_bits(lev as u32, leb as u32); }
                let (dc, deb, dev) = offset_to_code(*offset);
                let (dcode, dbits) = d_codes[dc as usize];
                bw.write_bits(rev_bits(dcode, dbits), dbits as u32);
                if deb > 0 { bw.write_bits(dev as u32, deb as u32); }
            }
        }
    }
    let (eob_code, eob_bits) = ll_codes[256];
    bw.write_bits(rev_bits(eob_code, eob_bits), eob_bits as u32);
    let bits = bw.flush();
    out.extend_from_slice(&(bits.len() as u32).to_le_bytes());
    out.extend_from_slice(&bits);
}

// ─── Compress ────────────────────────────────────────────────────────────

fn lz77_compress_with_block_size(tokens: &[Token], orig_size: usize, block_size: usize) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(orig_size as u32).to_le_bytes());
    let num_blocks = (tokens.len() + block_size - 1) / block_size;
    out.extend_from_slice(&(num_blocks as u32).to_le_bytes());
    for chunk in tokens.chunks(block_size) { encode_block(chunk, &mut out); }
    out
}

fn lz77_compress(input: &[u8]) -> Vec<u8> {
    let tokens = lz77_tokenize(input);
    // Try multiple block sizes and pick the best
    let block_sizes = [8192, 16384, 32768, 65536];
    let mut best = lz77_compress_with_block_size(&tokens, input.len(), BLOCK_SIZE);
    for &bs in &block_sizes {
        if bs == BLOCK_SIZE { continue; }
        let result = lz77_compress_with_block_size(&tokens, input.len(), bs);
        if result.len() < best.len() { best = result; }
    }
    best
}

// ─── Context Mixing Compressor (PAQ-style) ──────────────────────────────

fn cm_stretch() -> [i16; 4097] {
    let mut t = [0i16; 4097];
    for i in 1..4096 {
        let p = i as f64 / 4096.0;
        t[i] = ((p / (1.0 - p)).ln() * 64.0).round().max(-2047.0).min(2047.0) as i16;
    }
    t[0] = -2047; t[4096] = 2047; t
}

fn cm_squash() -> [u16; 4096] {
    let mut t = [0u16; 4096];
    for i in 0..4096 {
        let s = (i as f64 - 2048.0) / 64.0;
        let p = 1.0 / (1.0 + (-s).exp());
        t[i] = (p * 4096.0).round().max(1.0).min(4095.0) as u16;
    }
    t
}

#[inline] fn cmh(a: usize, b: usize) -> usize { a.wrapping_mul(0x9E3779B9).wrapping_add(b) }
#[inline] fn cm_p(c: &[u16; 2]) -> u32 { let c0 = c[0] as u32 + 1; let c1 = c[1] as u32 + 1; c0 * 4096 / (c0 + c1) }
#[inline] fn cm_upd(c: &mut [u16; 2], bit: u8) {
    c[bit as usize] += 1;
    if c[0] as u32 + c[1] as u32 > 4096 { c[0] = (c[0] >> 1) + 1; c[1] = (c[1] >> 1) + 1; }
}

struct CMRangeEnc { low: u64, range: u32, cache: u8, csize: u32, out: Vec<u8> }
impl CMRangeEnc {
    fn new() -> Self { CMRangeEnc { low: 0, range: 0xFFFFFFFF, cache: 0, csize: 1, out: Vec::new() } }
    fn shift(&mut self) {
        let h = (self.low >> 32) as u8;
        if (self.low as u32) < 0xFF000000 || h != 0 {
            self.out.push(self.cache.wrapping_add(h));
            for _ in 1..self.csize { self.out.push(0xFF_u8.wrapping_add(h)); }
            self.cache = (self.low >> 24) as u8; self.csize = 1;
        } else { self.csize += 1; }
        self.low = ((self.low as u32) << 8) as u64;
    }
    fn enc(&mut self, p0: u32, bit: u8) {
        let b = (self.range >> 12) * p0;
        if bit == 0 { self.range = b; } else { self.low += b as u64; self.range -= b; }
        while self.range < (1 << 24) { self.range <<= 8; self.shift(); }
    }
    fn flush(&mut self) { for _ in 0..5 { self.shift(); } }
}

struct CMRangeDec<'a> { range: u32, code: u32, data: &'a [u8], pos: usize }
impl<'a> CMRangeDec<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut d = CMRangeDec { range: 0xFFFFFFFF, code: 0, data, pos: 0 };
        for _ in 0..5 { d.code = (d.code << 8) | d.rb() as u32; } d
    }
    fn rb(&mut self) -> u8 { if self.pos < self.data.len() { let b = self.data[self.pos]; self.pos += 1; b } else { 0 } }
    fn dec(&mut self, p0: u32) -> u8 {
        let b = (self.range >> 12) * p0;
        let bit; if self.code < b { bit = 0; self.range = b; } else { bit = 1; self.code -= b; self.range -= b; }
        while self.range < (1 << 24) { self.code = (self.code << 8) | self.rb() as u32; self.range <<= 8; }
        bit
    }
}

const CMH_BITS: usize = 23;
const CMH_SIZE: usize = 1 << CMH_BITS;
const CMH_MASK: usize = CMH_SIZE - 1;
const CM_NM: usize = 10;

struct CtxMix {
    o: [Vec<[u16; 2]>; 10],
    wt: [i32; CM_NM + 1],
    prev: [u8; 10],
    wctx: usize,
    mt: Vec<u32>, mb: Vec<u8>, ma: bool, mby: u8,
    sse: Vec<[u16; 2]>,
    stretch: Box<[i16; 4097]>,
    squash: Box<[u16; 4096]>,
}

impl CtxMix {
    fn new() -> Self {
        CtxMix {
            o: [
                vec![[0u16; 2]; 512],     // o0
                vec![[0u16; 2]; 65536],   // o1
                vec![[0u16; 2]; CMH_SIZE], // o2
                vec![[0u16; 2]; CMH_SIZE], // o3
                vec![[0u16; 2]; CMH_SIZE], // o4
                vec![[0u16; 2]; CMH_SIZE], // o5
                vec![[0u16; 2]; CMH_SIZE], // o6
                vec![[0u16; 2]; CMH_SIZE], // word
                vec![[0u16; 2]; CMH_SIZE], // o8
                vec![[0u16; 2]; CMH_SIZE], // sparse
            ],
            wt: [256, 512, 1024, 1536, 1536, 1024, 512, 768, 768, 512, 1024],
            prev: [0; 10], wctx: 0,
            mt: vec![0u32; 1 << 22], mb: Vec::new(), ma: false, mby: 0,
            sse: vec![[0u16; 2]; 64 * 256],
            stretch: Box::new(cm_stretch()), squash: Box::new(cm_squash()),
        }
    }

    fn begin(&mut self) {
        let cp = self.mb.len(); self.ma = false;
        if cp >= 8 {
            let mut h = 0x12345usize;
            for i in 0..8 { h = cmh(h, self.prev[i] as usize); }
            let idx = h & (self.mt.len() - 1);
            let pp = self.mt[idx] as usize;
            if pp > 0 && pp < cp && pp >= 8 &&
               (0..8).all(|i| self.mb[pp-8+i] == self.mb[cp-8+i]) && pp < self.mb.len() {
                self.ma = true; self.mby = self.mb[pp];
            }
            self.mt[idx] = cp as u32;
        }
    }

    fn predict(&self, partial: usize) -> (u32, [i16; CM_NM+1], [usize; 10], usize) {
        let p = &self.prev;
        let (p0,p1,p2,p3,p4,p5,p6,p7) = (p[0] as usize, p[1] as usize, p[2] as usize,
            p[3] as usize, p[4] as usize, p[5] as usize, p[6] as usize, p[7] as usize);
        let c = [
            partial,
            p0*256 + partial,
            cmh(cmh(p1, p0), partial) & CMH_MASK,
            cmh(cmh(cmh(p2, p1), p0), partial) & CMH_MASK,
            cmh(cmh(cmh(cmh(p3, p2), p1), p0), partial) & CMH_MASK,
            cmh(cmh(cmh(cmh(cmh(p4, p3), p2), p1), p0), partial) & CMH_MASK,
            cmh(cmh(cmh(cmh(cmh(cmh(p5, p4), p3), p2), p1), p0)^partial, 0xABCD) & CMH_MASK,
            cmh(self.wctx, partial) & CMH_MASK,
            cmh(cmh(cmh(cmh(cmh(cmh(cmh(cmh(p7,p6),p5),p4),p3),p2),p1),p0)^partial, 0x1234) & CMH_MASK,
            cmh(cmh(p2, p0), partial) & CMH_MASK,
        ];
        let mut st = [0i16; CM_NM+1];
        let mut sum: i64 = 0;
        for i in 0..10 {
            let pr = cm_p(&self.o[i][c[i]]);
            st[i] = self.stretch[pr.min(4096) as usize];
            sum += self.wt[i] as i64 * st[i] as i64;
        }
        if self.ma {
            let bp = match partial { 1=>7, 2..=3=>6, 4..=7=>5, 8..=15=>4, 16..=31=>3, 32..=63=>2, 64..=127=>1, _=>0 };
            let pb = (self.mby >> bp) & 1;
            let mp = if pb == 0 { 3900u32 } else { 196 };
            st[10] = self.stretch[mp.min(4096) as usize];
            sum += self.wt[10] as i64 * st[10] as i64;
        }
        let wt: i64 = self.wt.iter().map(|&w| w.abs() as i64).sum::<i64>().max(1);
        let sm = (sum / wt) as i32;
        let idx = (sm + 2048).max(0).min(4095) as usize;
        let pmr = (self.squash[idx] as u32).max(1).min(4095);
        let pq = (pmr >> 6) as usize;
        let si = (p0 & 0xFF) * 64 + pq;
        let sp = cm_p(&self.sse[si]);
        let pq = (pmr >> 6) as usize;
        let si = (p[0] as usize & 0xFF) * 64 + pq;
        let sp = cm_p(&self.sse[si]);
        // Light SSE: 70% mixer, 30% SSE
        let pf = ((pmr * 70 + sp * 30) / 100).max(1).min(4095);
        (pf, st, c, si)
    }

    fn update(&mut self, bit: u8, _pmix: u32, st: &[i16; CM_NM+1], c: &[usize; 10], si: usize) {
        let err = if bit == 1 { _pmix as i32 } else { _pmix as i32 - 4096 };
        for i in 0..CM_NM+1 {
            self.wt[i] -= ((18i64 * err as i64 * st[i] as i64) >> 16) as i32;
            self.wt[i] = self.wt[i].max(1).min(65536);
        }
        for i in 0..10 { cm_upd(&mut self.o[i][c[i]], bit); }
        cm_upd(&mut self.sse[si], bit);
    }

    fn end(&mut self, byte: u8) {
        self.mb.push(byte);
        if byte.is_ascii_alphabetic() || byte == b'\'' {
            self.wctx = cmh(self.wctx, byte.to_ascii_lowercase() as usize);
        } else { self.wctx = 0; }
        for i in (1..10).rev() { self.prev[i] = self.prev[i-1]; }
        self.prev[0] = byte;
    }
}

fn cm_compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let mut m = CtxMix::new();
    let mut enc = CMRangeEnc::new();
    for &byte in input {
        m.begin();
        let mut partial: usize = 1;
        for bi in (0..8).rev() {
            let bit = ((byte >> bi) & 1) as u8;
            let (pr, st, c, si) = m.predict(partial);
            enc.enc(pr, bit);
            m.update(bit, pr, &st, &c, si);
            partial = partial * 2 + bit as usize;
        }
        m.end(byte);
    }
    enc.flush();
    let mut out = Vec::new();
    out.extend_from_slice(&(input.len() as u32).to_le_bytes());
    out.extend_from_slice(&(enc.out.len() as u32).to_le_bytes());
    out.extend_from_slice(&enc.out);
    out
}

fn cm_decompress(input: &[u8]) -> Vec<u8> {
    if input.len() < 8 { return Vec::new(); }
    let orig = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let esz = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    if input.len() < 8 + esz { return Vec::new(); }
    let mut m = CtxMix::new();
    let mut dec = CMRangeDec::new(&input[8..8+esz]);
    let mut output = Vec::with_capacity(orig);
    for _ in 0..orig {
        m.begin();
        let mut partial: usize = 1;
        let mut byte: u8 = 0;
        for bi in (0..8).rev() {
            let (pr, st, c, si) = m.predict(partial);
            let bit = dec.dec(pr);
            m.update(bit, pr, &st, &c, si);
            if bit == 1 { byte |= 1 << bi; }
            partial = partial * 2 + bit as usize;
        }
        output.push(byte);
        m.end(byte);
    }
    output
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let lz77_result = lz77_compress(input);
    let bwt_result = if input.len() <= 2_000_000 { bwt_compress(input) } else { Vec::new() };
    let cm_result = cm_compress(input);
    let mut best_mode = 0u8;
    let mut best = &lz77_result;
    if !bwt_result.is_empty() && bwt_result.len() < best.len() { best_mode = 1; best = &bwt_result; }
    if cm_result.len() < best.len() { best_mode = 4; best = &cm_result; }
    let mut out = Vec::with_capacity(1 + best.len());
    out.push(best_mode);
    out.extend_from_slice(best);
    out
}


// ─── Decompress ──────────────────────────────────────────────────────────

fn lz77_decompress(input: &[u8]) -> Vec<u8> {
    if input.len() < 8 { return Vec::new(); }
    let orig_size = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let num_blocks = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    let mut pos = 8;
    let mut output = Vec::with_capacity(orig_size);

    for _ in 0..num_blocks {
        if output.len() >= orig_size || pos >= input.len() { break; }
        let block_mode = input[pos]; pos += 1;
        // 0 = none, 1 = xor-delta, 2 = MTF

        if pos + 2 > input.len() { break; }
        let ll_count = u16::from_le_bytes([input[pos], input[pos + 1]]) as usize; pos += 2;
        if pos + (ll_count + 1) / 2 > input.len() { break; }
        let mut ll_lens = vec![0u8; 286];
        let ll_pk = (ll_count + 1) / 2;
        for ii in 0..ll_pk {
            if pos + ii >= input.len() { break; }
            let byte = input[pos + ii];
            let idx0 = ii * 2;
            if idx0 < ll_count { ll_lens[idx0] = byte & 0x0F; }
            if idx0 + 1 < ll_count { ll_lens[idx0 + 1] = byte >> 4; }
        }
        pos += ll_pk;

        if pos >= input.len() { break; }
        let d_count = input[pos] as usize; pos += 1;
        if pos + (d_count + 1) / 2 > input.len() { break; }
        let mut d_lens = vec![0u8; 40];
        let d_pk = (d_count + 1) / 2;
        for ii in 0..d_pk {
            if pos + ii >= input.len() { break; }
            let byte = input[pos + ii];
            let idx0 = ii * 2;
            if idx0 < d_count { d_lens[idx0] = byte & 0x0F; }
            if idx0 + 1 < d_count { d_lens[idx0 + 1] = byte >> 4; }
        }
        pos += d_pk;

        if pos + 4 > input.len() { break; }
        let bds = u32::from_le_bytes([input[pos], input[pos+1], input[pos+2], input[pos+3]]) as usize; pos += 4;
        if pos + bds > input.len() { break; }

        let (ll_sym, ll_len, ll_max) = build_decode_table(&ll_lens);
        let (d_sym, d_len, d_max) = build_decode_table(&d_lens);
        let mut reader = BitReader::new(&input[pos..pos + bds]); pos += bds;
        let mut prev_lit_dec = 0u8;
        let mut mtf_dec = init_mtf();

        loop {
            if output.len() >= orig_size { break; }
            let sym = decode_sym(&mut reader, &ll_sym, &ll_len, ll_max);
            if sym == 256 { break; }
            if sym < 256 {
                let byte = match block_mode {
                    1 => (sym as u8) ^ prev_lit_dec,
                    2 => {
                        // Inverse MTF: position → original byte
                        let pos_in_list = sym as usize;
                        let b = mtf_dec[pos_in_list];
                        for i in (1..=pos_in_list).rev() { mtf_dec[i] = mtf_dec[i - 1]; }
                        mtf_dec[0] = b;
                        b
                    }
                    _ => sym as u8,
                };
                prev_lit_dec = byte; output.push(byte);
            } else {
                let (bl, eb) = code_to_length_base(sym);
                let extra = if eb > 0 { reader.read_bits(eb as u32) as usize } else { 0 };
                let length = bl + extra;
                let dsym = decode_sym(&mut reader, &d_sym, &d_len, d_max);
                let (bd, deb) = code_to_offset_base(dsym as u8);
                let dextra = if deb > 0 { reader.read_bits(deb as u32) as usize } else { 0 };
                let offset = bd + dextra;
                if offset == 0 || offset > output.len() { break; }
                let start = output.len() - offset;
                for j in 0..length { let b = output[start + j]; output.push(b); }
            }
        }
    }
    output
}

pub fn decompress(input: &[u8]) -> Vec<u8> {
    if input.is_empty() { return Vec::new(); }
    let mode = input[0];
    let data = &input[1..];
    match mode {
        1 => bwt_decompress(data),
        4 => cm_decompress(data),
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

# Autosqueeze Inefficiency Audit

**Date:** 2026-03-15  
**Current Score:** ratio=0.2589, compress=0.1 MB/s, decompress=11.0 MB/s  
**Corpus:** 2.6 MB total (7 files, text-heavy)

---

## 1. Bit Budget Analysis

### Overall Output Structure

```
[1 byte]  Mode selector (0=LZ77, 1=BWT)
[4 bytes] Original size (u32 LE)
[4 bytes] Number of blocks (u32 LE)
Per block:
  [1 byte]   Literal transform mode (0/1/2)
  [2 bytes]  Lit/Length code count (u16 LE)
  [~143 B]   Lit/Length code lengths (nibble-packed, up to 286 symbols)
  [1 byte]   Distance code count
  [~20 B]    Distance code lengths (nibble-packed, up to 40 symbols)
  [4 bytes]  Bitstream size (u32 LE)
  [variable] Huffman-coded bitstream (literals, matches, EOB)
```

### Per-File Bit Budget Breakdown (LZ77 path)

| Component | Approx. Cost | Notes |
|-----------|-------------|-------|
| Mode byte | 8 bits/file | Trivial |
| File header (size + num_blocks) | 64 bits/file | Fixed |
| Block headers (transform + table counts) | ~32 bits/block | |
| Huffman code length tables | ~1300 bits/block (LL) + ~160 bits/block (dist) | Nibble-packed, no RLE |
| Bitstream size field | 32 bits/block | Redundant — could compute from block end |
| Huffman-coded literals | ~60-70% of bitstream | Dominant cost |
| Huffman-coded match lengths | ~5-10% of bitstream | |
| Extra bits (length) | ~3-5% of bitstream | Fixed cost per match |
| Huffman-coded distance codes | ~10-15% of bitstream | |
| Extra bits (distance) | ~10-15% of bitstream | Large distances are expensive |
| EOB symbols | ~5-15 bits/block | One per block |

### BWT Path Budget

| Component | Approx. Cost | Notes |
|-----------|-------------|-------|
| Mode byte | 8 bits | |
| Original size + num blocks | 64 bits | |
| Per block: orig_idx | 32 bits | BWT rotation index |
| Per block: encoded size | 32 bits | |
| Per block: range-coded data | Variable | Adaptive, no explicit tables |

---

## 2. Top 10 Sources of Wasted Bits (Ranked by Impact)

### #1: No Context Mixing or Higher-Order Modeling — ~15-25% ratio improvement left on table

**The Problem:** The LZ77 path uses order-0 Huffman coding for literals and length/distance codes. This throws away ALL contextual information. After an 'e' in English text, 'r', 's', 'd', ' ' are far more likely than 'z' or 'q'. Huffman assigns the same code to 'r' regardless of what preceded it.

The BWT path uses adaptive range coding (order-0), which is better but still order-0. BWT+MTF creates some implicit context, but the range coder's model is a flat frequency table with simple scaling.

**The Fix:** For the LZ77 path: switch literals to adaptive range coding with order-1 or order-2 context models (hash the previous 1-2 bytes, maintain per-context frequency tables). For the BWT path: the current approach is already near-optimal for BWT — the real win is in context modeling on the LZ77 side, or a PPM/CM approach entirely.

**Estimated Savings:** On text (Moby Dick, logs, source), this alone could shave 15-25% off compressed size. On the full corpus: ~40-60 KB saved.

### #2: Huffman Tables Transmitted Per Block with No Compression — ~5-12% overhead on small blocks

**The Problem:** Each block stores full Huffman code lengths nibble-packed (4 bits per symbol). For the lit/length alphabet of 286 symbols, that's 143 bytes of table data per block. With the default 32K token block size, you might have 40+ blocks for Moby Dick. That's ~5.6 KB in tables alone.

Worse: no RLE, no delta coding, no CL-CL (code-length code-length) compression like DEFLATE uses. DEFLATE uses a 3-level compression scheme for its tables; we just dump raw nibbles.

**The Fix:** Use DEFLATE-style CL-CL encoding (repeat codes 16/17/18 for runs). Or better: delta-code against a previous block's table. Or best: skip per-block tables entirely and use adaptive coding.

**Estimated Savings:** ~2-5 KB on Moby Dick, proportionally more on files with many blocks. Across corpus: ~5-10 KB.

### #3: Block Size Selection Is Brute-Force and Suboptimal — ~2-5% ratio waste

**The Problem:** The compressor tries 4 fixed block sizes (8K, 16K, 32K, 64K tokens) and picks the smallest output. This is:
- Wasteful at compress time (runs encoding 4x)
- Suboptimal because the "best" block size varies within a file (logs might want tiny blocks for the header region and bigger blocks for repetitive bodies)
- Token-count based, not byte-count based — a block of 32K tokens where most are matches covers very different data spans than 32K literal tokens

**The Fix:** Adaptive block splitting: compute the entropy of the current block incrementally and split when the coding cost would decrease. DEFLATE does this. Brotli does it better.

**Estimated Savings:** ~2-5% on heterogeneous files (logs, source code). ~5-15 KB across corpus.

### #4: DEFLATE-Style Length/Distance Alphabet Is Wasteful for 1MB Window — ~3-5% on match-heavy data

**The Problem:** The code uses DEFLATE's distance code alphabet but extended to 40 codes to cover a 1MB window. DEFLATE was designed for a 32KB window with 30 distance codes. The extension to codes 30-39 uses 14-18 extra bits per distance, which is extremely costly.

The length alphabet is also DEFLATE's, capped at 258. Modern compressors (Zstandard, LZMA) allow much longer matches. On highly repetitive data, a match of length 10,000 must be encoded as ~39 separate 258-length matches.

**The Fix:** 
- Use a variable-length distance coding scheme (like Zstd's) optimized for larger windows
- Allow longer matches (at least 65535, or unlimited with length escaping)
- Consider separate near/far distance models

**Estimated Savings:** ~3-5% on match-heavy files. Most impactful on repetitive.bin (but it's already 90 bytes, so limited absolute savings) and structured.json, logs.

### #5: DP Optimal Parser Only Iterates Twice — ~1-3% ratio waste

**The Problem:** The DP optimal parser runs exactly 2 iterations to converge cost estimates. The initial code-length estimate (8 bits per literal, 5 bits per distance code) is very rough. Two passes help but don't fully converge — especially for files with unusual distributions.

Additionally, the parser only checks a subset of match lengths (`[3,4,5,...,10,11,13,15,17,19,23,27,31,35,43,51,67,83,99,115,131]`) rather than all possibilities. This misses the optimal split point for many matches.

**The Fix:** 
- Run 3-4 DP iterations (diminishing returns after 4)
- Check ALL valid match lengths, not just the "base" lengths — the cost can differ for lengths within the same code bracket due to how extra bits interact with subsequent symbol costs

**Estimated Savings:** ~1-3% on text. ~5-10 KB across corpus.

### #6: Hash Chain Limit of 512 with Order-4 Hash — Missed Matches at Long Distances

**The Problem:** A 4-byte hash with only 64K buckets (16-bit) has massive collision rates. The chain limit of 512 means for large windows (1MB), many valid long-distance matches are never found. This particularly hurts on data with long-range repetitions (logs with repeated patterns, structured data).

**The Fix:**
- Use a larger hash table (20+ bits)
- Implement a binary search tree or suffix array for match finding instead of hash chains
- At minimum: use a 3-level hash (hash4 + hash6 + hash8) for better long-range match detection

**Estimated Savings:** ~1-3% on structured/log data. ~3-8 KB across corpus.

### #7: BWT Pipeline Wastes Bits on the 258-Symbol Range Coder Model

**The Problem:** After BWT → MTF → RLE-zero encoding, the symbol space is {0=RUNA, 1=RUNB, 2-256=shifted MTF values, 257=EOB} = 258 symbols. The range coder maintains a flat 258-symbol model. But after MTF, the distribution is EXTREMELY skewed — most symbols are 0, 1, or small MTF indices. Having 258 equally-weighted initial symbols wastes model capacity.

Also: the model rescaling (`total > RC_BOT/2`) is aggressive. With `RC_BOT = 65536` and threshold at `32768`, the model gets rescaled (halved) frequently, losing adaptation precision.

**The Fix:**
- Use a two-level model: one for "is this a zero-run code?" and one for the non-zero symbol
- Or: use a structured model that gives much more probability mass to small values initially
- Increase RC_BOT or use a less aggressive rescaling schedule
- Consider order-1 context in the range coder (previous symbol type predicts next)

**Estimated Savings:** ~2-4% on BWT-mode files. Most impactful on Moby Dick and source code.

### #8: No Preprocessing / Filtering for Specific Data Types

**The Problem:** The compressor treats all data uniformly. But:
- CSV/sensor data has strong column-wise correlations (delta coding columns would help enormously)
- JSON has structural redundancy that LZ77 partially captures but a dictionary approach would crush
- Source code has reserved word patterns that a static dictionary would handle well

The literal transforms (None, XorDelta, MTF) are a start, but XorDelta is byte-wise (not column-wise or word-wise), and the choice is per-block with no fine-grained adaptation.

**The Fix:**
- Detect data types and apply appropriate preprocessing
- For CSV: column-wise delta encoding
- Static dictionaries for known formats
- Word-level modeling for text

**Estimated Savings:** ~5-15% on structured data (JSON, CSV, logs). ~15-30 KB across corpus.

### #9: Bitstream Size Field Is Redundant — 32 bits wasted per block

**The Problem:** Each block stores a 4-byte bitstream length. This is needed for the decoder to know where the bitstream ends, but it's redundant information — the decoder could instead read until EOB and know it's done. The field exists because the bit reader needs to know its bounds.

**The Fix:** Remove the field. Have the bit reader just read from the overall stream; the EOB symbol terminates each block naturally. Alternatively, use the block structure to compute sizes.

**Estimated Savings:** 32 bits × num_blocks. For Moby Dick with ~40 blocks: 160 bytes. Across corpus: ~300-500 bytes. Minor but free.

### #10: The Dual-Path Architecture Itself Wastes a Mode Byte and Prevents Hybrid Approaches

**The Problem:** The compressor tries LZ77 and BWT independently and picks the smaller. This means:
- 1 byte wasted on mode selection per file
- No ability to use BWT for one region and LZ77 for another within the same file
- Both pipelines run fully, doubling compress time
- For files near the 2MB BWT cutoff, you may miss the better algorithm

More fundamentally: the best modern compressors don't choose between LZ and BWT — they use LZ77 followed by entropy coding with context mixing (LZMA, Zstd) or full CM (PAQ, ZPAQ). The dual-path approach is a bolted-on afterthought.

**The Fix:** Either:
- Allow per-block mode selection (LZ77 or BWT per block)
- Or: unify into a single pipeline. LZ77 → adaptive range coding with context models gets most of BWT's benefit without the O(n log² n) sort

**Estimated Savings:** ~1-2% from better mode selection. Conceptually: the architectural unification enables all the other improvements.

---

## 3. Theoretical Achievable Ratio

### Current Performance by File

| File | Size | Compressed | Ratio | Theoretical* |
|------|------|-----------|-------|-------------|
| 01_moby_dick.txt | 1,276,266 | 393,933 | 0.3015 | ~0.22-0.25 |
| 02_structured.json | 170,239 | 6,861 | 0.0403 | ~0.02-0.03 |
| 03_repetitive.bin | 100,000 | 90 | 0.0009 | ~0.0005 |
| 04_random.bin | 100,000 | 100,010 | 1.0001 | ~1.0001 |
| 05_source_code.c | 292,843 | 69,005 | 0.2357 | ~0.16-0.19 |
| 06_sensor_data.csv | 331,265 | 90,205 | 0.2724 | ~0.15-0.20 |
| 07_logs.txt | 328,826 | 22,048 | 0.0670 | ~0.04-0.05 |

*Theoretical = achievable with fixes below, within same general algorithmic family (LZ77+entropy). Not PPM/CM class.

### If ALL Identified Wastes Were Fixed

| Fix | Estimated Ratio Improvement |
|-----|-----------------------------|
| #1 Context modeling | -0.030 to -0.050 |
| #2 Table compression | -0.003 to -0.008 |
| #3 Adaptive block splitting | -0.005 to -0.012 |
| #4 Better length/distance codes | -0.003 to -0.008 |
| #5 More DP iterations + full search | -0.003 to -0.008 |
| #6 Better match finding | -0.002 to -0.006 |
| #7 Better BWT range coder model | -0.003 to -0.007 |
| #8 Data-type preprocessing | -0.010 to -0.020 |
| #9 Remove redundant fields | -0.0002 |
| #10 Unified architecture | -0.003 to -0.008 |
| **TOTAL** | **-0.062 to -0.127** |

**Current ratio: 0.2589**  
**Theoretical achievable: ~0.13 to 0.20**  
**Best realistic target (all fixes, same arch): ~0.17-0.19**  
**With CM/PPM-class rewrite: ~0.12-0.15**

For reference: gzip on this corpus typically gets ~0.30-0.33, bzip2 ~0.22-0.25, zstd ~0.25-0.28, xz/LZMA ~0.20-0.22. We're already beating gzip. The theoretical target of ~0.17-0.19 would put us between bzip2 and xz.

---

## 4. Fundamental Architectural Limitations

### 4.1 Order-0 Entropy Coding Is the Biggest Single Limitation

Everything in the current architecture feeds into order-0 Huffman (LZ77 path) or order-0 adaptive range coding (BWT path). This is the single most impactful architectural ceiling. Every modern high-ratio compressor uses at least order-1 context, and the best use context mixing with thousands of models.

**Impact:** Caps achievable ratio at roughly gzip-class for LZ77, bzip2-class for BWT. Context modeling would unlock ~15-25% further compression on text.

### 4.2 Block-Based vs Streaming

Block-based encoding forces table retransmission per block and prevents long-range adaptation. The current design can't leverage patterns that span block boundaries for entropy coding purposes (though LZ77 matches can cross boundaries since tokenization is global).

Streaming adaptive coding (like LZMA uses) eliminates table overhead entirely and allows continuous model adaptation.

### 4.3 Huffman vs Arithmetic/Range Coding on the LZ77 Path

The LZ77 path uses Huffman coding, which is integer-bit-aligned. Range coding gives fractional-bit precision. The BWT path already uses range coding. The LZ77 path should too.

On average, Huffman wastes ~0.05-0.1 bits per symbol vs range coding. Over millions of symbols (Moby Dick has ~1.27M bytes), that's 60-130 KB of waste potential — though in practice the waste is ~1-3% because most symbols have code lengths close to their ideal log2 cost.

### 4.4 Separate Namespaces for Literals and Matches

DEFLATE-style unified lit/length namespace (codes 0-255 are literals, 256=EOB, 257-285=lengths) means literals and length codes share a single Huffman tree. This is actually elegant but limits optimization: you can't use separate models for "what comes after a literal" vs "what comes after a match."

LZMA uses separate models for literal bytes, match lengths, and match distances, each with their own context models. This is architecturally superior.

### 4.5 No Repeat Match / Rep Codes

Modern LZ compressors (Zstd, LZMA) maintain a small buffer of recent match offsets (typically 3-4). When a new match reuses a recent offset, it's encoded with just 2-3 bits instead of the full distance code. This is HUGE for structured data where the same field offset repeats.

Autosqueeze has no rep-match mechanism. Every match encodes its full distance. On the logs and JSON files, this probably wastes 10-20% of distance coding bits.

### 4.6 Compress Speed: O(n²) Worst Case from Hash Chains

Hash chains with 512-depth limit and 1MB window make compression speed O(n × 512) per byte in the worst case. The current 0.1 MB/s is unacceptably slow for a production compressor. Suffix arrays or binary trees would give better matches faster.

---

## 5. Priority Recommendations (Bang-for-Buck)

If I had to rank fixes by (ratio improvement / implementation effort):

1. **Switch LZ77 path to range coding with order-1 context** — Biggest ratio win, moderate effort
2. **Add repeat-match (rep) codes** — Good ratio win, low effort  
3. **DEFLATE-style table compression (CL-CL)** — Moderate ratio win, low effort
4. **Increase max match length beyond 258** — Good for repetitive data, low effort
5. **Adaptive block splitting** — Moderate win, moderate effort
6. **Better hash (20-bit) + higher chain limit for long matches** — Moderate win, low effort
7. **3-4 DP iterations** — Small win, trivial effort
8. **Remove redundant bitstream size fields** — Tiny win, trivial effort
9. **Per-block mode selection (LZ77/BWT hybrid)** — Moderate win, moderate effort
10. **Data-type specific preprocessing** — High win but high effort and fragile

---

## 6. What's Actually Working Well

Credit where due:

- **DP optimal parsing** — Most LZ77 compressors use greedy or lazy matching. Optimal parsing is the right call for ratio.
- **1MB window** — Bigger than DEFLATE's 32KB. Good for log files and structured data.
- **BWT pipeline as alternative** — Smart to have two paths. BWT wins on some data.
- **Literal transforms** — XorDelta and MTF as block-level options is clever.
- **RLE-zero encoding in BWT path** — Correct bijective base-2 encoding of zero runs, matching bzip2's approach.
- **Range coder in BWT path** — Correct implementation with proper renormalization.

The foundation is solid. The wins come from layering context modeling on top.

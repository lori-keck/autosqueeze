# autosqueeze — Autonomous Compression Research

## Your Mission

You are an autonomous research agent. Your goal is to discover and implement novel compression algorithms that achieve the best possible compression ratio while maintaining reasonable speed.

## The Setup

- `src/compress.rs` — **THE FILE YOU EDIT.** Contains `compress()` and `decompress()` functions. Everything is fair game: algorithm, data structures, bit manipulation, entropy coding, dictionary approaches, transforms, etc.
- `src/benchmark.rs` — **DO NOT EDIT.** Measures compression ratio, speed, and correctness.
- `corpus/` — Test files of various types (text, binary, structured data). **DO NOT EDIT.**

## The Loop

For each experiment:

1. **Read** the current `src/compress.rs` and the last benchmark results
2. **Think** about what change might improve the compression ratio
3. **Edit** `src/compress.rs` with your proposed change
4. **Run tests** first: `cargo test --release 2>&1` — if tests fail, revert and try something else
5. **Run benchmark**: `cargo run --release --bin benchmark 2>&1`
6. **Evaluate**: Look at the `[SCORE]` section at the bottom
   - If `correct=false` → REVERT immediately, the change broke decompression
   - If `ratio` decreased (lower is better) → KEEP the change, commit with a descriptive message
   - If `ratio` increased or stayed the same → REVERT and try a different approach
7. **Log** what you tried and what happened (in your commit messages)
8. **Repeat** — try the next idea

## Constraints

- `compress()` takes `&[u8]` and returns `Vec<u8>`
- `decompress()` takes `&[u8]` and returns `Vec<u8>`
- **Lossless only**: `decompress(compress(data)) == data` must ALWAYS hold
- **No external crates** — stdlib only. The algorithm must be self-contained.
- **No hardcoding** — don't optimize for specific files in the corpus. The algorithm must be general-purpose.

## Research Directions to Explore

These are suggestions, not requirements. Try whatever you think might work.

### Tier 1 — Foundation (start here)
- LZ77/LZ78 — sliding window, dictionary-based compression
- Huffman coding — variable-length codes based on byte frequency
- Combine LZ + Huffman (this is basically what gzip/deflate does)

### Tier 2 — Advanced
- LZW (Lempel-Ziv-Welch) — dictionary that grows during compression
- Arithmetic coding — more efficient than Huffman for skewed distributions
- BWT (Burrows-Wheeler Transform) — reorders data to be more compressible, then use RLE/Huffman
- ANS (Asymmetric Numeral Systems) — modern entropy coding, faster than arithmetic coding
- Context mixing — predict next byte using multiple models, weight them

### Tier 3 — Novel / Experimental
- Hybrid approaches — combine multiple techniques adaptively based on data characteristics
- Block-level algorithm selection — analyze each block and pick the best algorithm for it
- Novel dictionary construction — what if you build the dictionary differently?
- Bit-level operations — work at the bit level instead of byte level
- Recursive compression — compress, then try to compress the compressed output
- Pattern detection — find and encode repeating patterns beyond simple byte sequences
- Delta encoding + compression — for structured/sequential data

### Tier 4 — Wild Ideas
- What if you used multiple passes?
- What if you preprocessed the data with a reversible transform before compressing?
- What if you split the data into channels (like separating RGB in an image) and compressed each differently?
- What if you used something nobody has tried before?

## What Success Looks Like

- **Starting baseline (RLE):** ratio ~1.0+ (RLE often makes things bigger on random data)
- **Good:** ratio < 0.7 (30% compression)
- **Great:** ratio < 0.5 (50% compression)
- **Excellent:** ratio < 0.4 (beating simple gzip territory)
- **Novel:** anything that achieves good compression through a technique that isn't a known algorithm

## Tips

- Start by replacing RLE with something fundamentally better (LZ77 is a good first step)
- Small incremental improvements are fine — you don't need to revolutionize compression in one step
- If an experiment fails to compile, read the error and fix it before trying something new
- The corpus has mixed file types — an algorithm that's great for text but terrible for binary isn't ideal
- Think about what makes data compressible: repetition, patterns, skewed distributions, structure

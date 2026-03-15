# Autosqueeze Research Briefing — March 15, 2026

## Current State
- **Ratio: 0.2589** | Compress: 0.1 MB/s | Decompress: 11 MB/s
- Per-file: Moby Dick 0.3015, JSON 0.0403, repetitive 0.0009, random 1.0001, source 0.2357, CSV 0.2724, logs 0.0670
- Moby Dick is 57% of compressed output — THE bottleneck

## Algorithm
- Dual-path: LZ77 (optimal DP, 1MB window, 512 hash chains) + BWT+MTF+RLE+order-1 range coder
- Per-file selector picks smaller
- Block Huffman (32K tokens) on LZ77 path, adaptive range coder on BWT path
- Per-block literal transforms: normal, XOR-delta, MTF
- Iterative 2-pass DP parser with Huffman-based cost model

## Failed Experiments (DON'T REPEAT)
- MTF without BWT, two-pass DP (PR #10), 4MB window, range coding with static headers
- Arithmetic replacing BWT pipeline, 3-byte hash chain, exhaustive DP length search
- Larger hash table (1M entries), compressed headers with extra overhead
- Rep-distance DP heuristic without bitstream support (GPT-5.4 tried, ratio 0.379)
- RLE prepass before LZ77 (no improvement — Moby Dick has 0.03% byte runs)
- Nibble coding after BWT+MTF (flat, headers ate the savings)
- BWT context partitioning (flat — per-partition headers ate savings)
- Better hash function / binary tree (LZ77 path isn't the bottleneck anymore)

## Top 5 Highest-Impact Changes (from 10 research agents)
1. **ORDER-1+ CONTEXT ON LZ77 PATH** — Switch from order-0 Huffman to range coding with context. 15-25% win.
2. **REP CODES** — Cache last 3-4 match distances, encode repeats in 2-3 bits. Easy, big win on structured data.
3. **SA-IS SUFFIX SORT** — Fix BWT from 0.1 MB/s to 5-50 MB/s. O(n) algorithm.
4. **REMOVE MAX MATCH 258** — Allow 65K+ matches. Wins on repetitive data.
5. **CM ON LZ77 LITERAL RESIDUALS** — Context mixing on what's left after LZ77 handles repetition.

## Key Insights from Research
- BWT pipeline is stronger than LZ77 for text — it effectively gives order-N context for free
- Moby Dick could theoretically go to 0.16 with order-6+ context mixing
- Our order-2 empirical entropy is 0.3539 for Moby Dick; we're ALREADY beating that
- The gap is all in order-3+ patterns (word completions, grammar, thematic repetition)
- Random.bin sets an incompressible floor of ~0.04 on overall ratio
- LZMA's key advantage: per-field context models (separate models for literals, lengths, distances) + rep codes
- zstd's key advantage: FSE/tANS entropy coding + rep codes + sequence abstraction
- cmix's key advantage: 2,077 models + LSTM neural mixing + preprocessing dictionaries

## Targets
- 0.22 = beat bzip2
- 0.20 = beat LZMA/xz class
- 0.15 = zpaq territory
- 0.12 = cmix territory (world record)

## Rules
- Edit ONLY `src/compress.rs`
- `cargo test --release` — ALL tests must pass
- `cargo run --release --bin benchmark` — verify ratio
- Branch per experiment, PR to main
- No external crates. Lossless only. Speed ≥1 MB/s preferred.

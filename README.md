# crunch 🗜️

**AI-discovered compression.** Point AI coding agents at a compressor, let them run overnight, wake up to a better algorithm.

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch), but for lossless data compression.

## What is this?

An experiment in autonomous algorithm discovery. We gave AI coding agents (Codex, Claude Code) a simple loop:

```
Read the compression code
    ↓
Make it better
    ↓
Run benchmarks (ratio + speed + correctness)
    ↓
Better? Keep. Worse? Revert.
    ↓
Repeat.
```

Three files:
- **`src/compress.rs`** — the algorithm (agents edit this)
- **`src/benchmark.rs`** — fixed scoring (measures ratio, speed, correctness)
- **`prepare.sh`** — sets up the test corpus

No human wrote the compression algorithm. We wrote the scaffolding, the agents wrote the compressor.

## Results

Starting from nothing (a basic RLE encoder that made files *bigger*), the agents discovered increasingly sophisticated techniques over ~30 iterations:

| Iteration | Technique | Ratio |
|-----------|-----------|-------|
| 1 | Run-length encoding | 1.913 ❌ |
| 2 | LZ77 sliding window | 0.490 |
| 3 | LZ77 + hash chains | 0.456 |
| 4 | LZ77 + lazy matching | 0.449 |
| 5 | Huffman coding | 0.371 |
| 6 | Block-adaptive Huffman | 0.338 |
| 7 | Optimal DP parsing + 1MB window | 0.295 |
| 8 | BWT + range coding | 0.265 |
| **9** | **Order-1 context modeling** | **0.259** |

**Compression ratio** = compressed size / original size. Lower is better.

The final Crunch algorithm achieves **0.259** — beating gzip (~0.33), matching bzip2 (~0.26), and approaching zstd-19 (~0.25) territory. From scratch. No external crates. Stdlib only.

### Per-file breakdown

```
  moby_dick.txt      1.3 MB → 384.8 KB   ratio: 0.3015
  structured.json  170.2 KB →   6.9 KB   ratio: 0.0403
  repetitive.bin   100.0 KB →     90 B   ratio: 0.0009
  random.bin        100.0 KB → 100.0 KB   ratio: 1.0001
  source_code.c    292.8 KB →  69.0 KB   ratio: 0.2357
  sensor_data.csv  331.3 KB →  90.2 KB   ratio: 0.2724
  logs.txt          328.8 KB →  22.0 KB   ratio: 0.0670

  OVERALL RATIO:  0.259
```

## The Crunch Algorithm

What the AI discovered is a dual-pipeline compressor that picks the best strategy per block:

**LZ77 pipeline:**
- Hash chain matching with optimal DP parsing (cost-model selects globally optimal parse, not greedy)
- 1MB sliding window with extended DEFLATE-style distance codes
- Per-block adaptive range coding (fractional-bit precision)
- Block preprocessing: normal, XOR-delta, or MTF — chosen per block

**BWT pipeline:**
- Burrows-Wheeler Transform for context sorting
- Move-to-Front transform to cluster symbols
- Run-length encoding on MTF output
- Adaptive order-1 range coder with iterative DP cost model

Each block automatically selects whichever pipeline produces smaller output.

## Quick start

```bash
# Set up test corpus
./prepare.sh

# Run benchmark
cargo run --release --bin benchmark

# Compress stdin → stdout
cargo run --release --bin compress < input.txt > compressed.bin

# Decompress
cargo run --release --bin compress -- -d < compressed.bin > output.txt

# Run tests
cargo test --release
```

## Constraints

- **Lossless** — `decompress(compress(data)) == data`, always
- **No external crates** — Rust stdlib only
- **General-purpose** — no hardcoding for specific file types
- **Correctness first** — any failing roundtrip rejects the experiment

## What we learned

AI agents are surprisingly good at incremental algorithm improvement when given:
1. A clear metric (compression ratio)
2. A fast feedback loop (compile → benchmark → score)
3. Freedom to explore (no restrictions on technique)

They independently discovered LZ77, Huffman coding, BWT, range coding, and optimal parsing — techniques that took humans decades to develop. They didn't invent anything *new*, but they composed known techniques effectively and found good parameter configurations autonomously.

The main limitation: agents optimize ruthlessly for the metric you give them. We measured ratio, so they tanked speed to 0.1 MB/s chasing marginal ratio gains. A production compressor would need a composite score balancing ratio and throughput.

## License

MIT

## Credits

Built by [Ego Death LLC](https://github.com/lori-keck). Compression discovered autonomously by AI coding agents.

Inspired by [@karpathy/autoresearch](https://github.com/karpathy/autoresearch).

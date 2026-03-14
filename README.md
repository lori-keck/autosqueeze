# autosqueeze 🗜️

Autonomous compression research — an AI agent searches for novel compression algorithms.

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch), but for compression instead of ML training. Point an AI coding agent at `program.md`, let it run overnight, wake up to a better compression algorithm.

## How it works

```
Agent reads program.md (instructions)
         ↓
Edits src/compress.rs (the algorithm)
         ↓
Runs cargo test (correctness check)
         ↓
Runs benchmark (compression ratio + speed)
         ↓
Better ratio? → Keep. Worse? → Revert.
         ↓
Repeat. 100+ experiments overnight.
```

Three files:
- **`src/compress.rs`** — the agent edits this. Contains `compress()` and `decompress()`.
- **`src/benchmark.rs`** — fixed scoring. Measures ratio, speed, correctness. Don't touch.
- **`program.md`** — agent instructions. The human edits this to guide research direction.

## Quick start

```bash
# 1. Prepare test corpus
./prepare.sh

# 2. Run baseline benchmark
cargo run --release --bin benchmark

# 3. Run tests
cargo test --release

# 4. Point your AI agent at program.md and let it rip
```

## The metric

**Compression ratio** = compressed_size / original_size. Lower is better.

- Starting baseline (RLE): ~1.0+ (often makes things bigger)
- Good: < 0.7
- Great: < 0.5
- Excellent: < 0.4

## Corpus

Mixed file types to test general-purpose compression:
- English text (Moby Dick)
- JSON structured data
- Repetitive binary patterns
- Pseudo-random binary (hard mode)
- Source code
- CSV sensor data
- Log files

## Rules

- Lossless only — `decompress(compress(data)) == data`, always
- No external crates — stdlib only
- No hardcoding for specific corpus files
- The algorithm must be general-purpose

## License

MIT

## Credits

Built by [Ego Death LLC](https://github.com/lori-keck)

Inspired by [@karpathy/autoresearch](https://github.com/karpathy/autoresearch)

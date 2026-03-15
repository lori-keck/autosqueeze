# autosqueeze — Autonomous Compression Research

## Your Mission

You are an autonomous research agent. Your goal is to discover and implement novel compression algorithms that achieve the best possible compression ratio while maintaining reasonable speed.

## The Setup

- `src/compress.rs` — **THE FILE YOU EDIT.** Contains `compress()` and `decompress()` functions. Everything is fair game: algorithm, data structures, bit manipulation, entropy coding, dictionary approaches, transforms, etc.
- `src/benchmark.rs` — **DO NOT EDIT.** Measures compression ratio, speed, and correctness.
- `corpus/` — Test files of various types (text, binary, structured data). **DO NOT EDIT.**

## IMPORTANT: Isolated Worktree Setup

**Multiple agents run in parallel.** You MUST use a git worktree to avoid collisions.

At the start of your session, run these commands EXACTLY:

```bash
REPO=/Users/lorikeck/github/autosqueeze
BRANCH="experiment/$(echo $RANDOM$RANDOM | head -c 8)"
WORKTREE="/tmp/autosqueeze-${BRANCH##*/}"

cd "$REPO" && git fetch origin main
git worktree add "$WORKTREE" -b "$BRANCH" origin/main
cd "$WORKTREE"
```

Then do ALL your work inside `$WORKTREE`. Never `cd` back to the main repo.

When you're done and have pushed:
```bash
cd /tmp && git -C "$REPO" worktree remove "$WORKTREE" 2>/dev/null
```

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

Read RESEARCH.md for detailed background on each technique. These are ordered by expected impact.

### Phase 1 — Foundation (START HERE)
- **LZ77** — sliding window, find repeated byte sequences, encode as (offset, length) pairs. This alone should drop ratio from 1.9 to ~0.5-0.7. This is the basis of gzip, zstd, and most modern compressors.
- Keep the implementation clean and correct. Speed doesn't matter yet.

### Phase 2 — Entropy Coding
- **Huffman coding** on top of LZ77 output — frequent symbols get short codes, rare ones get long codes.
- This is what gzip/deflate does (LZ77 + Huffman). Expected: another 10-20% improvement.
- Later: consider replacing Huffman with **ANS/FSE** (Asymmetric Numeral Systems) — faster and slightly better ratios. This is what zstd uses.

### Phase 3 — Transforms
- **BWT (Burrows-Wheeler Transform)** — reorders data so similar bytes cluster together, then simpler algorithms work better. bzip2 uses this.
- **Move-to-front transform** — after BWT, recently-seen bytes get small codes
- **Delta encoding** — store differences between consecutive values (great for structured data like CSV)

### Phase 4 — Advanced
- **Block-level algorithm selection** — analyze each block's characteristics and pick the best algorithm for it (some blocks are text, some are binary, some are random)
- **Adaptive dictionary** — build and maintain a dictionary during compression
- **Context modeling** — predict the next byte using the bytes you've already seen. The better your prediction, the less information you need to encode.

### Phase 5 — Novel / Experimental
- **Simplified context mixing** — multiple prediction models weighted together. This is what the Hutter Prize winners use (cmix/PAQ). Even a basic version could be interesting.
- **Multi-pass compression** — compress, analyze the output, compress again with different parameters
- **Byte-level pattern mining** — find repeating patterns beyond what LZ77's sliding window catches
- **Hybrid approaches** — what if you tried something nobody has tried before?
- **Channel separation** — split data by byte position or characteristics, compress each stream differently

### Key Insight
Compression IS prediction. If you can predict the next byte with 100% accuracy, the compressed size is zero. Every improvement in prediction = improvement in compression. The best compressors (cmix) are essentially language models.

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

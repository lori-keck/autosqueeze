# Compression Research Notes

## The Landscape (as of March 2026)

### Current Best Algorithms (ratio → speed tradeoff)

| Algorithm | Typical Ratio | Compress Speed | Decompress Speed | Notes |
|-----------|--------------|----------------|-------------------|-------|
| cmix/PAQ | ~0.11 | ~2 KB/s | ~2 KB/s | Best ratio ever. Unusably slow. Context mixing. |
| lzma/xz | ~0.30 | ~5 MB/s | ~50 MB/s | 7-Zip. Great ratio, slow compress. |
| brotli (max) | ~0.32 | ~3 MB/s | ~300 MB/s | Google. Used in web (Chrome, CDNs). |
| zstd (max) | ~0.35 | ~3 MB/s | ~800 MB/s | Facebook. 22 compression levels. |
| zstd (default) | ~0.40 | ~300 MB/s | ~800 MB/s | The sweet spot. Best all-rounder. |
| gzip/deflate | ~0.45 | ~30 MB/s | ~300 MB/s | The internet standard since 1992. |
| lz4 | ~0.60 | ~800 MB/s | ~4 GB/s | Real-time. Databases, filesystems. |
| snappy | ~0.65 | ~500 MB/s | ~1.5 GB/s | Google. Hadoop, BigTable. |
| **Our RLE baseline** | **1.91** | 400 MB/s | 500 MB/s | Makes everything bigger 😂 |

### Our starting point: ratio 1.913 (RLE)
### Realistic target: ratio < 0.50 (beating naive gzip)
### Stretch target: ratio < 0.35 (zstd territory)
### Moonshot: anything novel that hasn't been tried before

## Key Techniques to Explore

### 1. LZ77 (Lempel-Ziv 1977) — The Foundation
- Sliding window, find repeated sequences, encode as (offset, length) pairs
- The basis of gzip, deflate, zstd, and most modern compressors
- **This should be our first replacement for RLE**
- Expected improvement: ratio from 1.9 → ~0.5-0.7

### 2. Huffman Coding — Entropy Coding
- Variable-length codes: frequent bytes get short codes, rare bytes get long codes
- Usually paired with LZ77 (LZ77 finds repetition, Huffman encodes efficiently)
- gzip = LZ77 + Huffman (this is the deflate algorithm)
- Expected improvement on top of LZ77: another 10-20% reduction

### 3. BWT (Burrows-Wheeler Transform)
- Doesn't compress directly — REORDERS data so similar bytes cluster together
- Then simple algorithms (RLE, move-to-front + Huffman) work way better
- bzip2 uses BWT + MTF + Huffman
- Particularly good on text (which is most of our corpus)
- Reversible — you can reconstruct the original from the transformed version

### 4. ANS (Asymmetric Numeral Systems)
- Modern replacement for Huffman and arithmetic coding
- Invented by Jarek Duda (~2009)
- Used by Facebook's zstd (they call it FSE — Finite State Entropy)
- Faster than arithmetic coding, better compression than Huffman
- The "right" entropy coder for 2026

### 5. Context Mixing — The Frontier
- The technique behind cmix, PAQ, and Hutter Prize winners
- Multiple prediction models, each guessing the next byte
- Predictions get weighted and combined
- Key insight: **compression IS prediction**
- If you can predict the next byte with 99% accuracy, you barely need to encode it
- Slow but produces the best ratios known
- A simplified version could be interesting

### 6. Dictionary Approaches
- zstd can train custom dictionaries on sample data
- Massive gains on structured/repetitive data (JSON, logs, CSV)
- Agent could try building dictionaries adaptively during compression

### 7. Preprocessing Transforms
- Delta encoding (store differences between consecutive values)
- Run-length encoding AFTER a transform (BWT makes RLE actually useful)
- Byte reordering / channel separation
- Move-to-front transform (recently seen bytes get small codes)

## The Hutter Prize

- Compress 1GB of Wikipedia as small as possible
- Current record: **110.8 MB** (Sept 2024, by Kaido Orav & Byron Knoll)
- Uses fx2-cmix (context mixing + neural preprocessing)
- Prize: €5,000 per 1% improvement
- Key insight from the prize: compression quality correlates with AI/language understanding

## The LLM-as-Compressor Angle

- DeepMind showed Chinchilla (LLM) beats gzip on text compression
- The idea: use a model to predict next byte, encode only the "surprise"
- Nobody has made this practical (inference is too slow)
- But a simplified version (small context model, not a full LLM) could work
- This is essentially what context mixing does, just with simpler models

## Who's Working on This

- **Yann Collet** — created zstd and lz4 at Facebook. The GOAT of practical compression.
- **Jarek Duda** — invented ANS. Polish mathematician.
- **Matt Mahoney** — created PAQ series, major Hutter Prize contributor
- **Byron Knoll** — current Hutter Prize record holder (with Kaido Orav)
- **Large Text Compression Benchmark** (mattmahoney.net/dc/text.html) — the leaderboard
- Nobody has pointed autoresearch at compression yet — we might be first.

## Suggested Agent Research Path

1. **Phase 1: Foundation** — Replace RLE with LZ77. Instant massive improvement.
2. **Phase 2: Entropy** — Add Huffman coding on output. Another 10-20%.
3. **Phase 3: Transform** — Try BWT preprocessing before LZ. Big wins on text.
4. **Phase 4: Modern entropy** — Replace Huffman with ANS/FSE.
5. **Phase 5: Adaptive** — Block-level algorithm selection based on data type.
6. **Phase 6: Context** — Simplified context mixing. The frontier.
7. **Phase 7: Wild** — Let the agent try whatever it wants. See what happens.

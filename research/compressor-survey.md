# World-Class Compressor Survey

> Deep analysis of every major compressor's architecture, innovations, and stealable ideas for autosqueeze.

---

## Table of Contents

1. [cmix — The Compression King](#1-cmix--the-compression-king)
2. [PAQ8 — The Context Mixing Family](#2-paq8--the-context-mixing-family)
3. [ZPAQ — The Programmable Archiver](#3-zpaq--the-programmable-archiver)
4. [LZMA/xz — The Ratio Champion](#4-lzmaxz--the-ratio-champion)
5. [Brotli — Google's Web Compressor](#5-brotli--googles-web-compressor)
6. [zstd — The Perfect Balance](#6-zstd--the-perfect-balance)
7. [BSC — The BWT Successor](#7-bsc--the-bwt-successor)
8. [Cross-Compressor Technique Matrix](#8-cross-compressor-technique-matrix)
9. [What We Should Steal](#9-what-we-should-steal)

---

## 1. cmix — The Compression King

### Overview
- **Author:** Byron Knoll (started Dec 2013)
- **Current version:** v21 (Sept 2024)
- **enwik8 ratio:** 14.6 MB / 100 MB = **0.146** (~1.17 bits/byte)
- **enwik9 ratio:** 107.9 MB / 1000 MB = **0.108**
- **Speed:** ~1.5 KB/s compress AND decompress
- **Memory:** 32 GB recommended (20+ GB typical)
- **License:** GPL

### Architecture (Three Layers)

#### Layer 1: Preprocessing
Before any compression happens, cmix transforms the input to make it more compressible:

- **Binary executable filtering** — x86/ARM instructions get their relative call/jump addresses converted to absolute. This clusters similar addresses together. Borrowed from paq8pxd.
- **Natural language transform** — Words get replaced with dictionary indices. Uses a 412 KB English dictionary (from the fx-cmix Hutter Prize entry). "the" → token 3, etc. Massively reduces entropy of text.
- **Image detection & filtering** — Detects PNG/BMP/TIFF headers, applies delta filtering (pixel = pixel - predicted_pixel). Also from paq8pxd.

#### Layer 2: 2,077 Independent Models
This is where the magic lives. cmix v21 runs **2,077 separate prediction models** simultaneously. Each model, for every single bit, outputs a probability that the next bit will be 1. Model types include:

- **Order-N context models** — Predict based on the last N bytes (N from 0 to 20+). Higher orders catch longer patterns but are sparser.
- **Word models** — Track word boundaries, predict based on previous words.
- **Match models** — Find the longest match of the current context in history, predict based on what followed the match.
- **Sparse models** — Use non-contiguous context bytes (e.g., every other byte, or bytes at specific offsets). Catches periodic patterns.
- **Indirect context models (ICM)** — Two-level: context → bit history state → prediction. The indirection allows generalization across similar contexts.
- **Record models** — Detect fixed-length records (like CSV rows or binary structs) and use column-based prediction.
- **Image models** — 2D context models that look at neighboring pixels (above, left, above-left). Use PNG-style predictors.
- **Distance models** — Model the relationship between current position and recent match distances.
- **Specialized models** — x86 opcode models, XML/HTML tag models, etc.

Most models are sourced from paq8l, paq8pxd, and fxcm (all open source context mixers). cmix's contribution is combining an absurd number of them.

#### Layer 3: LSTM Neural Network Mixer
Here's cmix's key innovation over PAQ. Instead of mixing with a simple logistic regression (like PAQ does), cmix uses an **LSTM (Long Short-Term Memory) neural network** to combine the 2,077 model predictions:

1. All 2,077 predictions are transformed to logistic domain: `log(p / (1-p))`
2. These become the input to an LSTM network
3. The LSTM is trained online using **backpropagation through time (BPTT)**
4. Uses **Adam optimizer** with layer normalization and learning rate decay
5. LSTM forget and input gates are coupled (reduces parameters)
6. The LSTM learns which models to trust in which contexts, and captures temporal patterns in model reliability
7. Output is a single probability, fed to arithmetic coder

The LSTM mixer is what separates cmix from PAQ8. The neural network can learn complex, non-linear relationships between model predictions that a linear mixer misses. This alone accounts for roughly 5-10% better compression over PAQ8px on text.

After the LSTM, there's also a **Secondary Symbol Estimation (SSE)** stage that further refines the probability based on recent prediction accuracy in similar contexts.

### Why It's So Good
- **Sheer model count**: 2,077 different "opinions" about each bit
- **Neural mixing**: LSTM captures which models to trust when, and adapts over time
- **Preprocessing**: Getting text into dictionary-token form is huge (probably 15-20% alone)
- **Bit-level prediction**: Every single bit is individually predicted and coded
- **Arithmetic coding**: Theoretically optimal entropy coding given perfect probabilities

### Why It's So Slow
- Every bit requires forward pass through LSTM + all 2,077 models
- 8 bits per byte × millions of bytes = billions of model evaluations
- LSTM backpropagation for online learning at every step
- 32 GB of model state tables

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| Preprocessing (dictionary transform for text) | Medium | 15-20% better ratio | **YES** — huge ROI |
| Preprocessing (exe filter) | Medium | 5-10% on binaries | Yes, if we handle binaries |
| Multiple model predictions + mixing | High | 10-20% over single model | Yes, but start with 3-5 models not 2000 |
| SSE (post-prediction refinement) | Medium | 2-5% | Yes, cheap to add |
| LSTM mixer | Very High | 5-10% over linear mixing | Not yet — way too complex/slow |
| Bit-level prediction | Medium | Better granularity | Maybe later, byte-level is fine to start |

---

## 2. PAQ8 — The Context Mixing Family

### Overview
- **Author:** Matt Mahoney + community (Alexander Ratushnyak, Serge Osnach, many others)
- **Active variants:** paq8px (most developed), paq8pxd
- **enwik8 ratio:** ~1.27 bits/byte (paq8px), ~12.7 MB compressed
- **Speed:** ~5-15 KB/s
- **Memory:** 1-8 GB depending on settings
- **License:** GPL

### The PAQ Family Tree
```
PAQ1 (2002) — simple context mixing
  → PAQ6 (2004) — adaptive weighting, SSE
    → PAQ7 (2005) — logistic mixing replaces linear
      → PAQ8 (2006) — specialized models, file type detection
        → paq8l, paq8o, paq8px, paq8pxd (ongoing community forks)
```

### Architecture: The Context Mixing Pipeline

The entire PAQ architecture can be understood as a prediction pipeline:

```
Input byte stream
    ↓
[Preprocessing] — detect file types, apply transforms
    ↓
[Bit extraction] — process one bit at a time
    ↓
[Context Models] — 500+ models each produce a probability
    ↓
[Logistic Mixing] — weighted combination in logistic space
    ↓
[SSE / APM] — secondary probability refinement
    ↓
[Arithmetic Coder] — encode bit using final probability
    ↓
Compressed output
```

### Key Components (Deep Dive)

#### A. Context Models (the prediction engines)

Each context model maintains a table mapping contexts to predictions. The core types:

**1. Direct Context Model (CM)**
- Maps a hash of the last N bytes to a prediction counter
- Counter = (n0, n1) where n0 = count of 0-bits, n1 = count of 1-bits seen after this context
- Prediction = n1 / (n0 + n1), with smoothing
- When counts get too high, both are halved (aging mechanism — recent data matters more)
- Multiple orders run simultaneously (order 0 through order 8+)

**2. Indirect Context Model (ICM)**
- Two-level mapping: context → 8-bit state → prediction
- The state is a compact representation of recent bit history (up to ~40 bits of history stored in 8 bits using a state machine)
- **State table**: 256 states, each encoding a specific pattern of recent 0s and 1s
- The indirection allows many different contexts to share statistics through common bit-history states
- Fewer parameters than direct CM, better generalization on sparse data

**3. Match Model**
- Finds the longest match of the current context anywhere in the input history
- Uses a hash table of context hashes → positions
- Predicts based on what bit followed the match, with confidence proportional to match length
- Long matches → very confident predictions (and very good compression)
- This is conceptually similar to LZ77 but used for prediction, not encoding

**4. Word Model (text-specific)**
- Tracks word boundaries (spaces, punctuation)
- Maintains contexts based on: current partial word, previous word, word pairs
- Handles capitalization separately
- Critical for English text compression

**5. Sparse Model**
- Uses non-contiguous bytes as context (e.g., bytes at positions -1, -3, -5)
- Catches periodic patterns like CSV columns, fixed-width records
- Multiple sparse patterns run in parallel

**6. Record Model**
- Auto-detects fixed-length records
- Uses (column_position, previous_column_value) as context
- Excellent for tabular data, binary formats with fixed structs

#### B. Logistic Mixing (the combiner)

This is PAQ's signature innovation. Given N model predictions p₁...pₙ:

1. **Transform to logistic domain**: stretch(p) = ln(p / (1-p))
2. **Weighted sum**: Σ wᵢ × stretch(pᵢ)
3. **Transform back**: squash(x) = 1 / (1 + e⁻ˣ)
4. **Update weights** using gradient descent on coding cost

The weight update rule:
```
wᵢ += η × (y - p) × stretch(pᵢ)
```
where y is the actual bit, p is the combined prediction, η is the learning rate (~0.002–0.01).

**Why logistic mixing beats linear mixing:** Extreme predictions (p near 0 or 1) get amplified. A model that says "99% sure it's a 1" contributes much more than a wishy-washy "55% sure" model. This matches intuition — confident models should dominate.

**Hierarchical mixing**: PAQ8 uses multiple layers of mixers. First-stage mixers combine related models (e.g., all text models together). Second-stage mixers combine the first-stage outputs. This forms a tree:

```
          Final prediction
               ↑
        [Mixer Layer 2]
        ↗            ↖
[Text Mixer]    [Binary Mixer]
   ↑  ↑  ↑         ↑  ↑  ↑
  models...       models...
```

Different mixer sets are selected based on detected data type.

#### C. SSE (Secondary Symbol Estimation) / APM (Adaptive Probability Map)

After mixing, PAQ applies one or more SSE stages:

1. Take the mixed prediction p (quantized to ~32 levels)
2. Use a context (e.g., previous byte) to select a table row
3. The table maps (context, quantized_p) → refined_p
4. The table entries are updated toward the actual outcome

This corrects systematic biases in the mixer output. For example, if the mixer consistently predicts 70% but the truth is 80% in certain contexts, the SSE learns this correction.

**APM** is essentially the same thing. Multiple SSE/APM stages can be chained, each using different contexts for refinement. Typical PAQ8 uses 2-4 APM stages after mixing. Each stage reduces error by ~1%.

#### D. Arithmetic Coding

PAQ uses a standard binary arithmetic coder:
- Maintains an interval [low, high)
- For each bit, splits interval proportionally based on predicted probability
- If bit = 0, take the lower portion; if bit = 1, take the upper portion
- When the interval gets small enough, output bits and renormalize
- Theoretical limit: output = Σ -log₂(p_correct) bits, where p_correct is the probability assigned to the actual bit

The arithmetic coder itself is nearly optimal — all compression quality depends on how good the probability predictions are.

### Benchmark Numbers (paq8px on enwik8)
- **Compressed size:** ~12.7 MB (ratio 0.127)
- **Bits per byte:** ~1.27
- **Compression speed:** ~5-15 KB/s
- **Memory:** ~2-8 GB depending on model selection

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| Logistic mixing of multiple models | Medium | 15-20% over single model | **YES** — core technique |
| Weight update via gradient descent | Low | Built into logistic mixing | **YES** |
| Indirect context models (ICM) | Medium | Better generalization | Yes, for sparse data |
| SSE/APM post-refinement | Low-Medium | 2-5% each stage | **YES** — cheap gains |
| Match model for prediction | Medium | 5-10% | **YES** |
| File type detection + specialized models | Medium | 10-15% | Yes, later phases |
| Bit history state tables | Medium | Memory efficiency | Maybe, adds complexity |
| Hierarchical mixer tree | Medium | 3-5% over flat mixing | Later optimization |

---

## 3. ZPAQ — The Programmable Archiver

### Overview
- **Author:** Matt Mahoney (same person who created PAQ)
- **Current version:** 7.15 (Sept 2016, essentially feature-complete)
- **Compression:** Comparable to 7-zip at level 5, much better at max
- **Speed:** Varies wildly by level (10 KB/s to 100 MB/s)
- **Memory:** 30 MB to 2 GB
- **License:** MIT / Public Domain

### Architecture: The Programmable Compressor

ZPAQ's radical innovation: **it doesn't define a compression algorithm**. Instead, it defines a virtual machine (ZPAQL) that implements the decompressor, and stores the decompressor program in the archive header.

This means:
- Archives are **forward-compatible forever** — any future version can read old archives
- The algorithm can be **optimized per data type** — different blocks use different algorithms
- The decompressor is small enough to store in the header (typically 100-500 bytes of bytecode)

#### ZPAQL Virtual Machine
- Register-based: 4 32-bit registers (A, B, C, D), condition flag, 16-bit PC
- Two memory arrays: M (bytes) and H (32-bit words)
- ~40 instructions (arithmetic, logic, branches, memory access)
- No stack, no CALL instruction (keeps it simple)
- JIT-compiled to x86/x86-64 for speed (since v4.00)

#### Three Sections of a ZPAQ Program

**1. COMP — Context Model Chain**
An ordered sequence of up to 255 component instances, each one of 9 types:

| Type | Name | Function |
|------|------|----------|
| CONST | Constant | Fixed prediction (baseline) |
| CM | Context Model | Hash(context) → prediction table lookup |
| ICM | Indirect Context Model | Context → bit-history state → prediction |
| MIX | Mixer | Weighted average of N predictions in logistic domain |
| MIX2 | Binary Mixer | 2-input mixer with weights summing to 1 |
| AVG | Fixed Average | MIX2 with fixed weights |
| SSE | Secondary Symbol Estimation | Refines prediction using context + quantized input prediction |
| ISSE | Indirect SSE | Bit-history-based weight selection for mixing |
| MATCH | Match Model | Longest-match prediction |

These are the same building blocks as PAQ, but here they're **configurable, not hardcoded**.

**2. HCOMP — Context Computation**
A ZPAQL program that computes contexts for the COMP components. Called once per decoded byte. Can implement arbitrary context logic — word parsing, record detection, etc.

**3. PCOMP — Post-processing**
Optional ZPAQL program that transforms decoded output. Used for things like inverse delta coding, decompressing JPEG, etc. Only runs during decompression.

#### Compression Levels (Built-in Presets)

ZPAQ offers preset levels that select different COMP/HCOMP/PCOMP configurations:

- **Level 1:** LZ77 only (fast, similar to gzip)
- **Level 2:** LZ77 with BWT post-processing
- **Level 3:** Mid-range context mixing (~20 models)
- **Level 4:** Full context mixing (~50+ models, similar to PAQ)
- **Level 5:** Maximum context mixing + specialized models

#### Journaling / Deduplication
- Archives are append-only — updates add new transaction blocks
- Files are split into variable-size fragments using content-defined chunking (Rabin fingerprinting)
- Each fragment is SHA-1 hashed; duplicates are stored once
- Can roll back to any previous state (every update is a "snapshot")

### Key Innovations
1. **Algorithm-in-the-archive**: Future-proofing through VM embedding
2. **JIT compilation**: ZPAQL bytecode compiled to native x86 at runtime (~10x faster than interpretation)
3. **Configurable mixing chains**: Same components as PAQ but composable, not hardcoded
4. **Content-defined chunking**: Sub-file deduplication via rolling hash

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| Content-defined chunking for dedup | Medium | Huge on redundant data | Yes, if we handle archives |
| Configurable compression pipelines | High | Flexibility | Yes long-term, not now |
| JIT compilation of hot paths | Very High | 10x speed | Not yet |
| The 9 component types as building blocks | Medium | Great architecture model | **YES** — good design reference |
| LZ77 + BWT hybrid | Medium | Good ratio/speed balance | Worth testing |

---

## 4. LZMA/xz — The Ratio Champion

### Overview
- **Author:** Igor Pavlov (7-Zip), started ~1998
- **enwik8 ratio:** ~0.30 (30 MB compressed)
- **Silesia ratio:** ~0.31
- **Compress speed:** ~3-5 MB/s (typical settings)
- **Decompress speed:** ~50-100 MB/s (fast!)
- **Memory:** 12 MB - 1.5 GB (compression), 1-65 MB (decompression)
- **License:** Public domain (LZMA SDK)

### Why LZMA Crushes DEFLATE

DEFLATE (gzip/zip) typically gets ~0.45 ratio. LZMA gets ~0.30. That's 33% better. Here's why:

| Feature | DEFLATE | LZMA |
|---------|---------|------|
| Window size | 32 KB | Up to 4 GB |
| Entropy coder | Huffman | Range coder |
| Match finder | Hash chains only | Hash chains + binary trees + Patricia trees |
| Match optimization | Greedy/lazy | Dynamic programming (optimal parsing) |
| Context sensitivity | Minimal | Extensive per-field contexts |
| Repeat distances | None | 4-slot repeat distance cache |

### Architecture (Deep Dive)

#### Stage 1: LZ77 Match Finding

LZMA's match finder is far more sophisticated than DEFLATE's:

**Binary Tree Match Finder (BT4)**
- Maintains a balanced binary tree of hash chains
- For each position, find ALL matches up to the maximum length
- Binary tree nodes sorted by match content
- Hash table with 4-byte keys for initial lookup, then tree traversal for longer matches
- Finds the longest match AND all shorter matches simultaneously
- This matters because sometimes a shorter match followed by another match beats one long match

**Hash Chain Match Finder (HC5)**
- Faster than BT4 but finds fewer matches
- 5-byte hash for initial lookup
- Follows chain of positions with same hash
- Good for fast compression modes

**Match Distance Repetition (Rep Codes)**
LZMA maintains a cache of the 4 most recently used match distances. The packet format has special "repeat" codes:
- **SHORTREP**: Repeat the last distance, length 1 (just one byte copy)
- **LONGREP[0]**: Repeat the last distance, explicit length
- **LONGREP[1-3]**: Repeat the 2nd/3rd/4th most recent distance
- These encode with fewer bits than a new distance, and they're extremely common in real data (think: two arrays interleaved, structured records, etc.)

**Optimal Parsing (Dynamic Programming)**
This is LZMA's most important innovation. Instead of greedily picking the longest match:

1. Consider all possible encodings of the next few bytes:
   - Literal (encode the byte directly)
   - Match with any of the found distances/lengths
   - Repeat with any of the 4 cached distances
2. Use dynamic programming to find the combination of packets that minimizes total encoded size
3. Look ahead up to 4096 bytes to make globally better decisions

This alone accounts for ~5-10% better compression over greedy matching.

#### Stage 2: Context-Sensitive Range Coding

Instead of using Huffman coding (like DEFLATE), LZMA uses a **binary range coder** with extensive context modeling:

**Range Coder Basics:**
- Maintains an interval [low, low + range)
- For each bit, splits the interval proportionally: bit=0 gets p×range, bit=1 gets (1-p)×range
- When range gets too small, outputs bytes and renormalizes
- Mathematically equivalent to arithmetic coding but avoids patented implementations

**The Critical Innovation — Per-Field Contexts:**
LZMA encodes each bit with a **context specific to what that bit represents**. This is what most people miss about LZMA:

- A "match vs literal" decision bit uses (previous packet type, position in file % pb) as context
- Literal bytes use (high bits of previous byte, position % lp) as context
- Match length bits use different contexts for each bit position within the length field
- Distance slot bits use different contexts for each bit position

This prevents unrelated bit types from polluting each other's statistics. DEFLATE's Huffman coding treats all literal/length/distance codes in the same context — huge missed opportunity.

**Context Parameters (lc, lp, pb):**
- **lc** (literal context bits, 0-8): How many bits of the previous byte to use for literal context. Default 3.
- **lp** (literal position bits, 0-4): How many bits of position to use for literal context. Default 0.
- **pb** (position bits, 0-4): How many bits of position to use for match/rep decision context. Default 2.

These let LZMA adapt to data alignment. Setting pb=3 helps with 8-byte-aligned data (like 64-bit structures).

### Benchmark Numbers
| Corpus | Size | LZMA compressed | Ratio |
|--------|------|----------------|-------|
| enwik8 | 100 MB | 29.7 MB | 0.297 |
| enwik9 | 1 GB | 254 MB | 0.254 |
| Silesia | 212 MB | 65.4 MB | 0.309 |
| Calgary | 3.15 MB | 0.96 MB | 0.305 |

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| Repeat distance cache (rep codes) | Low-Medium | 5-10% | **YES** — easy win |
| Optimal parsing (DP) | High | 5-10% over greedy | Yes, but after basic LZ77 works |
| Per-field context modeling | Medium | 10-15% over flat Huffman | **YES** — this is huge |
| Range coder instead of Huffman | Medium | 2-5% | Yes, but ANS/FSE is better |
| Binary tree match finder | Medium-High | Better matches = better compression | Yes, after hash chains |
| Position-dependent contexts (pb/lp) | Low | 2-5% on aligned data | **YES** |

---

## 5. Brotli — Google's Web Compressor

### Overview
- **Author:** Jyrki Alakuijala and Zoltán Szabadka (Google), 2013-2015
- **RFC:** 7932
- **enwik8 ratio:** ~0.32 (level 11)
- **Compress speed:** ~3 MB/s (level 11), ~300 MB/s (level 1)
- **Decompress speed:** ~300-500 MB/s
- **Memory:** 1 MB - 400 MB
- **License:** MIT
- **Adoption:** Supported by all major browsers, CDNs, HTTP/2

### Architecture

Brotli is essentially **LZ77 + Huffman + context modeling + a big static dictionary**. It's a next-generation DEFLATE designed specifically for web content.

#### Key Innovation 1: Static Dictionary (120 KiB)

Brotli ships with a **pre-built dictionary of ~120 KiB** containing:
- Common English words and word fragments
- Common HTML/CSS/JavaScript tokens
- Common HTTP headers
- Common programming constructs

The dictionary contains **122 word lists** with 6 transform types each (original, uppercase first, uppercase all, with trailing space, with trailing punctuation, etc.) = **732 effective dictionaries** of different transform+word combinations.

This is transformative for small files. Consider compressing a 1 KB HTML page:
- gzip has to build its dictionary from 1 KB of data — barely anything to work with
- Brotli already knows "<!DOCTYPE html>", "<div class=", "function()", etc.
- On small web resources, Brotli beats gzip by **20-30%** largely due to this dictionary

#### Key Innovation 2: Context Modeling for Literals

Unlike DEFLATE (which uses one Huffman table for all literals in a block), Brotli uses **2nd-order context modeling**:

1. Literals are predicted using the **previous two bytes** as context
2. Each unique (prev_byte, prev_prev_byte) pair maps to a **context ID** (0-63)
3. Each context ID selects from one of up to 256 different Huffman tables
4. The context mapping is per-block and stored in the compressed stream

This means Brotli learns that after "th" the letter "e" is very likely (short code), while after "qx" all letters are roughly equally likely (longer codes). DEFLATE can't distinguish these cases.

Context IDs are computed using one of 4 preset **context mode functions**:
- **LSB6**: Use low 6 bits of previous byte (good for binary data)
- **MSB6**: Use high 6 bits (good for UTF-8)
- **UTF8**: Special mapping optimized for UTF-8 text
- **Signed**: For signed numeric data

#### Key Innovation 3: Block Splitting

Brotli divides input into **meta-blocks**, and each meta-block into **blocks** that can use different Huffman tables. The block splitting algorithm uses a graph-theoretic approach:

1. Split the data into small initial blocks
2. Build a graph where nodes are blocks and edges represent the cost of merging
3. Use shortest-path / minimum-cost algorithms to find optimal block boundaries
4. Each block independently selects its Huffman table set

This handles mixed content (e.g., an HTML file with embedded JavaScript, CSS, and base64 images) much better than a single table for the whole file.

#### Key Innovation 4: Copy Distance Modeling

Like LZMA, Brotli maintains a **ring buffer of recent distances** (the last 4 distances plus some derived distances like last_distance ± 1). Copy distances are encoded as:
- Recent distance (very cheap — 2-3 bits)
- Short distance (direct code)
- Long distance (prefix code + extra bits)

Additionally, Brotli defines **distance context** — the Huffman table used for distance encoding depends on the copy length. Short copies tend to have short distances; long copies might reference further back. Using separate tables for each exploits this correlation.

#### Format Structure
```
Stream = Header + MetaBlock*

MetaBlock = MetaBlockHeader + CommandBlock*

CommandBlock = InsertLengthCode + CopyLengthCode + Literal* + DistanceCode

Insert/Copy lengths are encoded jointly as "commands" using a 2D code table,
exploiting the correlation between insert and copy lengths.
```

### Benchmark Numbers
| Corpus | Size | Brotli (level 11) | Ratio | gzip (level 9) | Brotli advantage |
|--------|------|-------------------|-------|----------------|-----------------|
| enwik8 | 100 MB | 32.1 MB | 0.321 | 36.4 MB | 12% smaller |
| Silesia | 212 MB | 70.8 MB | 0.334 | 76.1 MB | 7% smaller |
| HTML (typical) | varies | varies | varies | varies | 15-25% smaller |
| Small files (<10KB) | varies | varies | varies | varies | 20-30% smaller |

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| Static dictionary for common patterns | Low | 10-30% on small files, 3-5% on large | **YES** — easy and effective |
| Context modeling for literals (2nd order) | Medium | 5-10% over flat Huffman | **YES** |
| Joint insert/copy length coding | Medium | 2-3% | Later optimization |
| Block splitting with cost analysis | High | 5-10% on mixed content | Yes, but complex |
| Distance context based on copy length | Low-Medium | 2-3% | Yes, cheap |
| Recent distance cache | Low | 5-8% | **YES** (same as LZMA rep codes) |

---

## 6. zstd — The Perfect Balance

### Overview
- **Author:** Yann Collet (Facebook/Meta), 2015-present
- **enwik8 ratio:** ~0.35 (level 19), ~0.40 (level 3, default)
- **Compress speed:** 300-500 MB/s (level 1-3), 3-10 MB/s (level 19-22)
- **Decompress speed:** 800-1500 MB/s (ALL levels!)
- **Memory:** 1 MB - 128 MB
- **License:** BSD / GPLv2 dual
- **Adoption:** Linux kernel, HTTP (RFC 8878), databases, everything

### Why zstd Is a Masterpiece

zstd occupies a unique position: **it's competitive with LZMA on ratio while being 10-100x faster to decompress**. Here's how:

### Architecture

#### The Sequence Abstraction

zstd's core representation is the **sequence**: `(literals_length, match_offset, match_length)`. Every block of input is decomposed into a sequence of these tuples:

```
Input: "ABCDEFABCXYZ..."
       ^^^^^^ 3 literals "ABC" + match(offset=6, length=3) for "DEF"→copy "ABC"
       
Encoded as sequences:
  (literals_length=6, offset=0, match_length=0)    // first 6 bytes, no match
  (literals_length=3, offset=6, match_length=3)    // 3 literals then copy 3
  ...
```

This is conceptually identical to LZ77 but the encoding is what makes zstd special.

#### FSE: Finite State Entropy (tANS)

zstd uses **FSE** — Yann Collet's implementation of Jarek Duda's **tANS (table-based Asymmetric Numeral Systems)** — instead of Huffman coding. This is zstd's secret weapon for speed.

**How FSE works:**

Traditional entropy coding:
- Huffman: fast but wastes up to 1 bit per symbol (can't do fractional bits)
- Arithmetic: optimal but slow (requires multiplication/division per symbol)

FSE/tANS achieves arithmetic-coding accuracy at Huffman-coding speed:

1. **Build a distribution table**: Map symbol frequencies to states in a finite state machine
2. **Encode**: Current state + next symbol → new state (single table lookup!)
3. **Decode**: Current state → symbol + previous state (single table lookup!)

The state machine has 2^N states (typically N=9 to 12, so 512-4096 states). Each state encodes exactly one symbol and transitions to a new state. The state itself carries the fractional-bit information that Huffman can't represent.

**Why FSE is fast:**
- Encoding: one table lookup + one addition + conditional bit output
- Decoding: one table lookup + one bit read (branchless!)
- No multiplication or division (unlike arithmetic/range coding)
- Cache-friendly: the entire decode table fits in L1/L2 cache
- Branchless decode loop → perfect for modern CPUs with deep pipelines

**FSE vs Huffman on real data:**
- Huffman wastes ~0.05 bits/symbol on average
- FSE wastes ~0.001 bits/symbol
- On a 100 MB file, this is ~50 KB difference — not huge, but free

#### Sequence Encoding

zstd encodes its three sequence fields (literal_length, match_length, offset) using **interleaved FSE streams**:

1. Three separate FSE tables — one for each field
2. The three streams are interleaved bit-by-bit in a single bitstream
3. Decoder reads them in lockstep

The interleaving is key: it keeps all three FSE states hot in registers, and the decoder loop is extremely tight.

#### Rep Codes (Repeat Offsets)

zstd maintains a **3-slot repeat offset cache** (rep1, rep2, rep3):
- Offset = 1: Use rep1 (most recent offset)
- Offset = 2: Use rep2
- Offset = 3: Use rep3
- Offset > 3: New offset (value = offset - 3)

When a rep code is used, the cache is reordered (LRU-style). This is extremely effective:
- On typical data, 30-50% of matches reuse a recent offset
- A rep code costs ~1-2 bits vs 10-20+ bits for a new offset
- This is the single biggest efficiency gain over basic LZ77

#### Literal Compression

Literals (bytes that don't match) are compressed separately using **Huffman coding** (not FSE — because Huffman decoding of raw bytes is actually faster due to simpler state management):

1. Collect all literal bytes in the block
2. Build a Huffman table
3. Encode literals with the table
4. Table is stored in the block header (compressed with FSE!)

#### Block Structure
```
Frame = Magic + FrameHeader + Block* + Checksum

Block = BlockHeader + LiteralsSection + SequencesSection

LiteralsSection = Huffman table + compressed literals
SequencesSection = FSE tables (×3) + interleaved encoded sequences
```

#### Dictionary Mode

zstd has first-class **dictionary support**:
1. Train a dictionary on sample data: `zstd --train samples/* -o dict`
2. The dictionary captures common byte patterns and their frequencies
3. During compression, the dictionary acts as a pre-filled window
4. Massive gains on small files with shared structure (JSON APIs, log lines, etc.)
5. Dictionary up to 112 KB, stored separately and referenced by ID

On repeated small data (like Redis values or Kafka messages), dictionary mode can improve ratio by **50-80%**.

### Benchmark Numbers
| Corpus | zstd -1 | zstd -3 | zstd -19 | gzip -9 | lzma |
|--------|---------|---------|----------|---------|------|
| enwik8 | 44.1 MB | 40.0 MB | 34.5 MB | 36.4 MB | 29.7 MB |
| Silesia | 75.0 MB | 70.0 MB | 63.5 MB | 76.1 MB | 65.4 MB |
| Compress speed | 500 MB/s | 300 MB/s | 5 MB/s | 30 MB/s | 3 MB/s |
| Decompress speed | 1200 MB/s | 1200 MB/s | 1200 MB/s | 300 MB/s | 50 MB/s |

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| FSE/tANS entropy coding | High | Optimal coding at Huffman speed | **YES** — THE modern entropy coder |
| Sequence abstraction (lit_len, offset, match_len) | Low | Clean architecture | **YES** — good framing |
| Rep codes (3-slot offset cache) | Low | 10-15% on typical data | **YES** — easy, huge win |
| Interleaved FSE streams | High | Decode speed | Later optimization |
| Dictionary training | Medium | 50-80% on small/similar files | Yes, for specific use cases |
| Separate literal compression | Low | Cleaner code, sometimes better ratio | **YES** |
| Huffman for literals, FSE for codes | Medium | Best of both worlds | Refinement stage |

---

## 7. BSC — The BWT Successor

### Overview
- **Author:** Ilya Grebnov
- **Library:** libbsc
- **Silesia ratio:** ~0.29 (better than LZMA!)
- **Speed:** 50-100 MB/s compress, 100-200 MB/s decompress
- **Memory:** 5× block size (typically 25-125 MB)
- **License:** Apache 2.0
- **Key insight:** BWT + context modeling + fast sorting = competitive with LZ-based methods

### Architecture

BSC follows the classic BWT pipeline but modernizes every component:

```
Input → [Block Sorting (BWT)] → [Context Modeling] → [Entropy Coding] → Output
```

#### Stage 1: Burrows-Wheeler Transform (BWT)

The BWT reorders the input to cluster similar contexts together. Quick recap:

1. Form all rotations of the input block
2. Sort them lexicographically
3. Take the last column of the sorted matrix
4. This output has long runs of repeated characters (because similar contexts cluster)

Example: "banana$" →
```
Rotations (sorted):     Last column:
$banana                 a
a$banan                 n
ana$ban                 n
anana$b                 b
banana$                 $
na$bana                 a
nana$ba                 a
                        → "annb$aa"
```

After BWT, the data is highly amenable to run-length-like coding.

BSC uses **libdivsufsort** by Yuta Mori for the BWT — the fastest known suffix array construction algorithm, running in O(n) expected time.

#### Stage 2: Context Modeling (The BSC Innovation)

This is where BSC diverges from bzip2. Where bzip2 uses a simple Move-to-Front (MTF) transform after BWT, BSC uses **full context modeling** on the BWT output:

**Quantized Local Frequency Estimation (QLFC):**
BSC models the BWT output using contexts derived from:
- Previous 1-2 output symbols
- Local frequency statistics (which symbols have appeared recently)
- Run length information

The context model maintains probability estimates for each symbol given the context, similar to PPM (Prediction by Partial Matching) but optimized for BWT output characteristics.

Why this matters: After BWT, the data has a specific statistical structure — long runs of the same character, followed by a different character that's predictable from the sorting context. BSC's context model exploits this structure directly, while bzip2's MTF + Huffman is a crude approximation.

**M03 Variant:**
BSC also has an experimental M03 mode (bsc-m03) that uses Michael Maniscalco's "context-aware" BWT compression:
- After BWT, uses the BWT context (the sorted prefix) to predict the output symbol
- Essentially does PPM on the BWT domain
- Gets even better compression at the cost of speed

#### Stage 3: Entropy Coding

BSC supports multiple entropy coders:
- **Arithmetic coding** — best ratio
- **Quantized Local Frequency Coding (QLFC)** — BSC's custom coder, good balance
- **Range coding** — fast variant

#### Multi-threaded Block Processing
BSC processes blocks in parallel:
- Default block size: 25 MB (configurable up to 2 GB)
- Each block is independently BWT-sorted, context-modeled, and entropy-coded
- Easy to parallelize since blocks are independent
- The BWT sort is the bottleneck, but libdivsufsort is already very fast

### BSC vs bzip2

| Feature | bzip2 | BSC |
|---------|-------|-----|
| BWT sort | O(n²) worst case | O(n) via libdivsufsort |
| Post-BWT | MTF → RLE → Huffman | Context modeling → Arithmetic/QLFC |
| Block size | 900 KB max | 25 MB+ |
| Threads | Single | Multi-threaded |
| Silesia ratio | ~0.35 | ~0.29 |
| Speed | ~5-10 MB/s | ~50-100 MB/s |

BSC is better in **every dimension** — faster, better ratio, larger blocks, multi-threaded.

### Benchmark Numbers
| Corpus | bzip2 | BSC | LZMA | BSC advantage over bzip2 |
|--------|-------|-----|------|--------------------------|
| Silesia | 73.8 MB | 61.5 MB | 65.4 MB | 17% smaller (beats LZMA!) |
| enwik8 | 29.0 MB | 25.5 MB | 29.7 MB | 12% smaller |
| Calgary | 0.86 MB | 0.79 MB | 0.96 MB | 8% smaller |

Note: BSC actually **beats LZMA** on some corpora, especially text-heavy ones. BWT + good context modeling is very competitive.

### Stealable Ideas for Autosqueeze
| Idea | Difficulty | Expected Impact | Worth It? |
|------|-----------|-----------------|-----------|
| BWT as preprocessing before entropy coding | Medium | Much better than raw LZ on text | **YES** — Phase 3 candidate |
| Context modeling on BWT output (not just MTF) | Medium-High | 10-15% over bzip2-style MTF | Yes, if we do BWT path |
| libdivsufsort for O(n) BWT | Low (it's a library) | Fast BWT | **YES** — use it |
| Multi-threaded block processing | Medium | Linear speedup | Yes, later |
| Large block sizes (25 MB+) | Low | Better BWT clustering | **YES** |
| QLFC entropy coding | Medium | Specialized for BWT output | Maybe, arithmetic/ANS may be simpler |

---

## 8. Cross-Compressor Technique Matrix

Which techniques appear in which compressors:

| Technique | cmix | PAQ8 | ZPAQ | LZMA | Brotli | zstd | BSC |
|-----------|------|------|------|------|--------|------|-----|
| LZ77 match finding | — | — | ✓ | ✓✓ | ✓ | ✓ | — |
| BWT | — | — | ✓ | — | — | — | ✓✓ |
| Context mixing | ✓✓ | ✓✓ | ✓✓ | — | — | — | — |
| Huffman coding | — | — | — | — | ✓ | ✓(literals) | — |
| Arithmetic coding | ✓ | ✓ | ✓ | — | — | — | ✓ |
| Range coding | — | — | — | ✓✓ | — | — | opt |
| ANS/FSE | — | — | — | — | — | ✓✓ | — |
| Repeat distance cache | — | — | — | ✓ | ✓ | ✓ | — |
| Static dictionary | — | ✓(text) | — | — | ✓✓ | ✓(trainable) | — |
| Preprocessing transforms | ✓✓ | ✓✓ | ✓ | — | — | — | — |
| SSE/APM refinement | ✓ | ✓✓ | ✓ | — | — | — | — |
| Neural network mixing | ✓✓ | — | — | — | — | — | — |
| Optimal parsing (DP) | — | — | — | ✓✓ | ✓ | ✓ | — |
| Per-field context modeling | ✓ | ✓ | ✓ | ✓✓ | ✓ | — | ✓ |
| Block splitting | — | — | — | — | ✓✓ | ✓ | ✓ |
| Multi-threading | — | — | ✓ | ✓(LZMA2) | — | ✓ | ✓✓ |
| File type detection | ✓ | ✓✓ | — | — | — | — | — |

### Universal Lessons

Every top compressor uses some combination of:
1. **Redundancy elimination** (LZ77 or BWT — find repeated patterns)
2. **Statistical modeling** (predict what comes next)
3. **Entropy coding** (encode predictions efficiently)

The differences are in HOW they do each step and HOW MUCH compute they spend.

---

## 9. What We Should Steal

### Priority 1: Immediate Wins (Phase 1-2)
These are high-impact, lower-difficulty techniques we should implement ASAP:

1. **LZ77 with hash chain match finding** — Replace RLE. Instant ~60% ratio improvement. (from LZMA/zstd/Brotli)

2. **Repeat distance cache (rep codes)** — 3-4 slot cache of recent match distances. 10-15% improvement for ~50 lines of code. (from zstd/LZMA/Brotli)

3. **Huffman coding for entropy stage** — Standard, well-understood, fast. 10-20% on top of LZ77. (from Brotli/zstd/DEFLATE)

4. **Separate encoding for literals vs match codes** — Don't mix them in the same Huffman table. (from zstd)

### Priority 2: Major Improvements (Phase 3-4)
These need more work but are well worth it:

5. **Per-field context modeling** — Different probability models for literal bytes vs match lengths vs match distances. LZMA's biggest insight. 10-15% improvement.

6. **FSE/tANS entropy coding** — Replace Huffman with ANS. Near-optimal compression at Huffman speed. (from zstd)

7. **BWT preprocessing option** — For text-heavy data, BWT + context modeling beats LZ77. Offer both paths. (from BSC)

8. **Static dictionary for known data patterns** — Pre-built dictionary of common byte sequences. 10-30% on small files. (from Brotli)

9. **SSE/APM post-refinement** — After your main predictor, run the prediction through a context-dependent correction table. 2-5% for minimal cost. (from PAQ/cmix)

### Priority 3: Advanced Optimization (Phase 5-6)

10. **Optimal parsing via DP** — Instead of greedy LZ77, use dynamic programming to find the globally best encoding. 5-10% improvement. (from LZMA)

11. **Context mixing with 3-5 models** — Run a few different context models (order-1, order-3, match model) and mix their predictions with logistic weighting. Not 2000 models — just a few well-chosen ones. (simplified PAQ/cmix)

12. **Block splitting with cost estimation** — Detect when data characteristics change and start a new block with fresh statistics. (from Brotli)

13. **Preprocessing transforms** — Dictionary transform for text, delta coding for images, x86 filter for executables. (from cmix/PAQ)

### Priority 4: Moonshot Ideas (Phase 7)

14. **LSTM or small neural mixer** — Use a tiny neural network (not 2077 models, maybe 5-10) to mix predictions. This is the cmix innovation in miniature.

15. **Trainable dictionaries** — Like zstd's dictionary training, but for our specific corpus.

16. **Hybrid architecture** — BWT path for text blocks, LZ77 path for binary/mixed, auto-detect and switch.

### The Key Insight

The fundamental lesson from all these compressors:

> **Compression = Prediction + Entropy Coding**

The better you predict the next byte, the less entropy remains, and the less the entropy coder has to output. Every innovation in this survey is either:
- A better way to **predict** (context mixing, match models, dictionary, BWT clustering)
- A better way to **encode the prediction** (ANS/FSE, range coding, arithmetic coding)

Our autosqueeze roadmap should follow this dual path: improve prediction AND improve coding, in parallel.

---

*Survey compiled March 2026. Sources: byronknoll.com/cmix, mattmahoney.net/dc, Wikipedia, RFC 7932 (Brotli), RFC 8878 (zstd), github.com/facebook/zstd, github.com/IlyaGrebnov/libbsc, handwiki.org LZMA article, various compression benchmarks.*

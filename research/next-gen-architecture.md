# Next-Generation Compression Architecture for `autosqueeze`

## Executive summary

The current `compress.rs` is a pragmatic hybrid: a 1 MiB-window LZ77 path with hash chains, short iterative optimal parsing, per-block static prefix coding, and a fallback BWT/MTF/RLE/range-coding path for smaller files. It is a solid step past the original RLE baseline, but it still lives in the broad `gzip → bzip2` neighborhood. A corpus-wide ratio target of **0.15** is far beyond what plain LZ77, plain BWT, or lightly adaptive entropy coding will reliably deliver.

To get anywhere near **0.15**, the compressor needs to move from “find repeats and code them better” into **aggressive preprocessing + grammar/replication modeling + context-mixed entropy coding**. In practice, that means:

1. **Split the file into typed regions** rather than forcing one pipeline.
2. **Use a stronger long-match and repetition engine** than the current hash-chain LZ77.
3. **Replace symbol-by-symbol static codebooks with adaptive bitwise arithmetic coding** driven by multiple context models.
4. **Add a second stage for structured redundancy**: words/tokens, columnar fields, line templates, numeric transforms, and dictionary references.
5. **Encode the resulting event stream with a lightweight PAQ-style mixer**, not a single flat model.

My recommendation is a **single-file pure-Rust compressor with a typed preprocessing front-end, LZ/phrase modeling middle layer, and binary arithmetic coder back-end**. Think of it as:

**segmentation + structural transforms + LZ-rep parser + token/byte context mixing + binary arithmetic coding**

This is the only architecture here with a plausible path toward a ratio around **0.15 on a favorable text-heavy / structured corpus** while still decompressing in reasonable time. It will not be cmix-level ratio on arbitrary data, but it can get materially closer than the current design.

---

## 1. Overall architecture

## High-level pipeline

### Stage A — Global analysis and block typing
Process the input in **independent superblocks** of **4 MiB** by default, with optional 8 MiB for high-compression mode.

For each superblock:

1. Compute cheap statistics:
   - byte histogram
   - UTF-8 plausibility
   - line-length distribution
   - delimiter frequencies (`\n`, `,`, `\t`, `:`, `{}`, `[]`, `"`)
   - zero/high-bit density
   - repeated-line / repeated-word rate
   - match-density estimate from a fast 4-byte hash sampler
2. Classify the block into one of a few coding families:
   - **TEXT**: English/source/markup/JSON/log-like
   - **STRUCTURED TEXT**: CSV/TSV/NDJSON/repeated records
   - **BINARY REPETITIVE**: executable/data blobs with many repeats
   - **MIXED / RAW**: fallback
3. Emit a compact block header describing:
   - block type
   - transforms selected
   - local dictionary mode
   - entropy model variant

This classification matters because the target ratio is impossible if every byte is treated as generic anonymous data.

### Stage B — Preprocessing / normalization
Apply a transform stack chosen per block. Not every block gets every transform.

Candidate transform order:

1. **Record splitter / line model**
2. **UTF-8 / text tokenization**
3. **Structured-field splitter**
4. **Numeric normalization / delta / varint reshaping**
5. **Case / prefix / suffix factorization**
6. **LZ-rep phrase parsing** on the transformed stream
7. **Residual byte/event stream** into context mixer + arithmetic coder

The output of preprocessing is not just bytes. It is a stream of **events**:

- literal byte
- token-dictionary reference
- previous-token reference
- line-template reference
- LZ match
- repeat-distance match
- small integer / delta-coded number
- raw escape / fallback

### Stage C — Modeling
Use a **hierarchical mixed model**:

1. **Event model**: predicts whether next symbol is literal, match, token ref, line ref, number, etc.
2. **Submodels per event type**:
   - literal-byte models
   - match-length model
   - match-distance model
   - token-id model
   - line-template model
   - numeric residual model
3. **Bitwise context mixer** combines predictions from multiple submodels.
4. **Binary arithmetic coder** encodes the final bits.

### Stage D — Entropy coding
Use a **binary arithmetic range coder** with 12–16 bit probability precision and adaptive probabilities updated after every bit.

Reason: the architecture needs many overlapping contexts and mixed probabilities. That fits arithmetic coding naturally. rANS is great for tabled symbol models, but this design is centered on **adaptive bitwise prediction**, where arithmetic coding is the right tool.

### Stage E — Decompression path
Decompression reverses the transforms in this order:

1. decode bits via arithmetic coder
2. reconstruct event stream
3. rebuild dictionaries/templates/matches
4. invert numeric transforms
5. reassemble fields/records
6. emit original bytes

This stays reasonable because the heavy work is in modeling, not in superlinear inverse transforms like naive BWT sorting.

---

## 2. Context model design

## Design philosophy

A 0.15-class compressor needs **multiple specialized weak predictors** combined into a stronger predictor. The right model is not one giant neural net; it is a small **PAQ-style mixer** built from deterministic contexts.

## Top-level model families

Use roughly **14–18 submodels**, grouped into the following families.

### A. Generic byte-context models
These operate on the residual literal stream.

1. **Order-0 byte model**
   - context: none
   - purpose: baseline byte frequencies

2. **Order-1 byte model**
   - context: previous byte

3. **Order-2 byte model**
   - context: previous 2 bytes

4. **Order-3 byte model**
   - context: previous 3 bytes

5. **Order-4 hashed byte model**
   - context: previous 4 bytes hashed into a table

6. **Sparse long context model**
   - context: bytes at positions `-1, -2, -3, -5, -8, -13`
   - purpose: captures patterned text and some binary structure without giant tables

These are only active when the current event is a literal byte.

### B. Word/text models
For text-like blocks, literal bytes are better predicted through lexical structure.

7. **Character class model**
   - contexts over class sequence: lower/upper/digit/space/punct/newline/other
   - predicts class transitions and narrows literal distribution

8. **Word-prefix model**
   - during alphabetic runs, context is current word prefix length 1–4 plus previous delimiter
   - predicts next letter strongly on natural language and identifiers

9. **Previous-word / token-follow model**
   - hash of previous whole token or previous 2 tokens
   - predicts next token type or first byte of next token

10. **Line-position model**
   - context: column modulo small power-of-two plus previous separator class
   - works very well on aligned logs, source code indentation, CSV-like text

### C. Structured record models
For repeated records / delimited text.

11. **Field-index model**
   - context: record field number, prior delimiter, current quote state
   - separate predictors per field position

12. **Previous-record same-field model**
   - predicts current field from same field in previous row
   - especially strong for timestamps, IDs with small increments, repeated hostnames/statuses

13. **Template model**
   - context: line skeleton where literals are replaced with placeholders (`[WORD]`, `[NUM]`, `[HEX]`, etc.)
   - if a line matches a known skeleton, encode template ID + residual slots

### D. Match / repetition models
These predict LZ-like events, not bytes.

14. **Event-type model**
   - predicts literal vs short rep vs long rep vs dictionary token vs template vs number
   - context: previous 2 event types, recent match success, local entropy bucket

15. **Repeat-distance model**
   - special handling for last 4 distances, like LZMA/ROLZ-style reps
   - contexts: rep-slot rank, prior event type, local block type

16. **Match-length model**
   - contexts: distance bucket, whether distance is recent, preceding byte class

### E. Numeric models
17. **Number-shape model**
   - decimal vs hex vs signed vs timestamp-like
   - predicts digit count, punctuation pattern, and whether delta coding applies

18. **Numeric residual bit model**
   - for numbers transformed to delta or XOR-from-previous-in-field, encode residual bits MSB-first with contexts from bit position and high bits already decoded

---

## How the models are mixed

Use a **2-level logistic-like mixer**, implemented without floating point if desired.

### Level 1: per-family mixers
Each family combines its own internal predictions into one probability. Example:

- byte family combines order-0/1/2/3/4/sparse
- text family combines class/word-prefix/token-follow/line-position
- structure family combines field/template/previous-record
- match family combines event-type/repeat-distance/match-length
- numeric family combines shape/residual

### Level 2: global mixer
A top-level mixer combines the family outputs based on coarse context:

- block type
- current event type
- current byte bit position
- whether inside word / number / quoted string / whitespace run
- recent coding cost bucket

### Mixer implementation
Use small fixed-point weights.

Recommended implementation:

- prediction scale: 0..4095
- weights: i16
- update by a clipped gradient step after each bit
- 8–16 contexts for the second-level mixer keyed by block-type/state

This gives most of the gain of classic PAQ mixing without exploding complexity.

### Probability state tables
For many submodels, use a compact **state machine table** instead of raw counters:

- state encodes approximate `(n0, n1)` or equivalent skew/confidence
- each context cell stores one `u8` or `u16` state
- update through a precomputed transition table

This is important for keeping the whole thing inside a single-file pure-Rust implementation with sane memory.

---

## 3. Entropy coder specification

## Recommendation: binary arithmetic range coder

Use a **bitwise arithmetic range coder**, not rANS.

### Why not rANS?

rANS/FSE is excellent when:
- symbols are encoded from one of a small number of finite alphabets
- probabilities are static or quasi-static per block
- you want speed and simple table lookups

But this proposed architecture relies on:
- many overlapping contexts
- online adaptation
- event-specific bit models
- mixed predictions every bit

That is exactly where arithmetic coding wins.

## Coder structure

### Core coder
- 64-bit `low`
- 32-bit or 64-bit `range`
- renormalize by bytes
- probability represented as integer `p1` in `[1, 4095]` or `[1, 65535]`
- encode one bit at a time

### Bit order
For multi-bit integers:
- encode **MSB-first**
- contexts can condition on already decoded high bits
- especially valuable for lengths, distances, token IDs, and numeric residuals

### Event coding structure
Encode in this order:

1. event type
2. if literal:
   - byte bits with literal models
3. if match:
   - rep-vs-new-match flag
   - length bits
   - distance slot
   - extra distance bits
4. if token ref:
   - token class
   - token ID or MTF rank
5. if template line:
   - template ID
   - slot count
   - slot payloads
6. if number:
   - number mode
   - delta/XOR/raw flag
   - residual bits

### Termination
Per block:
- explicit end-of-block event
- optional checksum only in debug builds or format version if desired

### Decoder speed expectations
This will be slower than Huffman or rANS but still **reasonable** if designed carefully:
- decompression should be comfortably faster than compression
- expected practical class: slower than zstd, much faster than cmix/paq
- acceptable target for first serious version: **10–40 MB/s decompression on text-heavy corpora** on modern CPUs

That is “reasonable time” for a research compressor targeting extreme ratio.

---

## 4. Match finder design

## Recommendation: hybrid multi-match engine

Do **not** use a single plain hash chain as the main engine. The current design’s 1 MiB window + 512 chain cap is decent but leaves a lot of ratio behind.

Use a hybrid of:

1. **4–8 byte hash tables** for immediate candidates
2. **Binary-tree match finder** for long matches in a large sliding window
3. **Repeat-distance cache** for recent offsets
4. **Optional phrase dictionary / secondary window** for block-global repeated substrings

## Primary window
- size: **8 MiB** default high-compression mode
- optionally 4 MiB for balanced mode

8 MiB is large enough to capture repeated sections, code fragments, logs, and templated text that 1 MiB misses, without becoming insane.

## Binary-tree finder

### Why binary tree?
Compared with hash chains:
- much better long-match discovery
- better candidate ordering
- easier to cap work deterministically
- standard choice for high-ratio LZMA-style parsing

### Data structure
Maintain for each position:
- left child index
- right child index
- optional depth cap
- hash heads for fast entry into the tree

Each new position is inserted into the binary tree keyed by lexicographic suffix comparison over the window.

### Search policy
At each position:
- probe recent rep distances first
- probe 4-byte and 8-byte hash candidates
- descend binary tree with comparison budget
- keep top `N` candidates by estimated coding gain, not by raw length only

Recommended limits:
- balanced mode: 32–64 tree comparisons
- max mode: 128–256 tree comparisons

## Secondary phrase dictionary
For text/structured blocks, build a **local phrase table** during a forward scan:
- repeated words
- repeated key strings
- common separators with context
- repeated line fragments of 8–64 bytes

This is not a full suffix array. It is a compact hash-indexed phrase dictionary used as an extra match source.

## Why not suffix array?
Suffix arrays are great for offline global search, but for a streaming single-file compressor they are less attractive here because:
- building them per large block is expensive
- memory overhead is substantial
- they complicate incremental parsing
- decompressor does not benefit directly

A suffix array is justified for BWT; it is not the right central match finder for this design.

## Parsing strategy
Use **full optimal parsing with lazy candidate pruning**, not greedy or 2-pass approximate DP.

At each byte position, consider:
- literal
- short reps (`rep0..rep3`)
- short match from local hash
- long match from tree
- dictionary phrase
- template event if line mode fires
- token event if tokenizer fired

Then compute dynamic-programming cost using current model-estimated prices.

For tractability:
- cap active candidates per position to 12–20
- quantize prices to fixed-point integers
- process per block, not whole file

This is a major ratio lever.

---

## 5. Preprocessing pipeline

This is the part most likely to unlock 0.15 on the right corpus.

## Guiding principle

Do not merely transform bytes; transform the data into **predictable structure** while preserving exact reversibility.

## Pipeline by block type

### A. TEXT blocks

#### Step 1: UTF-8 and lexical scan
Split into token classes:
- alphabetic word
- number
- whitespace run
- punctuation / delimiter
- mixed identifier
- quoted string body

#### Step 2: adaptive token dictionary
Maintain a block-local dictionary of common tokens:
- words length 3–32
- identifiers
- JSON keys / source keywords / repeated substrings

Encode tokens via:
- recent-token MTF rank if very recent
- dictionary ID if stable and common
- raw literal fallback otherwise

This alone can dramatically reduce text entropy because common words become small IDs instead of byte sequences.

#### Step 3: case factorization
For alphabetic tokens:
- store lowercase canonical form in dictionary
- encode case pattern separately:
  - all lower
  - first upper
  - all upper
  - mixed fallback bitmap

#### Step 4: prefix/suffix reuse
For identifiers and natural language variants:
- detect shared prefixes/suffixes against recent tokens
- examples: `compress`, `compression`, `compressor`
- encode as base-token ref + suffix literal when profitable

#### Step 5: line template extraction
For newline-rich text:
- derive skeletons by replacing variable spans with placeholders
- e.g. `2026-03-15 INFO user=brett action=login` becomes template with slots for date/user/action
- reuse template IDs for repeated log lines / records

### B. STRUCTURED TEXT blocks
This is where ratios can collapse hard in a good way.

#### Step 1: row/field detection
Detect separators (`\n`, `,`, `\t`, `|`) and quoting.

#### Step 2: columnar split
Within a block, transpose records into fields:
- field 0 stream
- field 1 stream
- field 2 stream
- etc.

Then compress each field stream with specialized models.

Why this matters: CSV/TSV/NDJSON often has high repetition by column, not by row.

#### Step 3: field-specific normalization
Per field, test:
- constant-string dictionary
- previous-row copy / prefix-share
- numeric delta
- timestamp decomposition
- hex/base64 mode

#### Step 4: residual merge
Encode field events and a lightweight record map so decompressor can reconstruct exact row order.

### C. BINARY REPETITIVE blocks

#### Step 1: word-size probing
Test 2-, 4-, and 8-byte stride correlations.

#### Step 2: delta/XOR filters
For each stride candidate, cheaply estimate if any of these reduce entropy:
- bytewise delta
- little-endian word delta
- XOR with previous word

#### Step 3: choose filtered or raw stream
If a filter helps, feed filtered bytes to LZ/event coder; else fallback.

### D. MIXED blocks
Use minimal transforms:
- long-match engine
- rep distances
- generic literal context mixing
- no heavy tokenization unless benefit is clear

## Important transforms to include

### 1. Repeat-distance cache
Keep `rep0..rep3` like LZMA. Very cheap, very effective.

### 2. Small static dictionary bootstrap
Embed a tiny built-in dictionary for common text fragments:
- JSON punctuation patterns
- common English affixes
- markup fragments
- source-code keywords

Since no external crates and single-file only, the dictionary can be hardcoded as a compact byte blob.

### 3. Numeric normalization
Recognize numbers and encode them as:
- raw integer
- delta from previous in same field
- XOR from previous
- timestamp decomposition (date part, time part, zone)
- hex nybble stream for hex-like tokens

### 4. Whitespace/run modeling
For text and code, whitespace carries strong structure.
Encode runs of:
- spaces
- tabs
- newlines
separately from generic literals.

### 5. Quoted string mode
Inside JSON/string literals or source strings, switch contexts because escape patterns and character sets differ.

## What not to do

- Do not rely on naive BWT for large blocks; the current O(n log² n)-ish approach is not viable at the level needed.
- Do not use generic MTF over the entire file as a main strategy.
- Do not add dozens of transforms with tiny marginal wins and huge branch cost. Keep the transform set small but high-impact.

---

## 6. Memory budget and allocation strategy

## Target memory budget

Recommended high-compression memory target:
- **Compressor:** ~96–160 MiB
- **Decompressor:** ~16–40 MiB

That is entirely reasonable in 2026 for an extreme-ratio research compressor.

## Proposed memory layout per 4 MiB block

### Core block buffers
- input block: 4 MiB
- transformed event/literal scratch: 4–8 MiB worst case
- output buffer grows separately

### Match finder
For 8 MiB sliding history in max mode:
- history buffer: 8 MiB
- binary-tree left/right arrays: `2 * 8 Mi positions * 4 bytes` would be too large if literal per-byte history is stored globally, so use block/window-relative indexing and cap active window
- practical target:
  - ring buffer for bytes: 8 MiB
  - left child array: 8 Mi entries × 4 bytes = 32 MiB
  - right child array: 32 MiB
  - hash heads and auxiliaries: 4–8 MiB
- total match finder working set: **~76–80 MiB** in max mode

Balanced mode can halve this with a 4 MiB window.

### Context model tables
Use compact hashed tables, not giant explicit context maps.

Suggested allocation:
- literal byte models: 8–16 MiB
- text/field/template models: 4–8 MiB
- numeric and event models: 2–4 MiB
- mixer weights/states: <1 MiB

Total modeling memory: **~16–24 MiB**

### Dictionaries and templates
- token dictionary storage: 1–4 MiB
- line templates / field schemas: 1–2 MiB

### Total
Compressor max mode:
- match finding: 80 MiB
- model tables: 20 MiB
- scratch/dictionaries: 12 MiB
- input/output scratch: 12–24 MiB
- total practical peak: **124–148 MiB**

Decompressor:
- history buffer: 8 MiB
- context tables: 8–16 MiB
- dictionary/template state: 2–8 MiB
- total: **18–32 MiB** typical

## Allocation strategy

### 1. Preallocate once per mode
Avoid repeated `Vec` growth in hot loops.
Use large reusable buffers allocated at startup of compression/decompression.

### 2. Ring-buffer everything possible
- history bytes
- token recency lists
- recent line templates
- recent match candidates

### 3. Separate hot and cold memory
Hot arrays:
- probability states
- history bytes
- child pointers
- recent distances

Cold structures:
- dictionary strings
- block metadata
- template definitions

### 4. Use fixed-size arenas
For token dictionary and template storage, use append-only arenas with periodic reset per block. Avoid per-token heap churn.

---

## 7. Estimated per-file ratios with justification

These estimates are necessarily conditional, because the corpus was not provided. The right framing is: **what this architecture should do on likely data classes compared with the current compressor**.

## Current design expectation
From the code, the current compressor should roughly land in these regions:
- repetitive text: maybe **0.22–0.40** if lucky
- generic text/code: **0.30–0.50**
- structured records/logs: **0.25–0.45**
- mixed binaries: **0.45–0.95**

It has no serious context mixing, no token model, no column model, limited match search, and a weak BWT path. So 0.15 corpus-wide is very unlikely.

## Next-gen architecture estimates

### 1. Natural-language text / source code / markup
Expected ratio: **0.16–0.24**

Justification:
- token dictionary collapses frequent words/identifiers
- case factoring removes redundant capitalization entropy
- line-position and lexical contexts are strong
- LZ + rep distances handle repeated phrases/boilerplate
- arithmetic coding with context mixing beats static block coding

If the corpus is heavily English-like and repetitive, **~0.15–0.19** is plausible.

### 2. Logs / NDJSON / CSV / structured records
Expected ratio: **0.08–0.18**

Justification:
- template extraction destroys repeated syntactic scaffolding
- fieldwise splitting makes columns highly predictable
- timestamps and IDs shrink under delta/XOR transforms
- repeated keys/values collapse into dictionary and line-template references

This is the strongest case for the 0.15 goal. On a corpus dominated by such data, **0.15 overall is realistic**.

### 3. JSON with repeated keys and semi-structured values
Expected ratio: **0.10–0.20**

Justification:
- keys become dictionary IDs
- punctuation/quotes become template structure
- numeric and string fields benefit from field-local modeling

### 4. Executables / mixed binary blobs / compressed assets
Expected ratio: **0.35–0.85**

Justification:
- long-match finder helps somewhat
- some delta/XOR filters may help on tables
- but already-compressed or high-entropy sections will not bend much

No architecture without domain-specific transforms is going to push these near 0.15.

### 5. Entire corpus estimate
If the corpus is mostly text, code, logs, JSON, CSV, and similar structured content:
- conservative: **0.17–0.22**
- aggressive but credible: **0.14–0.18**

If the corpus contains a meaningful fraction of binaries/media/already-compressed payloads:
- more realistic: **0.20–0.30**

## Bottom line on 0.15

**0.15 is plausible only if the corpus is very favorable**: text-heavy, repetitive, structured, and not already entropy-flattened.

If the corpus includes lots of random-looking or already-compressed data, then 0.15 becomes fantasy without cmix-class complexity and slowness.

So my honest take:
- **On text/structured corpora:** yes, this architecture gives a real shot.
- **On arbitrary mixed corpora:** no, not without drifting much closer to PAQ/cmix territory.

---

## 8. Implementation plan with phases

The build should be staged so each phase yields a measurable win and preserves a clean decompression path.

## Phase 0 — Instrumentation and benchmark harness
Before changing the algorithm, add measurement discipline.

Deliverables:
- corpus benchmark script / methodology
- per-file ratio table
- speed measurements for compress/decompress
- block-type statistics
- token/match/template hit rates

Goal:
- know exactly where current ratio comes from
- know which file classes dominate corpus size

## Phase 1 — Replace current LZ core with a serious parser

### Work
- increase window to 4–8 MiB
- add repeat-distance cache (`rep0..rep3`)
- replace hash chains with hybrid hash + binary tree
- move from 2-iteration approximate DP to real blockwise optimal parsing
- refine match price model

### Expected gain
- **10–25% relative improvement** over current LZ path on repetitive text/code/logs

### Why first
This improves everything else and does not require format redesign beyond token stream changes.

## Phase 2 — Switch entropy backend to binary arithmetic coding

### Work
- replace static per-block prefix/range symbol coding with bitwise arithmetic coder
- add event-type model, literal byte contexts, length/distance models
- keep transforms minimal initially

### Expected gain
- **8–18% relative improvement** over Phase 1, especially on literals and mixed events

### Outcome
At this point the compressor becomes a true predictive coder rather than a better deflate variant.

## Phase 3 — Add text/token front-end

### Work
- lexical scanner for text-like blocks
- adaptive token dictionary
- case modeling
- whitespace-run and quoted-string modes
- previous-token and line-position contexts

### Expected gain
- **15–35% relative improvement** on text/code/markup-heavy files
- little effect on binary blocks

### Risk
Need careful fallback to raw bytes when tokenization is not profitable.

## Phase 4 — Add structured-record mode

### Work
- detect line/field-delimited blocks
- template extraction for repeated record skeletons
- per-field modeling
- previous-row/same-field prediction
- numeric normalization for fields

### Expected gain
- **20–50% relative improvement** on logs/CSV/NDJSON/structured text
- likely the single biggest lever toward 0.15 if the corpus contains such data

## Phase 5 — Add numeric and phrase transforms

### Work
- detect decimal/hex/timestamp patterns
- delta/XOR residual coding
- block-local phrase dictionary for repeated fragments not captured by LZ well
- prefix/suffix token factorization

### Expected gain
- moderate but high-value on structured and semistructured text

## Phase 6 — Add second-level context mixer

### Work
- family-level predictors
- top-level adaptive mixer keyed by state
- compact state tables and update schedules

### Expected gain
- **5–15% relative improvement** if the lower-level models are already good
- less dramatic alone, but essential for squeezing the tail

## Phase 7 — Prune and harden

### Work
- remove transforms that do not pay rent
- retune block size and table sizes
- optimize decompression fast path
- add format versioning, robustness checks, fuzz testing

### Deliverable
A research-grade compressor that is still maintainable in a single Rust file.

---

## Concrete format proposal

To make the spec actionable, here is a minimal file-format structure.

## File header
- magic: 4 bytes
- version: 1 byte
- mode flags: 1 byte
- original size: varint
- number of blocks: varint

## Per-block header
- block uncompressed size: varint
- block type: 2 bits
- transform flags: bitfield
- model profile ID: small integer
- optional dictionary/template metadata sizes: varints

## Block payload
Arithmetic-coded event stream with explicit end-of-block event.

### Event alphabet
- `LITERAL`
- `MATCH_NEW`
- `MATCH_REP0`
- `MATCH_REP1`
- `MATCH_REP2`
- `MATCH_REP3`
- `TOKEN_REF`
- `TEMPLATE_REF`
- `NUMBER`
- `RAW_RUN`
- `EOB`

Each event then encodes its payload fields bitwise under event-specific contexts.

---

## Why this is the right next architecture for `autosqueeze`

Because it directly addresses the current design’s bottlenecks:

### Current bottlenecks in `compress.rs`
1. **Match finding is too shallow** for high ratio.
2. **Token stream is too primitive**: just literals and matches.
3. **Entropy coding is not context-rich enough**.
4. **No structural understanding** of text/records/numbers.
5. **BWT path is too expensive and too weakly modeled** to be the answer.

### Proposed architecture fixes
1. stronger match discovery and parsing
2. richer event alphabet
3. context-mixed arithmetic coding
4. domain-aware reversible transforms for text and structure
5. no dependence on slow naive BWT

---

## Final recommendation

If the goal is a **real shot at 0.15**, do **not** spend more time polishing the current BWT or static-block-LZ path. That road tops out too early.

Build the next version around this core:

- **4–8 MiB typed blocks**
- **hybrid binary-tree LZ parser with rep distances**
- **token/record/numeric preprocessing for text-heavy data**
- **PAQ-lite context mixing**
- **binary arithmetic coding**

That architecture is still compatible with the stated constraints:
- pure Rust
- no external crates
- single-file compressor
- lossless
- reasonable decompression speed

And unlike a pure cmix clone, it has a path to being both strong and usable.

## Practical expectation

If the corpus is mostly structured/textual, this design can plausibly push `autosqueeze` from “solid custom compressor” into the **0.14–0.20** band, with **0.15** achievable on a favorable dataset.

If the corpus is broad and messy, 0.15 is probably too aggressive — but this is still the architecture most likely to get you closest without becoming completely impractical.

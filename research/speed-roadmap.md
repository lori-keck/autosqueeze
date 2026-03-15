# Autosqueeze Speed Optimization Roadmap

## Executive summary

At **0.1 MB/s**, the compressor is not losing to gzip because of one bad constant factor. It is slow because it does several **algorithmically expensive full-input passes**, and some of those passes are outright **superlinear-to-quadratic**.

The dominant cost centers in `src/compress.rs` are, in order:

1. **BWT suffix/rotation sort** in `bwt_forward()` — effectively **O(n² log n)** or worse in practice because the comparator may scan up to `n` bytes for each comparison and `sort_unstable_by` performs `O(n log n)` comparisons.
2. **LZ match finding** in `HashChain::find_matches()` — up to **512 candidates per position**, each with a byte-by-byte extension loop up to 258 bytes.
3. **Optimal DP parsing** in `lz77_tokenize()` — two full backward DP passes over all positions, iterating every stored match, plus additional sublength probing.
4. **Repeated block re-encoding work** in `lz77_compress()` / `encode_block()` / `estimate_block_size_mode()` — the token stream is encoded multiple times for block-size trials and three literal transforms.
5. **Range coder model maintenance** in `RcModel::update()` / decoder binary search — not the top bottleneck versus the above, but still expensive and very branchy.

The fastest practical route is **not** micro-optimizing the current code first. The big win is:

- **Stop running BWT in the hot path unless explicitly requested**
- Replace the current match finder + full optimal parser with a **fast match finder + bounded/lazy parser**
- Stop recomputing whole-block estimates and whole-stream re-encodes repeatedly

If done pragmatically, a realistic target is:

- **Phase 1 target:** **1–3 MB/s** with moderate effort
- **Phase 2 target:** **5–15 MB/s** with good engineering choices
- **Phase 3 target:** **20+ MB/s** possible if you accept more gzip/zstd-like speed tradeoffs and use high-performance suffix array / match-finder libraries
- **50 MB/s** is possible only if you largely stop doing expensive optimal parsing / BWT-style work and move toward a much faster LZ frontend and simpler entropy coding

For this codebase as written, **10 MB/s is a solid practical medium-term target**. It is ambitious but believable. **50 MB/s** would likely require changing the compressor’s identity, not just tuning it.

---

## 1. Where does all the time go?

## Mental profile of the hot path

### A. `bwt_forward()` is catastrophic

```rust
indices.sort_unstable_by(|&a, &b| {
    let mut i = 0;
    while i < n {
        let ca = data[(a + i) % n];
        let cb = data[(b + i) % n];
        if ca != cb { return ca.cmp(&cb); }
        i += 1;
    }
    Ordering::Equal
});
```

This is the single worst thing in the file.

Why:
- Sorting `n` rotations costs about **O(n log n)** comparisons.
- Each comparison may inspect **up to O(n)** bytes.
- Therefore practical complexity is roughly **O(n² log n)**, with awful cache behavior from modulo-wrapped byte access.
- On repetitive data, comparisons become especially long, making it even worse.

This means the BWT path is not just “a little slow,” it is structurally incompatible with competitive throughput at multi-megabyte block sizes.

### B. `HashChain::find_matches()` is the next major sink

For every position:
- Look up hash head
- Chase up to `HASH_CHAIN_LIMIT = 512` links
- For each candidate, extend the match byte-by-byte up to `MAX_MATCH = 258`

Worst-case work per position is huge:
- **512 candidate checks**
- each with potentially **hundreds of byte comparisons**

That makes the match finder roughly:
- practical complexity around **O(n * chain_limit * avg_match_probe_length)**
- and on repetitive inputs, `avg_match_probe_length` gets ugly fast

Also, **all matches are materialized up front**:

```rust
let mut all_matches: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
for pos in 0..n {
    all_matches[pos] = chain.find_matches(input, pos);
    chain.insert(input, pos);
}
```

So you pay the full match-finding cost for the entire input before parsing even starts, and you allocate a nested vector per byte position.

### C. `lz77_tokenize()` DP is expensive even after match finding

The parser does:
- **2 full backward DP iterations**
- for each position, iterate all matches at that position
- for each match, also test a list of selected shorter lengths

Hot inner loop:

```rust
for &(len, off) in &all_matches[pos] {
    ...
    if mc < cost[pos] { ... }
    for &sl in &[3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,67,83,99,115,131] {
        if sl >= MIN_MATCH && sl < len {
            ...
        }
    }
}
```

That means parser complexity is roughly:
- **O(n * matches_per_pos * sublength_trials * iterations)**

Even if matches per position is moderate, this adds up hard. Since `all_matches` is already expensive to produce, the DP compounds the problem.

### D. `encode_block()` work is repeated more than necessary

Current LZ path does:
- tokenize once
- try **4 block sizes**
- each block tries **3 literal transforms** using `estimate_block_size_mode()`
- then the winning mode is encoded

So a lot of the token stream is walked multiple times just to estimate sizes.

In particular:
- `estimate_block_size_mode()` builds histograms and code lengths for each mode
- `encode_block()` rebuilds them again for the chosen mode
- `lz77_compress()` recompresses the entire token stream for several block sizes

This is not the biggest bottleneck against the current BWT and DP issues, but once those are fixed, this repeated work will start to matter.

### E. Range coder cost is real but secondary

The range coder has several speed issues:
- `RcModel::update()` increments the cumulative array tail from `sym+1..=nsym` on every symbol
- decoder uses binary search over cumulative frequencies per symbol
- frequent renormalization with branches
- tiny model but lots of serial dependencies

Still, compared with quadratic BWT and huge match-finder/DP cost, the range coder is **not** the reason you are at 0.1 MB/s.

## Dominance ranking

For small-to-mid inputs where BWT is enabled:
1. **`bwt_forward()` absolutely dominates**
2. `HashChain::find_matches()`
3. DP in `lz77_tokenize()`
4. repeated block estimation / block-size retries
5. range coder + Huffman table construction

For large inputs where BWT is skipped:
1. `HashChain::find_matches()`
2. DP in `lz77_tokenize()`
3. repeated block estimation / retries
4. entropy coding details

---

## 2. BWT suffix sort: fastest known algorithms

Your current BWT is sorting **cyclic rotations** by comparator. That is not viable.

## What fast BWT implementations do

They usually compute a **suffix array** (or closely related transform) using a specialized algorithm, then derive BWT from it.

### Best practical options

#### A. **libsais**
- Modern, very fast suffix array / BWT construction library
- Linear-time SA-IS family approach in practice
- Excellent practical performance
- Strong candidate for best mix of **speed + implementation risk** if you can use FFI

#### B. **divsufsort / libdivsufsort**
- Classic, battle-tested
- Widely used for suffix array and BWT generation
- Very strong practical speed, often the default answer for production BWT tooling

#### C. **SA-IS**
- Theoretical **O(n)** suffix array construction
- Great algorithmic answer
- Good if you want an in-Rust implementation eventually
- But a naïve homegrown SA-IS is still a lot of engineering and can underperform tuned C libraries

## Recommendation

### If the goal is practical speed ASAP:
1. **Use libsais or libdivsufsort via FFI**
2. Shrink BWT block size if necessary
3. Or disable BWT by default and make it a “slow/high-ratio mode”

### If the goal is pure-Rust eventually:
- prototype with an external library first
- only later replace with a native Rust suffix array if needed

## Important strategic point

Even a fast suffix sorter does **not** make BWT the right default for a general-purpose fast compressor.

BWT can be decent for ratio on text-like data, but for throughput-sensitive compression, a modern LZ frontend is usually a better default. So the right move is probably:
- **BWT as optional slow mode**, not automatic competition on every eligible input.

---

## 3. Can the DP parser be faster?

Yes. A lot faster.

The current parser is close to “small-scale optimal parse by brute force over many candidate matches.” That is good for experimentation, bad for speed.

## Main problems

1. It needs `all_matches[pos]` for every position
2. It evaluates every candidate match at every position
3. It repeats the parse twice for cost convergence
4. It probes many sublengths per match

## Proven faster alternatives

### A. Lazy / near-greedy parsing
Used by gzip-style compressors and many fast LZ coders.

Idea:
- find best match at current position
- maybe compare with next position (“lazy matching”)
- choose quickly instead of full DP

Complexity drops massively.

Tradeoff:
- ratio usually a bit worse than optimal parsing
- but throughput improvement can be enormous

This is the lowest-effort big win.

### B. Bounded optimal parsing
Instead of full global DP with every match:
- limit number of candidates per position
- limit maximum considered match lengths / sublength variants
- limit lookahead horizon
- prune dominated states aggressively

This preserves much of the “smart parse” behavior while reducing runtime a lot.

### C. LZMA-style binary tree match finder
LZMA does not brute-force a 512-link chain at every position. It uses stronger data structures to find good long matches faster.

Benefits:
- fewer candidate checks than long hash chains on many inputs
- good match quality
- still compatible with optimal / semi-optimal parsing

Tradeoff:
- more engineering complexity
- more state maintenance

### D. HC4 / hash-chain variants with multiple hashes
Zstd/LZ4-family techniques use better-structured hash probing and bounded candidate search.

HC4 specifically is a useful reference point:
- several small hashes
- fewer useless probes
- better balance of speed and match quality

### E. Repeat-offset / rep-distance modeling
If you adopt repeated recent distances like LZMA/LZX/Zstd:
- parser state can prefer cheap rep matches quickly
- many useful matches are discovered / selected with less search
- can improve both speed and ratio in practice

## Recommended parser strategy

### Fast mode
- single-pass lazy parser
- cap candidate matches aggressively
- no iterative cost convergence
- no huge sublength list

### Balanced mode
- bounded DP / shortest-path parse
- only top K candidates per position, e.g. 4–16
- only a few sublengths: longest, a few code-boundary lengths, maybe rep distances
- one pass using pre-estimated costs or occasional block-local adaptation

### Slow mode
- fuller optimal parse if you still want a flagship ratio mode

## Bottom line

Yes, the parser can be much faster. The biggest improvement is **not cleverer DP math**; it is **feeding the parser far fewer candidates and abandoning exhaustive per-position evaluation**.

---

## 4. Range coder: SIMD opportunities? Batch processing?

## Short answer
- **SIMD helps only a little** for a classic scalar adaptive range coder.
- The main bottleneck is not arithmetic throughput, but **serial dependency**: each symbol updates `low`, `range`, and the model before the next symbol can be encoded.
- The better speed wins are from **simpler models**, **less frequent rebuild/update work**, or **switching to a different coder** for fast modes.

## What is and isn’t SIMD-friendly

### Hard to SIMD
Classic arithmetic/range coding is hard to vectorize because:
- symbol `i+1` depends on the renormalized state after symbol `i`
- adaptive model update also depends on prior symbols
- carry / renormalization logic is branchy

So “SIMD the whole range coder” is usually not where you get big wins.

### Possible SIMD-adjacent wins

#### A. SIMD for match comparison, not coding
This is much more promising.

In `find_matches()`, the byte-by-byte extension loop:

```rust
while l < max_l && data[c + l] == data[pos + l] { l += 1; }
```

is a great candidate for:
- word-at-a-time compare (`u64` / `u128`)
- platform SIMD loads with mismatch detection

This can substantially reduce the constant factor in match extension.

#### B. Batch histogram building
For Huffman block encoding, histogram collection over literals/lengths/distances can benefit from:
- chunked counting
- multiple local histograms merged at end
- possible SIMD-assisted byte histogramming for literals

Not as huge as algorithm changes, but useful later.

#### C. Faster model update structure
Current `RcModel::update()` increments the cumulative tail on every symbol:

```rust
for i in (sym+1)..=self.nsym { self.cum[i] += 1; }
```

For 258 symbols this is “small,” but still a lot of tiny dependent writes.

Better options:
- keep only frequencies and rebuild cumulative totals periodically
- use a Fenwick tree / BIT if you need prefix sums dynamically
- use static/block models instead of fully adaptive update-per-symbol

#### D. Multi-stream coding
If you really want SIMD-ish arithmetic coding speedups, one approach is **multiple independent streams** encoded in parallel. But that complicates format design and usually isn’t worth it here unless the format is still fluid.

## Best practical answer for this codebase

For speed mode:
- prefer **Huffman** or table-based ANS/FSE-style coding over adaptive range coding
- or at least make range coding block-static, not symbol-adaptive

For the current range coder specifically:
1. stop updating cumulative tails every symbol
2. reduce / batch rescaling
3. avoid binary search in decoder if possible
4. consider replacing with a faster block entropy coder in fast mode

---

## 5. What is a practical speed target?

## Not practical in the short term: 50 MB/s

Given current architecture:
- hash-chain match finder
- optimal-ish DP parse
- optional BWT
- repeated whole-stream/block evaluation
- adaptive range coding

**50 MB/s is not a realistic “optimization target.”** It is a **different compressor design target**.

## Practical staged targets

### Stage 1: **1 MB/s**
This should be achievable quickly by removing obvious disasters:
- disable BWT default path
- reduce chain limit
- simplify parser
- stop repeated block-size global retries

### Stage 2: **3–10 MB/s**
Achievable with a competent fast LZ frontend:
- better match finder (BT/HC4/multi-hash)
- lazy/bounded parser
- reduced entropy-coder overhead
- fewer whole-stream retries

### Stage 3: **10–20 MB/s**
Possible if you are disciplined about speed-first design:
- strong fast match finder
- limited parse depth
- SIMD/word-at-a-time match extension
- simpler/faster block entropy coding

### Stage 4: **20–50 MB/s**
Possible only if you shift toward:
- zstd/lz4/lz4hc-style engineering choices
- much simpler parse logic
- much less adaptive coding overhead
- probably no BWT in default mode

## My recommendation

Use these explicit product targets:

- **Fast mode:** **10–30 MB/s**, ratio closer to gzip/zstd-fast
- **Balanced mode:** **3–10 MB/s**, better ratio than gzip
- **Max ratio mode:** **0.5–2 MB/s**, optional BWT or fuller parse allowed

For the current codebase, the best near-term target is:

## **Aim for 5–10 MB/s first**

That is aggressive enough to matter, but realistic without rewriting the whole compressor into something else.

---

## 6. Speed vs ratio tradeoffs: Pareto frontier

Right now the implementation is in a bad part of the frontier: **very slow without correspondingly elite compression ratio**.

A healthier Pareto frontier would have three lanes:

## A. Fast lane
Design choices:
- no BWT
- hash/HC4 or shallow BT match finder
- lazy parse
- modest window
- static/block Huffman
- fixed block size

Expected outcome:
- high throughput
- ratio around gzip to modestly better, depending on corpus

## B. Balanced lane
Design choices:
- no BWT by default
- stronger match finder
- bounded optimal parse
- rep distances
- per-block Huffman or light adaptive coding

Expected outcome:
- moderate throughput
- ratio potentially better than gzip and competitive with slower DEFLATE-like encoders

## C. Max-ratio lane
Design choices:
- optional BWT or suffix-array-assisted transform
- larger search limits
- richer parser
- maybe adaptive range coder

Expected outcome:
- slow
- potentially best ratio on some text-like / repetitive corpora
- not for general-purpose fast compression

## Practical Pareto advice

### Keep BWT off the default frontier
BWT should live only in a **max-ratio mode** unless measurements prove it wins ratio enough to justify a large speed tax.

### Spend complexity budget on match finding before entropy coding
In LZ compressors, the ratio/speed frontier is usually shaped more by:
- how good your matches are
- how expensive your parse is

than by small differences between decent entropy coders.

### Avoid repeated global “try everything” logic
Trying multiple transforms and block sizes on the whole token stream improves ratio a bit, but usually moves you leftward on the frontier too much.

A better frontier comes from:
- one good default block size
- a cheap heuristic to choose transform mode
- optional deeper mode only when explicitly selected

---

## 7. Prioritized implementation plan with estimated gains

These are **rough multiplicative speedup estimates**, not additive percentages. Real gains will interact.

## Priority 0: Measure first

Before changing architecture, add profiling/timers around:
- `bwt_forward`
- `find_matches`
- DP loop in `lz77_tokenize`
- `estimate_block_size_mode`
- `encode_block`
- range encode/decode

Even basic wall-clock counters per phase will validate the roadmap quickly.

**Effort:** very low  
**Expected gain:** no direct speed gain, but prevents wasted work

---

## Priority 1: Disable BWT in default mode

### Change
Do **not** run:
```rust
let bwt_result = if input.len() <= 2_000_000 { bwt_compress(input) } else { Vec::new() };
```
on every small/medium input.

Instead:
- default to LZ path only
- enable BWT only with an explicit “best ratio / slow mode” flag
- optionally gate BWT by cheap heuristics on entropy / repetition / text-likeness

### Why it matters
Right now you may spend huge time constructing a BWT result only to discard it if larger than LZ77.

### Estimated speed gain
- On inputs where BWT currently runs: **2× to 50×+** depending on size/data
- On average overall workloads: probably the single biggest immediate win

### Effort
**Very low**

### Bang for buck
**Excellent**

---

## Priority 2: Replace full optimal parse with lazy or bounded parse

### Change
In `lz77_tokenize()`:
- remove 2-iteration full DP from default mode
- use a **lazy parser** or **bounded DP** with top-K matches only
- remove broad sublength enumeration from fast path

### Why it matters
This cuts both:
- parser CPU
- the amount of match information you need to generate/store

### Estimated speed gain
- **2× to 8×** on the LZ path
- ratio loss likely modest if done carefully

### Effort
**Medium**

### Bang for buck
**Excellent**

---

## Priority 3: Improve the match finder

### Change
Replace long hash-chain probing with one of:
- **HC4 / multi-hash chain**
- **binary tree match finder**
- at minimum, much lower `HASH_CHAIN_LIMIT` in fast mode

And stop storing all matches for all positions if not needed.

### Why it matters
Current match finding is brute force with bad worst-case behavior. Better data structures reduce candidate count dramatically.

### Estimated speed gain
- **2× to 6×** on LZ-heavy workloads
- also improves parser scalability

### Effort
**Medium to high**

### Bang for buck
**Very high**

---

## Priority 4: Add word-at-a-time / SIMD-assisted match extension

### Change
Optimize this pattern:
```rust
while l < max_l && data[c + l] == data[pos + l] { l += 1; }
```
using:
- `u64` / `u128` chunk compares
- mismatch detection via XOR and trailing-zero count
- optional architecture SIMD later

### Why it matters
This inner loop runs constantly inside match finding.

### Estimated speed gain
- **1.2× to 2×** overall after match-finder changes
- larger on repetitive data

### Effort
**Medium**

### Bang for buck
**Good**

---

## Priority 5: Stop repeated whole-stream block-size trials

### Change
Currently `lz77_compress()` encodes the whole token stream multiple times for block sizes:
- 8192
- 16384
- 32768
- 65536

Pick one default block size for fast/balanced mode.
Maybe keep multi-size search only in slow mode.

### Why it matters
Whole-stream retries are expensive and ratio gain is usually limited.

### Estimated speed gain
- **1.2× to 2×** depending on input size and proportion of encode time

### Effort
**Low**

### Bang for buck
**Very good**

---

## Priority 6: Reduce per-block transform search

### Change
Instead of trying all three literal transforms by full estimation each block:
- use a heuristic pre-check
- or evaluate only `None` vs `XorDelta` in fast mode
- reserve MTF literal transform for slow mode / text-like heuristics

### Why it matters
`estimate_block_size_mode()` walks tokens repeatedly and rebuilds code lengths repeatedly.

### Estimated speed gain
- **1.1× to 1.5×** overall

### Effort
**Low**

### Bang for buck
**Good**

---

## Priority 7: Make the entropy stage simpler/faster

### Options
1. Keep block Huffman for fast mode and reserve range coding for slow mode
2. If keeping range coder, make model updates cheaper
3. Avoid binary search decode; use lookup structures for small alphabets
4. Rebuild cumulative tables periodically instead of updating suffix sums every symbol

### Estimated speed gain
- **1.1× to 1.5×** overall if LZ path dominates
- maybe more on BWT mode, but BWT should already be slow/optional

### Effort
**Medium**

### Bang for buck
**Moderate**

---

## Priority 8: Replace BWT implementation for optional max-ratio mode

### Change
If BWT remains in the product:
- replace `bwt_forward()` with **libsais** or **libdivsufsort**
- keep BWT block size modest and configurable

### Why it matters
Makes BWT mode viable rather than pathological.

### Estimated speed gain
- **10× to 100×+** for BWT mode specifically
- but little effect on default mode if BWT is already disabled there

### Effort
**Medium** with FFI, **high** for homegrown pure-Rust suffix array

### Bang for buck
**Excellent for slow mode**, less important for default mode

---

## Suggested roadmap by phase

## Phase 1 — Immediate triage (lowest effort, biggest wins)

1. **Disable BWT by default**
2. **Use one block size by default**
3. **Reduce transform search in fast mode**
4. **Reduce `HASH_CHAIN_LIMIT` aggressively for fast mode**
5. **Replace 2-pass full DP with lazy parse for fast mode**

### Expected result
From **0.1 MB/s** to roughly **1–3 MB/s** if the implementation is cleaned up sensibly.

---

## Phase 2 — Real compression-engine improvements

1. Implement **bounded parser** and/or lazy vs balanced modes
2. Replace current match finder with **HC4** or **binary tree**
3. Add **word-at-a-time match extension**
4. Stop materializing full `Vec<Vec<(len,off)>>` for all positions unless a slow mode needs it

### Expected result
Move from **1–3 MB/s** toward **5–10+ MB/s**.

---

## Phase 3 — Entropy and format optimization

1. Simplify fast entropy coding path
2. Improve histogram/code-length builder performance
3. Make range coder cheaper or optional
4. Tune block size based on measurement, not brute-force retries

### Expected result
Potentially **10–15+ MB/s** in balanced mode.

---

## Phase 4 — Optional max-ratio mode cleanup

1. Replace BWT sorter with **libsais/divsufsort**
2. Keep BWT only for explicit slow/high-ratio mode
3. Evaluate whether BWT actually beats improved LZ path enough to justify its existence

### Expected result
BWT mode becomes respectable, but still not your fast path.

---

## Recommended product strategy

I would define three presets:

## `--fast`
- No BWT
- single block size
- HC4 or shallow hash match finder
- lazy parse
- Huffman block coding
- target: **10–30 MB/s** eventually

## `--balanced`
- No BWT
- stronger match finder
- bounded parse
- optional more expensive transforms only when heuristic says yes
- target: **3–10 MB/s** with good ratio

## `--max`
- optional BWT / suffix-array backend
- deeper parse
- more expensive modeling
- target: **0.5–2 MB/s**, ratio-first

That gives a sensible Pareto frontier instead of one mode that is slow for everybody.

---

## Final recommendation

If you want the **highest speed gain for the least engineering pain**, do this in order:

1. **Stop auto-running BWT**
2. **Kill full optimal DP in the default path**
3. **Use a faster match finder or slash chain depth**
4. **Remove repeated whole-stream/block “try all options” work**
5. **Only then worry about range coder optimization**

If you do only the first three well, you can plausibly move from **0.1 MB/s** to **multi-MB/s** territory. If you do them all well, **5–10 MB/s** is a realistic target. Beyond that, getting to **20+ MB/s** will require stronger architectural commitment to a speed-first compressor design.

## Short answer to the big question

- **Where does time go?** Mostly BWT sort, then match finder, then DP parse.
- **Fastest BWT algorithm?** In practice: **libsais** / **divsufsort**. Theoretical headline: **SA-IS O(n)**.
- **Can DP be faster?** Yes — massively, by moving to **lazy or bounded parsing** with a better match finder.
- **SIMD range coder?** Limited payoff. SIMD the **match extension**, not the arithmetic coder.
- **Practical target?** **5–10 MB/s first**, **10–20 MB/s later** if architecture improves. **50 MB/s** means a different design philosophy.
- **Best bang-for-buck fixes?** Disable BWT default path, simplify parse, improve match finder, stop retrying everything.

# Radical & Unconventional Compression Ideas

> Research analysis for autosqueeze — exploring approaches beyond traditional LZ/entropy coding.
> Generated 2026-03-15.

---

## 1. Neural Compression (Per-Block Tiny Neural Net)

**Concept:** Train a small neural network (e.g., a 1-layer RNN/LSTM or even a simple feedforward net) on each data block to predict the next byte. The compressed output is the network weights + the residual errors (encoded arithmetically using the net's predicted probabilities). At decompression, you reconstruct the net and use it to regenerate predictions, decoding residuals back to original data.

This is essentially what NNCP (Neural Network Compression) and CMIX do — they use neural nets as context mixers for probability prediction, then feed those probabilities into arithmetic coding.

**Feasibility in pure Rust:** 6/10. You'd need to implement:
- A tiny forward/backward pass (matrix multiply, sigmoid/tanh activations)
- A simple optimizer (SGD is sufficient)
- Arithmetic coding driven by the net's output probabilities
- Weight serialization

No external crates needed — matrix ops on small dimensions (e.g., 64x256) are trivial loops. The hard part is tuning: learning rate, architecture size, training iterations per block. Too many iterations = slow compression. Too few = poor predictions.

**Potential ratio improvement:** 5-15% better than traditional order-N context models on structured data. On highly patterned data (code, structured text), could see 20%+ improvement. On random/encrypted data, the overhead of storing weights makes it *worse*.

**Difficulty:** 7/10. The implementation is moderate, but getting the training loop stable and fast enough for practical compression is the real challenge. You're essentially doing online learning at compression time.

**Key insight:** The smallest useful net is probably a single hidden layer with 32-128 neurons. Weights for a 256→64→256 net = ~33K parameters = ~66KB at fp16. That's massive overhead unless your block is large (1MB+). You'd need aggressive weight quantization (4-bit or even 2-bit) to make the overhead acceptable.

---

## 2. Grammar-Based Compression (Sequitur / Re-Pair)

**Concept:** Instead of sliding-window matching (LZ), discover the *hierarchical grammatical structure* of the data. 

- **Sequitur** builds a context-free grammar from the input by enforcing two invariants: (1) no pair of adjacent symbols appears more than once (digram uniqueness), and (2) every rule is used at least twice (rule utility). The grammar *is* the compressed representation.
- **Re-Pair** (Recursive Pairing) repeatedly finds the most frequent pair of symbols, replaces all occurrences with a new symbol, and recurses. Simpler than Sequitur and often produces smaller grammars.

The compressed output is the grammar rules + the compressed start symbol.

**Feasibility in pure Rust:** 8/10. Both algorithms are straightforward to implement:
- Sequitur: hash table for digrams, doubly-linked list for the sequence, rule table
- Re-Pair: priority queue of pair frequencies, replacement logic

Pure data structures, no exotic math. Re-Pair is particularly clean to implement.

**Potential ratio improvement:** On par with or slightly better than LZ77 for general data. Where it *shines*: highly structured/repetitive data like XML, JSON, source code, DNA sequences — up to 20-30% better than gzip. On random data, slightly worse due to grammar overhead.

The real win: grammar compression finds *nested* repetitions that LZ misses. LZ with a 32KB window can't see a pattern that repeats every 100KB. Grammar compression can.

**Difficulty:** 4/10. This is one of the most accessible radical ideas. Re-Pair in particular is elegant and well-documented. The main implementation challenge is memory efficiency for large inputs (Re-Pair needs to track all pair frequencies).

**Key insight:** Grammar + arithmetic coding on the grammar rules is a powerful combo. The grammar finds structure; arithmetic coding exploits the statistical regularity of the grammar symbols.

---

## 3. Kolmogorov Complexity Approaches

**Concept:** The Kolmogorov complexity of a string is the length of the shortest program (in some fixed language) that outputs that string. If we could find that program, we'd have the theoretically optimal compression. In practice, this is uncomputable (literally — it's equivalent to the halting problem). But we can *approximate* it.

Practical approaches:
- **Program search:** For small blocks, try generating programs (arithmetic expressions, simple loops) that produce the data. If `for i in 0..1000: output(i % 256)` reproduces your block, that's 30 bytes instead of 1000.
- **Lempel-Ziv IS a Kolmogorov approximation** — it finds a "program" (copy instructions) that reproduces data.
- **Superoptimization-inspired:** Try random small programs, see if any produce the data.

**Feasibility in pure Rust:** 3/10. You'd need:
- A tiny bytecode VM / interpreter
- A program enumerator or genetic search
- Massive compute per block (exponential search space)

The VM is easy. The search is the problem — it's fundamentally exponential. Even with aggressive pruning, you'd spend minutes per kilobyte.

**Potential ratio improvement:** Theoretically unbounded — you could compress a billion zeros to 20 bytes. In practice, only useful for highly algorithmic data (counters, mathematical sequences, repeating patterns with simple generators). For general data: negligible improvement, massive time cost.

**Difficulty:** 9/10. Not because the code is hard, but because making it work in bounded time on real data is essentially an open research problem. You'd be solving a variant of program synthesis.

**Key insight:** Could be amazing as a *fallback* for specific data patterns. If a quick heuristic detects "this block might be algorithmically compressible" (low entropy but poor LZ ratio), try a bounded program search. Think of it as a specialized detector, not a general compressor.

---

## 4. Information Geometry / Manifold Learning

**Concept:** Treat byte sequences as points in high-dimensional space. A sliding window of N bytes = a point in R^N. If the data has structure, these points lie on a low-dimensional manifold. Find the manifold, encode positions on it instead of raw bytes.

Concretely: take overlapping windows of size k, embed them using PCA or a simple autoencoder. If the data lives on a d-dimensional manifold where d << k, you can encode each window with d coordinates instead of k bytes.

**Feasibility in pure Rust:** 4/10. You'd need:
- PCA implementation (eigendecomposition of covariance matrix — doable for small matrices with power iteration)
- Or a simple autoencoder (ties into neural compression above)
- Quantization of manifold coordinates
- Reconstruction + residual coding

PCA on small matrices is implementable. Full manifold learning (t-SNE, UMAP, etc.) is way too complex for pure Rust without crates.

**Potential ratio improvement:** Theoretical gains are real for data with geometric structure (audio samples, sensor data, time series). Could see 10-20% improvement on such data. For text/code: probably worse than grammar-based approaches because the "manifold" is irregular and high-dimensional.

**Difficulty:** 8/10. The math is heavy (eigensolvers, numerical stability), and it's unclear how to make this lossless. You'd need exact residual coding to recover the original data after projection, which adds overhead that may negate the manifold gains.

**Key insight:** This is more naturally suited to *lossy* compression (it's basically what JPEG does in the frequency domain). Making it work for lossless general-purpose compression is swimming upstream.

---

## 5. Multi-Scale Decomposition (Wavelet-Like Transforms on Bytes)

**Concept:** Apply wavelet-like transforms to byte data, decomposing it into coarse structure (low-frequency) and fine detail (high-frequency) at multiple scales. Encode each scale separately with optimized coding.

For bytes, this means:
- Split sequence into pairs, compute averages (low-freq) and differences (high-freq)
- Recurse on the averages
- The differences at each scale tend to be small/sparse → compress well

This is essentially the Haar wavelet, and it's the foundation of JPEG 2000.

**Feasibility in pure Rust:** 9/10. Haar wavelet is trivial:
```
avg = (a + b) / 2
diff = a - b
```
More sophisticated wavelets (Daubechies, etc.) are just different filter coefficients. All pure arithmetic. The lifting scheme makes it even simpler — in-place transforms with no temporary buffers.

**Potential ratio improvement:** On smooth/gradual data (sensor readings, audio samples, gradients): 15-30% better than raw entropy coding. On text/code: minimal improvement or worse, because text isn't "smooth" in the byte-value sense. The differences aren't small — jumping from 'A' (65) to 'z' (122) is a big delta.

**Difficulty:** 3/10. One of the easiest to implement. The wavelet transform is simple; the question is whether it helps for your target data types.

**Key insight:** This shines when combined with byte-reordering transforms. If you first apply a BWT (Burrows-Wheeler Transform), the output tends to have runs of similar values — *exactly* the kind of smooth data wavelets love. BWT → wavelet → entropy coding could be a powerful pipeline.

---

## 6. DNA/Genomics Compression Tricks

**Concept:** Genomics has developed specialized compression for sequences with:
- Small alphabets (DNA = 4 symbols, but generalizable)
- Massive exact and approximate repeats
- Tandem repeats (ABABAB...)
- Reverse complements

Key techniques:
- **Relative compression:** Compress against a reference genome. Only store differences.
- **Context-based models** with very deep contexts (order 10-20) — feasible because the alphabet is tiny
- **Repeat-aware parsing:** Detect tandem repeats, encode as (pattern, count)
- **BWT-based** with run-length on the BWT output (this is what modern genomics tools like BSC do)

**Feasibility in pure Rust:** 7/10. The individual techniques are all implementable. Deep context models just need more memory for the context tables. Tandem repeat detection is string matching. Relative compression needs a reference, which changes the compression model.

**Potential ratio improvement:** The specific techniques won't help much for general byte data (256-symbol alphabet makes deep contexts expensive). BUT the *philosophy* is transferable:
- Detect and exploit tandem repeats in any data
- Use reference-based compression (see #9)
- Adapt alphabet size to the data (if a block only uses 30 distinct bytes, treat it as a 30-symbol alphabet with deep contexts)

For actual genomic data: 5-10x better than gzip. For general data with detected patterns: 5-10% improvement from repeat detection alone.

**Difficulty:** 5/10. Individual techniques are moderate. The win comes from assembling them intelligently with data-type detection.

**Key insight:** The most transferable idea is **alphabet reduction + deep context modeling**. If a block uses a small effective alphabet, you can afford much deeper context models, which dramatically improves prediction.

---

## 7. Algebraic Coding Theory (Group Theory / Rings)

**Concept:** Use algebraic structures (groups, rings, finite fields) to construct more efficient codes. This is the foundation of error-correcting codes (Reed-Solomon, BCH, turbo codes), but the ideas can apply to compression:

- **Arithmetic in GF(2^8):** Treat bytes as elements of a Galois field. Some data transformations become simpler/more compressible in this representation.
- **Syndrome-based compression:** Related to Slepian-Wolf coding — compress a source by computing its "syndrome" with respect to a known structure.
- **Group-theoretic transforms:** Generalized FFTs over non-abelian groups can reveal structure that standard transforms miss.

**Feasibility in pure Rust:** 5/10. GF(2^8) arithmetic is standard (it's how AES works — lookup tables for multiply/divide). Group-theoretic transforms are more complex but tractable for small groups. The challenge is connecting algebraic structure to compression gains.

**Potential ratio improvement:** Marginal for general compression (1-3%). The algebraic approach is more about *provable* properties than raw ratio. Where it helps: data with algebraic structure (error-corrected data, mathematical sequences, structured binary formats).

**Difficulty:** 8/10. The math is genuinely hard, and the connection to practical compression gains is tenuous. This is more of an academic avenue.

**Key insight:** The most practical algebraic technique for compression is using GF(2^8) arithmetic for efficient XOR-based delta coding and for constructing good hash functions for LZ matching. Not revolutionary, but useful as a building block.

---

## 8. Fractal Compression for Byte Sequences

**Concept:** Fractal compression (used in image compression) exploits self-similarity at different scales. The idea: find parts of the data that look like scaled/transformed versions of other parts. Store the transformations instead of the data.

For images, this means finding 8x8 blocks that approximate scaled-down 16x16 blocks (with affine brightness/contrast adjustment). For byte sequences, you'd look for subsequences that are affine transformations of other subsequences.

**Feasibility in pure Rust:** 6/10. The algorithm is:
1. Partition data into "range" blocks
2. Search for "domain" blocks (larger, from a pool) that approximate each range after some transform
3. Store the (domain_index, transform_params) pairs

Implementing the search and transforms is straightforward. The challenge: for byte sequences, the "transforms" that preserve meaning are limited. You can't just scale byte values like pixel intensities.

**Potential ratio improvement:** For self-similar data (recursive structures, fractals, some scientific data): potentially 20-40%. For general text/binary: poor. Byte sequences rarely exhibit the kind of scale-invariant self-similarity that makes fractal compression work on images.

**Difficulty:** 6/10. The algorithm is well-documented from the image compression literature. Adapting it to byte sequences requires rethinking what "similarity transform" means in this context. Could define domain→range transforms as: offset + scale (on byte values), or reordering + delta.

**Key insight:** Don't try to make general fractal compression work on bytes. Instead, *detect* self-similar blocks as a preprocessing step and route them to a fractal-inspired coder. Use conventional compression for everything else. The detection is the key — if self-similarity exists, exploit it; if not, don't pay the overhead.

---

## 9. Delta Compression Against a Reference / Shipped Dictionary

**Concept:** Instead of compressing data in isolation, compress it against a *reference* that both compressor and decompressor know. This is how:
- Git packfiles work (delta against previous versions)
- HTTP dictionary compression works (shared Brotli dictionary)
- Zstandard's dictionary mode works

For autosqueeze, you could:
- Ship a built-in dictionary of common byte patterns (HTML tags, JSON structure, common English words, ELF headers, etc.)
- Let users train custom dictionaries on their data
- Use a universal "seed" dictionary derived from a large corpus

**Feasibility in pure Rust:** 9/10. This is the most practical idea on this list:
- Build a dictionary at compile time (embed as `const` data)
- Use it as the initial LZ window / initial context for probability models
- Standard LZ77 with a pre-populated window

Zstandard already proves this works beautifully. The implementation is essentially "initialize your LZ hash table with the dictionary contents."

**Potential ratio improvement:** 
- Small files (<4KB): 30-60% better ratio (huge win — the dictionary eliminates the cold-start problem)
- Medium files (4KB-1MB): 10-25% improvement
- Large files (>1MB): 2-5% improvement (the data itself provides enough context)

The win is *massive* for small files, which is exactly where most compressors perform worst.

**Difficulty:** 3/10. The implementation is simple. The hard part is *building a good dictionary* — but Zstandard's `zstd --train` algorithm (cover/fastcover) is well-documented and implementable.

**Key insight:** This is the single highest-ROI idea on this list. Ship a universal dictionary (or a few domain-specific ones: text, code, binary) and use it to bootstrap context. Brotli's built-in dictionary is one reason it beats gzip so consistently on web content. The dictionary can be as simple as 32KB of common patterns.

---

## 10. Learned / Offline-Trained Entropy Coding

**Concept:** Instead of learning probability distributions during compression (adaptive), pre-train them on a large corpus and ship them as static tables. The compressor uses these pre-trained probabilities directly (or blends them with adaptive estimates).

This is related to #9 (dictionary) but operates at the probability level rather than the pattern level:
- Pre-compute order-2 or order-3 context probability tables from a large training corpus
- Ship these tables (~256KB for order-2, ~16MB for order-3 — can be compressed themselves)
- At compression time, use these as priors that get refined by actual data

**Feasibility in pure Rust:** 8/10. Implementation:
- Offline: count byte trigrams/quadgrams in a training corpus, build probability tables
- Embed tables in the binary (or load from a file)
- At compression time: blend pre-trained probabilities with adaptive counts
  - `P(byte) = α × P_pretrained(byte | context) + (1-α) × P_adaptive(byte | context)`
  - α decays as more data is seen (adaptive takes over)

All of this is table lookups and arithmetic. No exotic math.

**Potential ratio improvement:**
- First 1-4KB of data: 10-30% improvement (pre-trained model is immediately accurate)
- After 100KB+: <2% improvement (adaptive model has converged on its own)
- Overall on typical files: 5-15% improvement

Like dictionaries, the big win is eliminating the cold-start problem of adaptive coding.

**Difficulty:** 4/10. Building the tables is just counting. Blending is simple arithmetic. The design decisions (context order, blending schedule, table size) require experimentation but not complex code.

**Key insight:** This pairs beautifully with #9. Dictionary provides LZ-level bootstrapping; pre-trained probability tables provide entropy-coding-level bootstrapping. Together they eliminate the cold-start problem at both levels. For a compressor that handles many small files (web assets, config files, log entries), this combination could be transformative.

---

## Summary Rankings

| Idea | Ratio Gain | Feasibility (Rust) | Difficulty | Priority |
|------|-----------|-------------------|-----------|----------|
| 9. Reference Dictionary | ★★★★★ | ★★★★★ | 3/10 | **#1 — Do this first** |
| 10. Learned Entropy Coding | ★★★★ | ★★★★ | 4/10 | **#2 — Pairs with #9** |
| 2. Grammar Compression | ★★★★ | ★★★★ | 4/10 | **#3 — Novel and effective** |
| 5. Wavelet Transform | ★★★ | ★★★★★ | 3/10 | **#4 — Easy experiment** |
| 6. DNA/Genomics Tricks | ★★★ | ★★★★ | 5/10 | **#5 — Cherry-pick ideas** |
| 1. Neural Compression | ★★★ | ★★★ | 7/10 | #6 — High effort, moderate gain |
| 8. Fractal Compression | ★★ | ★★★ | 6/10 | #7 — Niche applicability |
| 4. Info Geometry | ★★ | ★★ | 8/10 | #8 — Better for lossy |
| 7. Algebraic Coding | ★ | ★★★ | 8/10 | #9 — Academic interest |
| 3. Kolmogorov Complexity | ★★★★★ | ★ | 9/10 | #10 — Theoretically beautiful, practically intractable |

## Recommended Strategy

**Phase 1 (Quick Wins):**
1. Implement a reference dictionary system (#9) — biggest bang for buck
2. Add pre-trained probability tables (#10) — compounds with #9
3. These two together could improve small-file compression by 30-50%

**Phase 2 (Structural Innovation):**
4. Experiment with Re-Pair grammar compression (#2) as an alternative to LZ
5. Try BWT → wavelet → entropy coding pipeline (#5)
6. Add alphabet detection + deep context modeling from genomics (#6)

**Phase 3 (Advanced/Experimental):**
7. Neural context mixing (#1) for maximum ratio
8. Bounded program search (#3) as a fallback for algorithmic data
9. Fractal self-similarity detection (#8) as a preprocessing classifier

**Skip unless researching:** #4 (info geometry) and #7 (algebraic coding) — interesting but ROI is too low for a practical compressor.

# Academic Survey: Compression Breakthroughs, 2020-2026

## Executive take

This survey focuses on research directions that matter for pushing an overall compression target from **0.2589 toward 0.15 bits-per-input-bit equivalent**. The short version is:

- **No single classical breakthrough from 2020-2026 plausibly cuts 0.2589 straight to 0.15 by itself.**
- The most credible path is **hybrid**: strong classical structure discovery (LZ/BWT/grammar/repetitiveness indexes) + stronger probabilistic modeling on the hard residuals + tighter entropy coding.
- The literature shows the clearest *absolute* gains in recent years in two places:
  1. **learned / autoregressive compressors** on text-like sources, and
  2. **better exploitation of repetitiveness** via run-length BWT, LZ-End/LZ variants, and grammar-based access/index structures.
- The most relevant engineering lesson for a Rust implementation is that almost every promising idea here is **implementable in Rust**, but some are much more practical than others. In rough order of practicality: **entropy coding variants, LZ-family variants, BWT/run-based structures, grammar-based offline parsers, then learned/neural models**.

I use “ratio improvement” carefully below. Many papers do **not** report a clean universal “X% better than baseline” number on Silesia/Calgary/enwik. When exact benchmark deltas were unavailable from accessible abstracts/pages, I mark them as **not cleanly reported in the abstract** and interpret the likely relevance conservatively.

---

## 1. Neural compression and learned autoregressive models

### 1.1 L3TC / “Leveraging RWKV for Learned Lossless Low-Complexity Text Compression” (2024)
**Source:** arXiv:2412.16642

**Key idea**
- Uses a **learned probabilistic text model** plus an entropy coder.
- Chooses **RWKV** as a lower-compute alternative to transformer backbones.
- Adds an **outlier-aware tokenizer** so common patterns are modeled while outliers bypass expensive prediction.
- Adds a **high-rank reparameterization** that improves training quality without increasing inference cost.

**Reported improvement**
- Abstract reports **48% bit saving compared to gzip**.
- Also claims compression performance comparable to other learned compressors while using **50x fewer model parameters** and much faster decoding.

**Rust implementability**
- **Yes, implementable in Rust**, but realistically as a systems project rather than a quick codec.
- The entropy-coding side is straightforward in Rust.
- The RWKV inference path is implementable through Rust tensor/runtime bindings or custom kernels.
- The outlier-tokenizer idea is especially portable.

**Relevance to 0.2589 → 0.15**
- **High relevance** if your data is text-like, repetitive, or language-structured.
- The strongest lesson is not “use RWKV exactly,” but **use a cheap autoregressive predictor only where it pays**, and route outliers differently.
- This is one of the few 2024-era papers that points toward a plausible practical hybrid rather than a purely academic result.

**My read**
- Useful as a design pattern: **mixture-of-models compression** with explicit escape handling.
- For a Rust compressor chasing 0.15, this is more realistic than trying to embed a giant transformer everywhere.

---

### 1.2 “Lossless Compression of Large Language Model-Generated Text via Next-Token Prediction” (2025)
**Source:** arXiv:2505.06297

**Key idea**
- Observes that **LLM-generated text is unusually predictable to the LLMs that generated it**.
- Uses **next-token prediction probabilities** directly as the modeling backbone for lossless compression.
- Frames LLMs as strong compressors of their own output distributions.

**Reported improvement**
- Abstract reports **over 20x compression** on LLM-generated datasets.
- Baseline cited is **~3x for gzip** on those datasets.
- That is a huge domain-specific gain, but importantly it is on **LLM-generated text**, not generic mixed binary corpora.

**Rust implementability**
- **Yes, but expensive.**
- The coding path is easy; the challenge is inference cost, batching, KV-cache management, and deterministic tokenization.
- If you already have a Rust inference stack or FFI into a model runtime, it is feasible.

**Relevance to 0.2589 → 0.15**
- **Potentially very high on the right source distribution, limited otherwise.**
- If autosqueeze is targeting any AI-text-heavy or code-heavy corpus, this paper is a major signal.
- If your corpus is broad binary data, the relevance drops sharply.

**My read**
- The result is exciting, but the lesson is **domain-adaptive modeling** rather than a universal compressor.
- Best use: identify segments that are likely generated/probabilistic-language output and switch to a model-based coder there.

---

### 1.3 TRACE / NNCP / recent learned text compressors cited in 2024-2025 literature
**Source trail:** mentioned in the 2025 LLM-generated-text paper snippet and adjacent recent arXiv work.

**Key idea**
- A growing family of compressors uses **transformer or lightweight transformer predictors** for lossless text compression.
- The general scheme is stable: model predicts token/byte probabilities, entropy coder converts those probabilities to bits.

**Reported improvement**
- Accessible source snippets did **not provide a clean single comparable benchmark delta** across Silesia/Calgary/enwik.
- The field trend is that learned text compressors often beat older generic baselines like gzip/bzip2, but still face a cost-performance tradeoff against tuned context mixers and PAQ/cmix-style systems.

**Rust implementability**
- **Yes**, but practical success depends more on model serving architecture than codec logic.

**Relevance to 0.2589 → 0.15**
- **High as a research direction**, especially if integrated selectively.
- The best near-term angle is likely **learned residual coding**, not full end-to-end learned compression over every byte.

---

## 2. New entropy coding methods and asymmetric coding innovations

### 2.1 Continued practical relevance of ANS / asymmetric coders in modern systems
**Source trail:** post-2020 work remains more evolutionary than revolutionary in public literature reachable here; Matt Mahoney’s older ABC/fpaq lineage remains conceptually relevant.

**Key idea**
- Replace or refine arithmetic/range coding with **ANS-family coders** or related asymmetric coders that can approach arithmetic-coding efficiency with better speed or table-driven simplicity.
- The innovation space since 2020 has mostly been in **systems integration, vectorization, table design, and model/coder co-design**, more than a brand-new universal entropy coder overthrowing the rest.

**Reported improvement**
- For many papers and implementations, the gain is usually **small but real**: lower overhead, better throughput, sometimes tiny ratio gains from cleaner bitstream design.
- I did **not find a credible 2020-2026 paper in the accessible pass showing a giant ratio breakthrough purely from a new entropy coder alone**.

**Rust implementability**
- **Very high.** ANS/rANS/tANS-style coders are among the easiest high-value advanced components to implement well in Rust.
- Rust is a strong fit for table generation, bitstream control, SIMD wrappers, and deterministic testing.

**Relevance to 0.2589 → 0.15**
- **Medium, not sufficient alone.**
- Entropy coding improvements usually shave the final few percent, not a 40%+ step change.
- Still necessary in any serious attempt at 0.15, because weak coding wastes whatever gains the model creates.

**My read**
- You should absolutely expect to use a modern ANS/range coder in Rust.
- But do not mistake entropy coding for the main breakthrough lever; the real leverage is in the **probability model and transforms**.

---

### 2.2 Asymmetric binary coding lineage as implementation guidance
**Source:** Matt Mahoney’s data-compression notes (older but still relevant as engineering precedent)

**Key idea**
- Very compact asymmetric binary coders reduce state complexity and can be table-driven.
- This matters because aggressive models are useless if coding overhead or branchiness kills speed.

**Reported improvement**
- Historical notes show small ratio/speed benefits over simple arithmetic coders in toy/order-0 settings; not a 2020-2026 breakthrough itself.

**Rust implementability**
- **Excellent.** This is a clean candidate for a low-level Rust core.

**Relevance to 0.2589 → 0.15**
- **Supportive, not decisive.**
- Worth doing if you have a sophisticated model and need a clean final-stage coder.

---

## 3. LZ-family innovations

### 3.1 “Computing the LZ-End parsing: Easy to implement and practically efficient” (2024)
**Source:** arXiv:2409.07840

**Key idea**
- Revisits **LZ-End**, a variant competitive with LZ77 while enabling **efficient random access**.
- Simplifies and improves a previously less-practical algorithm.
- Uses lazy evaluation and lower-indirection structures to reduce practical cost and memory.
- Important point: this is not just theory; the paper explicitly argues for **practical implementability**.

**Reported improvement**
- The abstract does **not give a clean compression-ratio delta** over LZ77.
- It claims compression is **competitive with LZ77** and that their parser is **faster than the prior state of the art** for computing LZ-End.
- So the breakthrough here is mostly **algorithmic practicality**, not a universally better ratio.

**Rust implementability**
- **High.** In fact this is one of the most Rust-friendly results in the whole survey.
- The paper explicitly emphasizes a smaller toolset and easier implementation.
- Suffix-array/LCP/inverse-SA plus ordered predecessor/successor structures are all reasonable in Rust.

**Relevance to 0.2589 → 0.15**
- **Medium-high**, especially if random access matters or if your corpus has strong long repeated substrings.
- LZ-End by itself probably does not get you to 0.15, but it can be an important structural front-end before stronger residual modeling.

**My read**
- This is one of the best “actually buildable” academic findings here.
- If you want something novel but not insane, **LZ-End is a strong candidate**.

---

### 3.2 “Height-bounded Lempel-Ziv encodings” (2024)
**Source:** arXiv:2403.08209

**Key idea**
- Introduces **LZHB**, a family of LZ-like encodings with explicit bounds on the **height of the reference forest**.
- This gives a clean tradeoff between **compression and direct-access time**.
- The paper proves structural links to repetitiveness measures and run-length grammars.

**Reported improvement**
- The abstract’s main improvement is theoretical/practical algorithmics, not a headline benchmark ratio.
- It claims the first **linear-time algorithm** for greedy LZHB on constant alphabets and shows strong relationships such as `z_HB = O(g_rl)` under logarithmic height bounds.
- No clean public abstract number like “X% smaller than LZ77” is given.

**Rust implementability**
- **High to medium-high.**
- More subtle than vanilla LZ77, but still absolutely implementable in Rust.
- Great fit for careful arena-based graph/forest representations and succinct indexing.

**Relevance to 0.2589 → 0.15**
- **Medium.**
- The biggest value is architectural: it suggests that **controlled-reference-depth LZ variants** can preserve compression while making decode/access manageable.
- That matters if the ultimate system needs practical decompression, not just a benchmark stunt.

**My read**
- Stronger as a **design framework** than as a guaranteed ratio winner.
- Worth mining for parser constraints and hybrid grammar/LZ design.

---

### 3.3 Broader LZ line: compressed indexes, LZ77-on-repetitive-data, and “LZ-Next”-style directions
**Source trail:** recent repetitive-text indexing literature, including LZ-End and run-based indexes.

**Key idea**
- The 2020-2026 action in LZ is less about inventing a totally new household-name parser and more about:
  - making old parsings **computable in near-linear or linear time**,
  - tying them to **compressed self-indexes**,
  - and understanding them relative to grammar measures and BWT runs.

**Reported improvement**
- Typically algorithmic rather than raw benchmark-ratio headline improvements.
- The important pattern is **better exploitation of repetitiveness with stronger queryability**.

**Rust implementability**
- **Yes.**

**Relevance to 0.2589 → 0.15**
- **Medium-high as infrastructure.**
- A serious 0.15 attempt likely benefits from an LZ-style parser to expose structure before entropy coding or learned modeling.

---

## 4. Grammar compression advances (RePair, Sequitur, SLP-based)

### 4.1 Post-2020 grammar-compression trend: from pure compression to compressed self-indexing and local consistency
**Source trail:** references surfaced in repetitive-text/BWT literature, including LMS-based grammar self-indexing in 2021 and run-length grammar comparisons.

**Key idea**
- Recent grammar-compression work often centers on **SLP / run-length grammar / grammar self-index** formulations.
- The shift is from “smallest grammar in theory” toward **practical grammar structures with indexing, local consistency, and provable ties to repetitiveness measures**.

**Reported improvement**
- Hard to state as a single universal ratio number because these papers often optimize structure, query time, or approximation guarantees rather than benchmark bytes.
- Height-bounded LZ’s link to `g_rl` is relevant because it shows grammar size remains a central comparison baseline.

**Rust implementability**
- **Medium.**
- RePair-style offline grammar construction is definitely implementable.
- Fully succinct SLP self-indexing with strong query performance is harder, but still feasible in Rust if treated as a research subsystem.

**Relevance to 0.2589 → 0.15**
- **Medium-high on repetitive corpora; lower on mixed noisy data.**
- Grammar compression is most attractive when your corpus has repeated macro-structure beyond what local byte models capture.

**My read**
- Grammar methods are probably more valuable as a **structural preprocessor** than as the sole final compressor.
- For the 0.15 target, consider using grammar discovery to extract repeated nonlocal phrases before statistical coding.

---

### 4.2 RePair / Sequitur / SLP lessons that still matter in 2020-2026
**Key idea**
- RePair-style dictionary growth and Sequitur-style grammar induction remain attractive because they discover **hierarchical reuse** instead of just nearest-copy matches.
- Recent theory increasingly compares new structures against grammar size, which means grammar remains a strong “north star” even when not used directly.

**Reported improvement**
- No accessible 2020-2026 abstract in this pass gave a fresh universal ratio record for RePair/Sequitur descendants.
- The literature emphasis appears to be **better algorithms and indexing properties**, not public benchmark dominance.

**Rust implementability**
- **Yes.** RePair especially is quite implementable offline.

**Relevance to 0.2589 → 0.15**
- **Medium.**
- Likely useful if merged with residual entropy coding rather than deployed alone.

---

## 5. BWT innovations (tunneling, BCR, extended BWT, run-based indexing)

### 5.1 “Optimal-Time Queries on BWT-runs Compressed Indexes” (2020/2021)
**Source:** arXiv:2006.05104

**Key idea**
- Improves the core functions on **run-length BWT (RLBWT)** so they can be computed in **constant time with O(r) words**, where `r` is the number of BWT runs.
- Builds **OptBWTR**, supporting locate/count/extract in optimal time on highly repetitive strings.

**Reported improvement**
- The paper’s gain is not “better compression ratio than BWT,” but **much better query performance at the same repetitiveness-aware compressed scale**.
- It improves earlier `O(log log n)` query pieces to **O(1)** for key operations.

**Rust implementability**
- **Medium-high.**
- Bitvector/rank/select machinery and run indexing are absolutely implementable in Rust.
- More of a data-structure project than a toy codec.

**Relevance to 0.2589 → 0.15**
- **Medium.**
- If the corpus is repetitive, BWT-run structures are one of the cleanest ways to expose compressibility.
- Direct ratio benefit is indirect: better repetitive representation means better downstream coding.

**My read**
- This is one of the most important repetitive-data results in the period.
- Not glamorous, but highly relevant for any compressor that wants to exploit repeated structure aggressively.

---

### 5.2 BWT tunneling and related post-classic BWT work
**Source trail:** recent benchmark culture and repetitive-text literature; explicit modern accessible breakthrough papers were sparse in this pass.

**Key idea**
- BWT innovation after the classic era has focused on **run-aware representations**, better construction on repetitive data, and transformations that preserve reversibility while collapsing redundancy more aggressively.
- “Tunneling” and extended-BWT concepts remain interesting because they hint at **structural transforms that reduce redundant contexts before coding**.

**Reported improvement**
- I did **not recover a clean 2020-2026 accessible paper in this pass with a universally accepted new ratio record from BWT tunneling alone**.
- The measurable progress appears more incremental and data-structure-focused.

**Rust implementability**
- **Yes**, especially if scoped to offline block transforms and run-aware variants.

**Relevance to 0.2589 → 0.15**
- **Medium.**
- BWT still matters, but recent literature suggests it is strongest as one layer in a hybrid pipeline, not as the whole answer.

---

## 6. Context mixing improvements post-PAQ8

### 6.1 The big picture after PAQ8: engineering progress outran clean academic theory
**Source trail:** public benchmark pages (Matt Mahoney) and compressor lineage such as cmix, paq8px, fp8, zpaq.

**Key idea**
- Post-PAQ8 progress mostly came from **better mixers, model ensembles, image/text/binary specialization, and preprocessing**, not a single clean academic theorem.
- The community’s strongest practical codecs remain heavily **ensemble/context-mixing driven**.

**Reported improvement**
- On the **Silesia benchmark**, current 2026 leaderboard data from Matt Mahoney shows:
  - **paq8px_v210 -12L** at **27,987,907 bytes** total,
  - **precomp + cmix v21** at **28,261,094 bytes**,
  - **plain cmix v8** around **33,307,593 bytes**.
- This implies:
  - modern paq8px beats precomp+cmix by about **273,187 bytes**, roughly **~0.97% smaller** than precomp+cmix on total Silesia size;
  - precomp+cmix beats plain cmix v8 by roughly **5.05 MB**, around **~15.1% smaller**.
- So the large gains came from **preprocessing + context mixing** and long-term tuning, while newer PAQ-family work still ekes out further gains.

**Rust implementability**
- **Medium.**
- A full cmix/PAQ-class context mixer is implementable in Rust, but it is a serious modeling effort.
- The memory behavior, branchy bit-history logic, SSE-style mixers, and long tuning cycles are the hard part.

**Relevance to 0.2589 → 0.15**
- **Very high.**
- If the target is extremely aggressive, context mixing remains one of the few historically proven paradigms that actually pushes toward record territory on heterogeneous corpora.
- But reproducing PAQ/cmix quality is difficult; the literature is less neat than the code lineage.

**My read**
- If you want a practical route to 0.15 on mixed data, **a modernized context mixer or learned replacement for parts of it is still central**.
- The smartest move is probably **not** to re-create PAQ8 literally, but to combine:
  - hand-engineered contexts for cheap certainty,
  - learned predictors for long-range ambiguity,
  - strong preprocessing for obvious structures,
  - and a clean ANS/range backend.

---

### 6.2 Benchmark evidence from Large Text Compression Benchmark / Hutter-style lineage
**Source:** Matt Mahoney benchmark pages

**Key idea**
- The benchmark tables still show the long-running dominance of **CM / LZP+CM / PAQ-derived systems** on difficult text data.

**Reported improvement**
- On **enwik8**, the older benchmark page still lists:
  - **paq8hp12any -8**: 16,230,028 bytes
  - **zpaq ocmax.cfg,3**: 18,977,961 bytes
  - **bbb m100** (BWT): 20,847,290 bytes
  - **zip -9**: 36,445,443 bytes
- Relative to zip, paq8hp12any is about **55.5% smaller compressed output**.
- Relative to bzip2 (29,008,758), paq8hp12any is about **44.0% smaller**.
- Relative to bbb BWT (20,847,290), paq8hp12any is about **22.1% smaller**.

**Rust implementability**
- **Yes**, though costly.

**Relevance to 0.2589 → 0.15**
- **Very high**, because these numbers are proof that heterogeneous text still rewards sophisticated context models far beyond conventional codecs.

---

## 7. Practical compression records and benchmark state

### 7.1 Silesia corpus as of Jan 26, 2026
**Source:** Matt Mahoney’s Silesia benchmark page

**Top visible results**
1. **paq8px_v210 -12L** — **27,987,907 bytes**
2. **paq8px_v209 -12L** — **28,025,541 bytes**
3. **paq8px_v206 -12TL** — **28,241,197 bytes**
4. **precomp v0.4.7 -cn | cmix v21** — **28,261,094 bytes**
5. **plain cmix v8** — **33,307,593 bytes**

**Interpretation**
- **State-of-the-art still belongs to PAQ-family/context-mixing systems**, sometimes with preprocessors.
- **precomp + cmix** remains a very strong hybrid baseline.
- The gap from plain cmix to precomp+cmix is bigger than the gap from precomp+cmix to the latest paq8px, which suggests:
  - preprocessing and transforms matter a lot,
  - fine-grained model tuning still matters,
  - but late-stage gains are hard-earned.

**Relevance to 0.2589 → 0.15**
- **Directly relevant.**
- If your current 0.2589 is on a broad corpus resembling Silesia-style heterogeneity, then the benchmark evidence says the realistic path is still **hybrid and highly tuned**, not a single elegant algorithm.

---

### 7.2 Calgary corpus / enwik benchmarks
**Source:** Matt Mahoney’s data-compression pages and benchmark table

**Key observations**
- Calgary remains small and somewhat dated, but still useful for sanity checks.
- enwik8/enwik9 remain the best-known public targets for large-text compression comparisons.
- Context-mixing families dominate these text benchmarks; BWT and simpler LZ codecs trail.

**Representative numbers from accessible benchmark page**
- **Calgary tar**:
  - paq8hp12any -8: **594,269 bytes**
  - zpaq ocmax.cfg,3: **643,990 bytes**
  - bzip2 -9: **860,097 bytes**
  - zip -9: **1,023,101 bytes**
- **enwik8**:
  - paq8hp12any -8: **16,230,028 bytes**
  - zpaq ocmax.cfg,3: **18,977,961 bytes**
  - bzip2 -9: **29,008,758 bytes**
  - zip -9: **36,445,443 bytes**

**Interpretation**
- Classical fast codecs are nowhere near the frontier.
- BWT remains respectable, but **context-mixing and hybrid statistical modeling are still better** on difficult text.

**Relevance to 0.2589 → 0.15**
- **High.**
- Any project with an ambitious target should benchmark against this historical reality rather than against gzip/zstd alone.

---

## 8. Novel mathematical / structural frameworks

### 8.1 Repetitiveness measures as a framework, not just a benchmark metric
**Source trail:** LZHB paper and run-based index literature

**Key idea**
- Recent theory increasingly compares compressors against measures like:
  - grammar size (`g`, `g_rl`),
  - number of LZ phrases (`z`),
  - number of BWT runs (`r`),
  - constrained encodings like `z_HB`.
- This is important because it reframes compression from “which named codec wins?” to **which latent structural measure best captures the corpus**.

**Reported improvement**
- The improvement is conceptual/theoretical: some new encodings are shown to be bounded by or smaller than older repetitiveness measures for certain families.
- Example from LZHB: for some strings, `z_HB = o(g_rl)`.

**Rust implementability**
- **Yes**, if you design the compressor around structural analysis passes.

**Relevance to 0.2589 → 0.15**
- **Very high as research guidance.**
- If your current system cannot even estimate whether the corpus is “more run-BWT-like, more grammar-like, or more language-model-like,” then you are leaving gains on the table.

**My read**
- This is probably the most underappreciated insight from the 2020-2026 literature.
- The next breakthrough is likely a **meta-compressor** that chooses transforms/models based on structural diagnostics tied to these measures.

---

### 8.2 Domain-adaptive predictability as a framework
**Source:** 2024-2025 learned compression papers

**Key idea**
- The source distribution matters more than ever.
- LLM-generated text, code, structured logs, floating-point streams, and repetitive corpora each want different predictors.

**Reported improvement**
- Huge in-domain gains are reported, e.g. **20x on LLM-generated text**, **48% bit saving vs gzip** in a low-complexity learned text compressor, and strong gains in floating-point stream compression.

**Rust implementability**
- **Yes**, especially through a modular pipeline.

**Relevance to 0.2589 → 0.15**
- **Extremely high.**
- A single universal model probably will not get you there efficiently. A **domain-switching ensemble** might.

---

## 9. Special case worth noting: floating-point/time-series lossless compression

### 9.1 Elf / Elf+ (2023)
**Source:** arXiv:2306.16053

**Key idea**
- For floating-point streams, instead of classic XOR-only differencing, **erase selected low bits to create more trailing zeros**, then restore them losslessly through mathematically justified recovery.
- Adds better encoding of XORed values and significand counts.

**Reported improvement**
- The abstract claims strong wins across **22 datasets** versus **9 advanced competitors**, but the abstract does **not provide one universal percentage figure**.

**Rust implementability**
- **Very high.**
- This is exactly the kind of bit-level streaming transform Rust handles well.

**Relevance to 0.2589 → 0.15**
- **High if your corpus contains numeric streams; low otherwise.**
- Important because it reinforces the bigger lesson: **specialized front-ends beat generic codecs when the data type is known**.

---

## 10. Synthesis: what actually looks promising for a 0.2589 → 0.15 push?

### Most promising research-backed levers

#### A. Hybrid context-mixing + learned modeling
- The benchmark frontier still says **PAQ/cmix-style modeling works**.
- The 2024-2025 learned-compression work says **cheap autoregressive models can now add value**, especially on text-like or generated content.
- Best bet: use classical contexts as the low-latency backbone, and call learned predictors only on segments where they materially reduce cross-entropy.

#### B. Structural front-end before probabilistic coding
- LZ-End, height-bounded LZ, run-BWT, and grammar ideas all point to the same principle:
  - **expose repeated structure first**,
  - then entropy-code the residual uncertainty.
- This is more plausible than asking a neural model to rediscover all structure from raw bytes.

#### C. Strong preprocessors / type detectors
- Silesia results show how much **precomp + cmix** gains over plain cmix.
- Translation: even elite modelers benefit from being fed easier subproblems.
- For Rust, this probably means explicit modules for:
  - text/code,
  - XML/JSON/HTML,
  - executables/object files,
  - numeric streams,
  - images / known binary containers,
  - repetitive blocks.

#### D. A first-class modern entropy coder
- Not the main breakthrough, but essential.
- Use ANS or a strong range coder with careful normalization and cheap adaptation.

---

## 11. Practical recommendation ranking for Rust implementation

### Tier 1: highest-value and realistic
1. **Modern entropy coder** (rANS/range coder)
2. **Context-mixing backbone** with modular experts
3. **Preprocessing / type-specific transforms**
4. **LZ-End or strong LZ variant as a structural parser**

### Tier 2: high upside, more research risk
5. **Run-aware BWT / repetitive-data path**
6. **Grammar extraction / RePair-like macro-structure modeling**
7. **Selective learned predictor for text/code/LLM-like segments**

### Tier 3: compelling but likely too expensive as a first move
8. **Full neural end-to-end learned compressor for heterogeneous binary data**
9. **Heavy LLM-backed universal next-token compressor**

---

## 12. Bottom-line assessment of each area against the 0.15 target

| Area | Breakthrough level 2020-2026 | Rust feasibility | Helpfulness for 0.2589 → 0.15 |
|---|---|---:|---:|
| Neural / learned text compression | Real progress, strongest on text-like domains | Medium | High on the right corpus |
| New entropy coding methods | Incremental, important but not game-changing alone | High | Medium |
| LZ-family innovations | Strong algorithmic progress, practical parsing advances | High | Medium-High |
| Grammar compression | Strong structural theory, fewer headline benchmark wins | Medium | Medium |
| BWT innovations | Important in repetitive-data indexing/representation | Medium-High | Medium |
| Context mixing post-PAQ8 | Still the practical benchmark king on hard text | Medium | Very High |
| Practical records | Clear evidence hybrid modelers still win | N/A | Very High |
| Novel math frameworks | Repetitiveness measures are strategically important | High | Very High |

---

## 13. Final conclusions

1. **The literature does not show one magical 2020-2026 codec that makes classical compressors obsolete across all data.**
2. **The best empirical frontier remains hybrid and model-heavy**: PAQ/cmix lineage, preprocessing, and specialized modeling still dominate practical records.
3. **The best academic developments are enabling technologies**:
   - easier/practical **LZ-End**,
   - theoretically strong **height-bounded LZ**,
   - powerful **run-BWT indexes** for repetitive data,
   - and **learned low-complexity text compressors** that finally look somewhat deployable.
4. For a Rust project chasing **0.2589 → 0.15**, the most defensible roadmap is:
   - build a **strong modular entropy-coding core**,
   - add **structural parsers/transforms** for repeated data,
   - keep a **context-mixing backbone**,
   - layer in **selective learned prediction** only where the source type justifies it.

If the goal is a serious shot at **0.15**, the literature argues for a **meta-compressor** that measures the source, routes chunks through the right structural transform/predictor, and uses a strong final coder. That is more believable than betting everything on either “just neural” or “just another LZ variant.”

---

## References explicitly used in this survey

- Takaaki Nishimoto, et al. **Optimal-Time Queries on BWT-runs Compressed Indexes**. arXiv:2006.05104.
- Hideo Bannai, et al. **Height-bounded Lempel-Ziv encodings**. arXiv:2403.08209.
- **Computing the LZ-End parsing: Easy to implement and practically efficient**. arXiv:2409.07840.
- Yan Zhao, et al. **Leveraging RWKV for Learned Lossless Low-Complexity Text Compression (L3TC)**. arXiv:2412.16642.
- Yu Mao, et al. **Lossless Compression of Large Language Model-Generated Text via Next-Token Prediction**. arXiv:2505.06297.
- Yi Wu, et al. **Erasing-based lossless compression method for streaming floating-point time series**. arXiv:2306.16053.
- Matt Mahoney. **Silesia Open Source Compression Benchmark** and **Large Text Compression Benchmark / Data Compression Programs** pages, accessed March 2026.

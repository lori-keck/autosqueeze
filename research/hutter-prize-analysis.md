# Hutter Prize & Compression Frontier Analysis

*Research compiled March 2026*

---

## 1. Current Record

**Record holder:** Kaido Orav and Byron Knoll  
**Program:** fx2-cmix  
**Date:** September 3, 2024 (accepted October 8, 2024)  
**Total size:** 110,793,128 bytes (compressed file + decompressor)  
**Compression ratio:** ~11.08% of original 1 GB enwik9  
**Award:** €7,950  

The previous record was held by Kaido Orav alone with **fx-cmix** (February 2024) at 112,578,322 bytes (~11.26%), which won €6,911.

### Best Known Results on enwik8 (100 MB benchmark, not prize):
- **cmix v21** achieves 14,623,723 bytes (~14.6%)
- **nncp v3.2** (neural network) achieves 107,261,318 bytes on enwik9 (~10.73%) — best raw compression but doesn't qualify for Hutter Prize due to resource constraints

---

## 2. History of Improvements — Breakthrough Timeline

### enwik8 Era (2006–2019)

| Date | Author | Program | Total Size | Improvement | Award |
|------|--------|---------|------------|-------------|-------|
| Mar 2006 | Matt Mahoney | paq8f | 18,324,887 | Baseline | — |
| Sep 2006 | Alexander Rhatushnyak | paq8hp5 | 17,073,018 | 6.83% | €3,416 |
| May 2007 | Alexander Rhatushnyak | paq8hp12 | 16,481,655 | 3.47% | €1,732 |
| May 2009 | Alexander Rhatushnyak | decomp8 | 15,949,688 | 3.23% | €1,614 |
| Nov 2017 | Alexander Rhatushnyak | phda9 | 15,284,944 | 4.17% | €2,085 |
| Jul 2019 | Alexander Rhatushnyak | phda9v1.8 | 116,673,681* | — | No prize |

*\*Transitioned to enwik9 in 2020*

### enwik9 Era (2020–present)

| Date | Author | Program | Total Size | Award |
|------|--------|---------|------------|-------|
| May 2021 | Artemiy Margaritov | starlit | 115,352,938 | €9,000 |
| Jul 2023 | Saurabh Kumar | fast cmix | 114,156,155 | €5,187 |
| Feb 2024 | Kaido Orav | fx-cmix | 112,578,322 | €6,911 |
| Sep 2024 | Orav & Knoll | fx2-cmix | 110,793,128 | €7,950 |

**Total prize money distributed:** ~€29,945 of €500,000 pool (as of late 2025).

### Key Breakthrough Techniques at Each Stage:
1. **paq8hp5 (2006):** Custom dictionary built from enwik8 content, grouping semantically related words for better context modeling
2. **paq8hp12 (2007):** Expanded number of context models, refined dictionary organization
3. **decomp8 (2009):** Improved preprocessing and dictionary transforms
4. **phda9 (2017):** Advanced dictionary (phda series), refined context models, better SSE
5. **starlit (2021):** Improvements ported into cmix framework, LSTM enhancements
6. **fx-cmix / fx2-cmix (2024):** Integration of fxcm models by Kaitz (Kaido Orav), removal of older PAQ8HP model, addition of new fxcm model architecture, improved preprocessing dictionary

---

## 3. cmix Architecture (v21, September 2024)

### Overview
cmix is a lossless data compressor optimized purely for compression ratio at extreme CPU/memory cost. It compresses **one bit at a time** using probabilistic prediction + arithmetic coding.

### Three Main Components

#### A. Preprocessing
Transforms input into more compressible form before the main compression pass:
- **Binary executables:** x86 instruction rewriting (from paq8pxd)
- **Natural language text:** Word Replacing Transform (WRT) using an English dictionary (411,996 bytes from fx-cmix Hutter Prize entry). Words are replaced with 1–3 byte dictionary codes; uppercase encoded as special char + lowercase
- **Images:** BMP/TIFF detection and delta encoding (from paq8pxd)

#### B. Model Prediction
**cmix v21 uses 2,077 independent models.** Each model outputs a floating-point probability that the next bit is 1.

Model sources:
- **paq8l** models (Matt Mahoney's last PAQ8 version)
- **paq8pxd** models (community-maintained PAQ8 fork)
- **fxcm** models (Kaido Orav's custom models — added in v20/v21)
- **LSTM neural network** models (Byron Knoll's addition)

Model types include:
- N-gram context models (byte-level)
- Whole-word n-gram models
- Sparse context models (non-contiguous byte patterns)
- Analog models (high-order bits of 16-bit words)
- 2D context models (for tabular/image data)
- Specialized models for x86, BMP, TIFF, JPEG

#### C. Context Mixing (Multi-Layer)
Predictions from all 2,077 models are combined through a **hierarchical mixing architecture**:

1. **Bit-level mixers:** Multiple neural-network-based context mixers that combine model predictions using logistic-domain weighted averaging:
   - `P(1) = squash(Σ wᵢ · stretch(Pᵢ(1)))`
   - where `stretch(x) = ln(x/(1-x))` and `squash(x) = 1/(1+e⁻ˣ)`
   - Weights are updated online: `wᵢ ← wᵢ + η · xᵢ · (y - P(1))`
   
2. **LSTM Mixer:** A byte-level Long Short-Term Memory network trained via backpropagation through time (BPTT):
   - Adam optimizer with layer normalization
   - Learning rate decay
   - Coupled forget and input gates
   - This was a major innovation — applying deep learning to the mixing stage itself

3. **Secondary Symbol Estimation (SSE):** Post-processes the mixed prediction using context-indexed lookup tables that are continuously refined. Multiple SSE stages can be pipelined with different contexts.

4. **Final arithmetic coding:** The refined probability is fed to an arithmetic coder.

### Resource Requirements
- **Memory:** ~30 GB RAM recommended (31,650 MiB measured on enwik9)
- **Time:** ~623,000 seconds (~7.2 days) for enwik9 compression
- **Symmetry:** Compression and decompression take equal time
- **cmix v21 mixer neurons:** ~528,497 (measured on enwik6)

---

## 4. The PAQ Lineage: PAQ1 → PAQ6 → PAQ8 → cmix

### PAQ1 (January 2002, Matt Mahoney)
- **Innovation:** Context mixing — combining predictions from multiple context models instead of using a single best model (as PPM does)
- **Architecture:** 8 nonstationary context models, fixed-weight combining
- **Prediction format:** Pairs of bit counts (n₀, n₁) combined by weighted sum
- **Weights:** Fixed, ad-hoc (order-n contexts weighted n²)
- **Result:** Competitive with PPM compressors

### PAQ1SSE / PAQ2 (May 2003, Serge Osnach)
- **Innovation:** Secondary Symbol Estimation (SSE)
- **Mechanism:** Added a post-processing table between predictor and arithmetic coder. Table indexed by short context + current prediction, outputs refined prediction. Entries adjusted after each bit.
- **Impact:** Significant compression improvement

### PAQ3N (October 2003)
- **Innovation:** Sparse context model (non-contiguous byte contexts)

### PAQ4–PAQ6 (Nov–Dec 2003, Mahoney + Osnach)
- **Innovation:** Adaptive weight learning
- **Mechanism:** Weights adjusted via gradient descent to minimize prediction error:
  - `wᵢ ← wᵢ + [(S·n1ᵢ − S₁·nᵢ)/(S₀·S₁)] · error`
- **PAQ6** added analog model for multimedia data
- **Impact:** Now competitive with best PPM compressors; attracted broader community

### PAQAR (May–Jul 2004, Alexander Ratushnyak)
- **Innovations:** Many new models, multiple mixers with context-selected weights, SSE on each mixer output, x86 preprocessor
- **Impact:** Top-ranked compressor through end of 2004

### PAsQDa (Jan–Feb 2005, Przemyslaw Skibinski)
- **Innovation:** English dictionary preprocessor (Word Replacing Transform)

### PAQ7 (December 2005, Mahoney)
- **Innovation:** Neural network mixing replaces weighted averaging
- **Mechanism:** Models output probabilities (not count pairs). Combined in logistic domain via single-layer neural network
- **Also added:** BMP, TIFF, JPEG models

### PAQ8 Series (January 2006+)
- **PAQ8A (Jan 2006):** x86 model restored, bug fixes
- **PAQ8F (Feb 2006):** Memory-efficient context model, indirect context model
- **PAQ8G (Mar 2006):** Dictionary preprocessor reintegrated
- **PAQ8H (Mar 2006):** Base for Hutter Prize entries
- **PAQ8HP1–HP12 (2006–2007):** Alexander Rhatushnyak's Hutter Prize series
  - Custom dictionaries with semantic word grouping
  - Progressively more context models
  - Won 4 Hutter Prize awards
- **PAQ8L (Mar 2007):** Mahoney's last version, became foundation for cmix
- **PAQ8PX series (2008+):** Community-maintained, continued adding models and improvements

### cmix (December 2013+, Byron Knoll)
- **Innovation:** Massively scaled context mixing + LSTM neural network mixer
- **Built on:** paq8l and paq8pxd code, but dramatically expanded
- **Key additions over PAQ8:**
  1. Scaled from dozens to **2,077 models**
  2. Added **LSTM byte-level mixer** (deep learning meets compression)
  3. Multiple mixer layers (hierarchical mixing)
  4. Integrated preprocessing from multiple sources
  5. Incorporated fxcm models (Kaido Orav) in v20–v21
- **Trade-off:** Requires 32 GB RAM and days of compute, but achieves state-of-the-art ratios

### The Lineage Summary
```
PAQ1 (2002) — context mixing concept, fixed weights
  → PAQ2 (2003) — SSE post-processing
    → PAQ4-6 (2003) — adaptive weights
      → PAQ7 (2005) — neural network mixing
        → PAQ8 (2006+) — specialized models, dictionaries, x86 preprocessing
          → PAQ8HP series — Hutter Prize winners (dictionaries, semantic grouping)
          → PAQ8PX series — community extensions
            → cmix (2013+) — 2000+ models, LSTM mixer, hierarchical mixing
              → fx-cmix / fx2-cmix — current Hutter Prize record holders
```

---

## 5. Hutter Prize Scoring

### The Formula
The score is simply: **Total Size = Compressed File Size + Decompressor Size**

Where:
- **Compressed file:** The compressed form of enwik9 (or previously enwik8)
- **Decompressor:** A standalone executable (Win32 or Linux) that can reconstruct enwik9 from the compressed file

### Eligibility
To win a prize, a submission must achieve **Total Size ≤ 99% of the previous winning entry's Total Size** — i.e., at least a 1% improvement.

### Prize Calculation
- €5,000 per 1% improvement
- Total pool: €500,000
- Minimum claimable: €5,000
- After each award, the formula baseline is reset to the new winner

### Constraints (current rules)
- **Runtime:** Under 100 CPU-hours on a single core
- **Memory:** No more than 10 GB RAM
- **No GPUs or distributed computing**
- **Source code** must be released under a free software license
- **30-day public comment period** before award

### Why This Metric?
The total-size metric prevents trivial cheating: you can't just ship a huge decompressor that contains most of enwik9 as a lookup table (because its size counts against you). The compressor must genuinely discover patterns. This connects directly to Kolmogorov complexity — the total size approximates the algorithmic information content of enwik9.

---

## 6. Compression ↔ Intelligence (Hutter's Thesis)

### The Core Argument
Marcus Hutter's thesis: **Optimal data compression is equivalent to optimal prediction, which is equivalent to general intelligence.**

### The Theoretical Chain

1. **Solomonoff Induction (1964):** The ideal way to predict the next observation is to consider all possible programs that could have generated the observations so far, weight them by 2^(-length), and combine their predictions. Shorter programs get exponentially more weight — this is Occam's Razor formalized.

2. **Kolmogorov Complexity:** The true information content of a string is the length of the shortest program that produces it. This is uncomputable in general, but compression approximates it.

3. **AIXI (Hutter, 2000–2005):** The theoretically optimal agent for any computable environment. AIXI uses Solomonoff's prior to model the environment and selects actions maximizing expected future rewards. At each step, it implicitly favors the shortest programs consistent with observations — i.e., it compresses.

4. **The Equivalence:**
   - To compress text well → you must predict the next character well
   - To predict text well → you must understand grammar, semantics, world knowledge, reasoning
   - Understanding these things IS intelligence (or at least a major component of it)
   - Therefore: better compression → better understanding → more intelligence

### Why Wikipedia Text?
Wikipedia is a deliberately chosen target because it's:
- **Diverse:** Covers science, history, math, culture, biography, etc.
- **Complex:** Requires understanding of natural language, XML markup, mathematical notation, multilingual content
- **Representative:** Approximates "human knowledge" broadly
- **Large enough** to require genuine modeling rather than memorization

### Critiques and Nuances
- **Compression ≠ full intelligence:** Compression tests *passive* understanding. Intelligence also involves active reasoning, planning, creativity, embodiment
- **LLMs and compression:** Large language models are essentially compressors (predicting next tokens). ChatGPT-class models could theoretically achieve excellent compression but fail Hutter Prize constraints (model size exceeds the data)
- **Lossy vs lossless:** Human cognition is largely lossy compression. The Hutter Prize demands lossless, which is a stricter but different problem

---

## 7. Theoretical vs. Achieved Compression Gap

### Shannon's Estimates for English
Claude Shannon estimated English text contains approximately **0.6–1.3 bits per character** of information (entropy).

### Theoretical Limits for enwik8/enwik9
- enwik8 = 10⁸ bytes = 8 × 10⁸ bits raw
- At Shannon's low estimate (0.6 bits/char): theoretical limit ≈ **7.5 MB** for enwik8, **75 MB** for enwik9
- At Shannon's high estimate (1.3 bits/char): ≈ **16.25 MB** for enwik8, **162.5 MB** for enwik9

### Current Best Achieved
- **enwik8:** cmix v21 = 14,623,723 bytes (~14.6 MB) → **~1.17 bits/char**
- **enwik9:** nncp v3.2 = 107,261,318 bytes (~107 MB) → **~0.86 bits/char** (raw compression, no prize constraints)
- **enwik9 (Hutter Prize):** fx2-cmix = 110,793,128 bytes → **~0.89 bits/char** (including decompressor)

### Analysis
- Current compressors have **already beaten Shannon's upper estimate** (1.3 bits/char)
- We're operating in the range Shannon predicted as a lower bound (~0.6–1.0 bits/char)
- **The gap to the theoretical floor is small but persistent.** Getting from ~0.9 to ~0.6 bits/char would require another ~33% reduction — an enormous challenge
- enwik9 includes XML markup, non-English text, and mathematical/technical content that may have different entropy characteristics than Shannon's pure English estimates
- The "true" Kolmogorov complexity of enwik9 is unknowable, but we're likely within 30–50% of it

### Historical Compression Rate
- From 2006 to 2024, compression improved from 18.3 MB to ~14.6 MB on enwik8 — about **20% improvement over 18 years** (~1.2% per year)
- On enwik9, from ~116 MB (2021) to ~111 MB (2024) — about **4.5% in 3 years** (~1.5% per year)
- Gains are clearly decelerating as we approach theoretical limits

---

## 8. Recent Innovations (2024–2026)

### fx2-cmix (Sep 2024) — Current Hutter Prize Record
- **Authors:** Byron Knoll + Kaido Orav (Kaitz)
- **Key changes from fx-cmix:**
  - Removed the older PAQ8HP model entirely
  - Added the **fxcm model** (Kaido Orav's custom context-mixing framework)
  - Improvements ported from fx2-cmix repository
  - Better preprocessing dictionary
- **Significance:** Shows that model architecture pruning (removing old models) + replacement with better ones continues to yield gains

### cmix v21 (Sep 2024)
- Removed PAQ8HP model
- Added fxcm model from Kaitz
- Improvements from fx2-cmix integrated into mainline cmix

### NNCP (Neural Network Compression Program)
- **Best raw enwik9 compression:** 107,261,318 bytes (10.73%)
- Uses Transformer-based neural networks for prediction
- Requires ~7.6 GB RAM, but massive compute (~242,000 ns/byte)
- Doesn't qualify for Hutter Prize (computational constraints, model size)
- **Significance:** Demonstrates that neural approaches can beat context mixing on raw compression, but the model itself is too large to be practical under prize constraints

### Transformer-Based Compressors
- **TRACE (2022):** Fast Transformer-based lossless compressor
- **ts_zip / LLMZip concepts:** Using pre-trained LLMs as the prediction model for arithmetic coding
- **Challenge:** The model weights (billions of parameters) dwarf the data being compressed. A 7B parameter model is ~14 GB — you can't include it in a self-extracting archive for 1 GB of data
- **Research direction:** Distillation, quantization, and architecture search to create tiny but powerful predictors

### Key Trends (2024–2026)
1. **Hybrid architectures:** Combining classical context mixing (2000+ handcrafted models) with learned components (LSTM, small Transformers)
2. **Model pruning:** Removing models that don't carry their weight (literally — fx2-cmix removed PAQ8HP)
3. **Better dictionaries:** Continued refinement of preprocessing dictionaries specific to Wikipedia content
4. **Neural mixing:** Using neural networks not just as models but as the mixing/combining layer
5. **Architecture search:** Exploring which combination of models yields the best ensemble for a given memory budget
6. **Compute-bounded innovation:** The Hutter Prize constraints (100 CPU-hours, 10 GB RAM, no GPU) force practical ingenuity rather than brute-force scaling

### The LLM Elephant in the Room
Large language models (GPT-4, Claude, etc.) are fundamentally compression engines — they predict next tokens, which is equivalent to compression. However:
- Their model weights are orders of magnitude larger than the data being compressed
- They require GPUs
- They don't produce lossless reconstruction (generation ≠ decompression)
- The Hutter Prize explicitly constrains resources to prevent "just use a bigger model" solutions

The open research question: **Can you distill LLM-level understanding into a compressor small enough to beat cmix under Hutter Prize constraints?** This is arguably the most interesting frontier in compression research right now.

---

## Summary: What Pushes the Frontier

| Era | Key Technique | Impact |
|-----|--------------|--------|
| 2002 | Context mixing (PAQ1) | Beat PPM paradigm |
| 2003 | Secondary Symbol Estimation | Major accuracy boost |
| 2003 | Adaptive weights | Models self-tune |
| 2004 | Multiple mixers + x86 preprocessing | Architecture depth |
| 2005 | Neural network mixing | Learned combination |
| 2005–2007 | Dictionary preprocessing | Semantic compression |
| 2006–2017 | Custom dictionaries (PAQ8HP) | Domain-specific gains |
| 2014+ | Massive model ensemble (cmix) | 2000+ models |
| 2016+ | LSTM mixer (cmix) | Deep learning in mixing |
| 2021 | Starlit improvements | Refinements across stack |
| 2023–2024 | fxcm models + model pruning | Better model selection |
| Future? | LLM distillation / tiny Transformers | Open question |

The pattern is clear: each generation added a new **type** of intelligence to the compression stack. Fixed rules → learned weights → neural mixing → deep learning → massive ensembles → selective pruning. The frontier now is whether neural approaches (Transformers, distilled LLMs) can be made small and fast enough to compete under real-world constraints.

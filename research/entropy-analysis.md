# Entropy Analysis: Information-Theoretic Limits of the autosqueeze Corpus

**Date:** 2026-03-15  
**Current overall ratio:** 0.2589  
**World record (cmix):** ~0.12  
**Target:** Identify where bits are being wasted and how to close the gap

---

## Executive Summary

Our corpus is 2,599,439 bytes. Current compressed size: ~673,046 bytes (ratio 0.2589). The theoretical minimum at order-3 empirical entropy varies by file, but the two biggest opportunities are:

1. **Moby Dick (57% of compressed output)** — we're at 0.3015 vs order-3 limit of 0.2798. But cmix achieves ~0.16 on English text using order-6+ context mixing, meaning the true limit is far below order-3.
2. **Random data (15% of compressed output)** — incompressible by definition. This is dead weight. Every byte we waste here directly inflates our ratio.

The files already well-compressed (JSON at 0.0403, repetitive at 0.0009, logs at 0.0670) contribute only ~4.3% of compressed output. Optimizing them further has diminishing returns.

**The math:** If we could compress Moby Dick to 0.16 (cmix-level), source code to 0.12, CSV to 0.15, and logs to 0.04 while random stays at 1.0, the overall ratio would drop to ~0.147. That's our realistic ceiling with no-external-crates constraint.

---

## Corpus Overview

| File | Size (bytes) | % of Corpus | Current Ratio | Compressed (bytes) | % of Compressed |
|------|-------------|-------------|---------------|-------------------|-----------------|
| Moby Dick | 1,276,266 | 49.1% | 0.3015 | 384,794 | 57.2% |
| JSON | 170,239 | 6.5% | 0.0403 | 6,861 | 1.0% |
| Repetitive | 100,000 | 3.8% | 0.0009 | 90 | 0.0% |
| Random | 100,000 | 3.8% | 1.0001 | 100,010 | 14.9% |
| Source code | 292,843 | 11.3% | 0.2357 | 69,023 | 10.3% |
| CSV | 331,265 | 12.7% | 0.2724 | 90,237 | 13.4% |
| Logs | 328,826 | 12.6% | 0.0670 | 22,031 | 3.3% |
| **TOTAL** | **2,599,439** | | **0.2589** | **673,046** | |

---

## Per-File Entropy Analysis

### 1. Moby Dick (01_moby_dick.txt) — **THE #1 TARGET**

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 4.5979 | 0.5747 | 0.3015 | -0.2732 (beating order-0!) |
| 1 | 3.4736 | 0.4342 | 0.3015 | -0.1327 (beating order-1!) |
| 2 | 2.8311 | 0.3539 | 0.3015 | -0.0524 (beating order-2!) |
| 3 | 2.2380 | 0.2798 | 0.3015 | +0.0217 |

**Analysis:** We're already beating the order-2 empirical entropy — meaning our LZ77 with optimal parsing is capturing 2nd-order patterns effectively. But we're 2.17% above the order-3 limit. The real issue is that English text has structure far beyond order-3:

- **Word-level patterns:** "the", "and", "whale", "Captain Ahab" are predictable at character level but need 5-10 character contexts to exploit fully.
- **Grammatical structure:** After "the" comes a noun/adjective. After a period comes a capital letter. These are order-4+ patterns.
- **Thematic repetition:** Moby Dick repeats vocabulary ("whale", "sea", "ship", "white") — a word-level dictionary would help.
- **What cmix does differently:** Uses order-6 to order-12 context models, mixed with word-level models and match models. The gap between our 0.3015 and cmix's ~0.16 is almost entirely in higher-order context modeling.

**What we're wasting:** ~0.14 ratio points. On 1.276MB, that's ~179KB of unnecessarily encoded information. Every byte of this is higher-order linguistic structure we're not capturing.

**Specific opportunities:**
- Order-4 through order-8 contexts would capture word completions ("whal" → "e", "Capt" → "ain")
- A secondary PPM (Prediction by Partial Matching) model could adapt contexts of varying length
- Even a crude order-4 context model on top of LZ77 residuals could save 5-10%

### 2. Structured JSON (02_structured.json) — ALREADY EXCELLENT

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 4.1300 | 0.5162 | 0.0403 | -0.4759 |
| 1 | 1.6719 | 0.2090 | 0.0403 | -0.1687 |
| 2 | 0.8629 | 0.1079 | 0.0403 | -0.0676 |
| 3 | 0.6949 | 0.0869 | 0.0403 | -0.0466 |

**Analysis:** We're compressing to 0.0403 — which is **below the order-3 empirical entropy limit of 0.0869.** This means LZ77 is capturing structure that goes far beyond 3-byte contexts. This makes sense: the JSON has a rigid repeating template structure:

```json
{"id": N, "name": "user_N", "email": "userN@example.com", "score": N, "active": true/false, "tags": [...]}
```

LZ77 with a 1MB window can reference entire previous records as matches, effectively encoding each record as "copy previous record, then patch these N bytes." This is optimal for templated data — LZ77 excels here.

**What we're wasting:** Essentially nothing. At 0.0403 (only 6.8KB compressed), further optimization saves at most a few hundred bytes. Not worth the effort.

### 3. Repetitive Binary (03_repetitive.bin) — SOLVED

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 3.3219 | 0.4152 | 0.0009 | — |
| 1 | 0.0000 | 0.0000 | 0.0009 | +0.0009 |

**Analysis:** This file is a 10-byte repeating pattern ("ABCDEFGHIJ" × 10,000). Order-1 entropy is literally 0 — each byte perfectly predicts the next. LZ77 crushes this to 90 bytes (just the initial pattern plus a reference). We're at the practical minimum — 90 bytes is just the overhead of encoding the match structure and headers.

**What we're wasting:** 90 bytes of overhead. Irrelevant.

### 4. Random Binary (04_random.bin) — **THE DEAD WEIGHT**

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 7.9985 | 0.9998 | 1.0001 | +0.0003 |

**Analysis:** This is (pseudo-)random data with near-maximum entropy. Order-0 entropy is 7.9985 bits/byte out of a possible 8.0 — essentially incompressible. Our ratio of 1.0001 means we're adding 10 bytes of overhead (mode byte + headers), which is optimal.

**Important note on higher-order measurements:** The order-2 and order-3 estimates from naive calculation showed artificially low entropy (0.14 and 0.001). This is a **well-known estimation bias** — with 100K bytes and 65,536 possible 2-byte contexts, most contexts appear only 1-2 times, making the conditional entropy collapse toward zero. **This is statistical noise, not real structure.** The true entropy at all orders ≈ 8.0 bits/byte.

**What we're wasting:** Nothing. This is the information-theoretic floor. You cannot compress random data.

**However:** This 100KB file at ratio 1.0 contributes 14.9% of our total compressed output. If the benchmark rules allow detecting incompressible blocks and storing them raw with minimal overhead, we're already doing that optimally. The only "win" here would be if the data had hidden structure we're not seeing (it doesn't — I checked).

### 5. Source Code (05_source_code.c) — **HIGH-VALUE TARGET**

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 5.2066 | 0.6508 | 0.2357 | -0.4151 |
| 1 | 3.7024 | 0.4628 | 0.2357 | -0.2271 |
| 2 | 2.4222 | 0.3028 | 0.2357 | -0.0671 |
| 3 | 1.5473 | 0.1934 | 0.2357 | +0.0423 |

**Analysis:** This is the Linux kernel scheduler (sched/core.c) — highly structured C code. We're beating order-2 entropy but sitting 4.23% above the order-3 limit.

**What we're wasting:**
- **Identifier repetition:** Variable names like `rq->`, `p->`, `cpu`, `sched_`, `task_struct` repeat throughout but at distances beyond simple order-3 capture. A word/token-level model would help.
- **Syntactic patterns:** C has rigid syntax — `if (`, `for (`, `return `, `struct `, function signatures. These are order-4 to order-8 patterns.
- **Indentation correlations:** Indent level predicts code block type. Current literal transforms (XorDelta, MTF) may help but don't fully capture this.
- **Comment structure:** `/* ... */` and `//` comments have different statistics from code — a block-level detector could switch models.

**Specific opportunities:**
- Preprocessing: tokenize identifiers and replace with short codes before compression
- Higher-order context models would capture keyword completions
- BWT might be winning here (I see the compressor tries both LZ77 and BWT) — which path is it taking?

### 6. Sensor CSV (06_sensor_data.csv) — **HIGH-VALUE TARGET**

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 3.5210 | 0.4401 | 0.2724 | -0.1677 |
| 1 | 3.0570 | 0.3821 | 0.2724 | -0.1097 |
| 2 | 2.5719 | 0.3215 | 0.2724 | -0.0491 |
| 3 | 2.0321 | 0.2540 | 0.2724 | +0.0184 |

**Analysis:** Numeric CSV data. Only 29 unique bytes (digits, comma, period, newline). We're 1.84% above order-3 but the real waste is structural:

**What we're wasting:**
- **Timestamps are sequential:** `1700000000, 1700000060, 1700000120...` — the timestamps increment by 60. Delta encoding would reduce these to tiny residuals (mostly "60").
- **Sensor IDs are small integers:** Values 1-20, stored as ASCII text. A binary representation would be 1 byte instead of 1-2.
- **Floating point values have structure:** Temperature (12-28°C), humidity (35-74%), pressure (997-1020 hPa) all have constrained ranges. Delta encoding within each column would exploit the slow drift of sensor readings.
- **Column separation:** Currently all columns are compressed together. Separating columns (all timestamps together, all temperatures together, etc.) and compressing each stream would dramatically improve ratios because same-type values cluster better.

**This file is the strongest case for domain-specific preprocessing.** However, the rules say "no hardcoding for specific files." The preprocessing needs to be general-purpose — detecting numeric sequences, column structure, and delta-encodable patterns automatically.

**Specific opportunities:**
- Automatic delta encoding detection: if consecutive values in a byte stream are close, switch to delta mode
- Column reordering/separation for detected CSV structure
- Numeric-aware encoding: detect ASCII numbers and encode their values more efficiently

### 7. Log Files (07_logs.txt) — ALREADY GOOD

| Order | Entropy (bits/byte) | Min Ratio | Current Ratio | Gap |
|-------|-------------------|-----------|---------------|-----|
| 0 | 5.0500 | 0.6313 | 0.0670 | -0.5643 |
| 1 | 1.9741 | 0.2468 | 0.0670 | -0.1798 |
| 2 | 0.8103 | 0.1013 | 0.0670 | -0.0343 |
| 3 | 0.4719 | 0.0590 | 0.0670 | +0.0080 |

**Analysis:** Log lines have extreme template repetition — similar to JSON. Each line follows:
```
[2024-01-XXT...] [LEVEL] [component] Message with Nms
```

LZ77 crushes this because entire phrases like `Request processed in `, `Connection timeout after `, `[INFO]`, etc. repeat constantly. We're only 0.8% above the order-3 limit.

**What we're wasting:** Minimal. The remaining gap is in timestamp digits (dates/times that vary) and numeric values (milliseconds). Delta encoding of timestamps would help marginally.

---

## Where Are the Bits Going? — Prioritized Opportunities

### Priority 1: Moby Dick (potential savings: ~179KB, ~27% of compressed output)

The biggest single opportunity. English text at 0.3015 vs cmix-level 0.16 means we're encoding roughly **2× the bits necessary** for this file.

**Root cause:** LZ77+Huffman/BWT are fundamentally limited to patterns within their window and block structure. They don't model language. What we need:

1. **PPM (Prediction by Partial Matching):** Adaptively select context length from order-0 to order-8. When "whal" predicts "e" with high confidence, use a short code. When context is ambiguous, fall back to shorter contexts. This alone could drop text compression to ~0.20-0.22.

2. **Context mixing (simplified):** Run 2-3 models simultaneously (order-1, order-4, order-8) and blend their predictions. Even a crude 3-model mixer could approach 0.18-0.20 on text.

3. **Word-level modeling:** Maintain a dictionary of seen words, predict whole words after spaces. English has ~10K common words; predicting the right one in context is powerful.

### Priority 2: Source Code (potential savings: ~12-25KB)

Source code at 0.2357 could realistically reach 0.12-0.15 with:
- Token-aware compression (identifiers as units)
- Higher-order contexts that capture keyword/identifier patterns
- Indentation-aware modeling

### Priority 3: CSV Data (potential savings: ~25-40KB)

CSV at 0.2724 could reach 0.10-0.15 with:
- Automatic column separation and per-column compression
- Delta encoding for sequential numeric data
- Detecting that timestamps are arithmetic progressions

### Priority 4: Accept Random Tax

Random data at 1.0001 is a fixed cost. 100KB out of 2.6MB = 3.8% of corpus. At ratio 1.0, it contributes 14.9% of compressed output. This is immovable. Any overall ratio target must account for this floor.

**Impact on overall ratio floor:** Even if every other file compressed to 0, the random file alone gives us a floor ratio of 100,000/2,599,439 = **0.0385**. Realistically, with all files at near-theoretical limits, we might reach **0.10-0.15**.

---

## The Path from 0.2589 to 0.12

| Milestone | Target Ratio | Key Change | Expected Savings |
|-----------|-------------|------------|-----------------|
| Current | 0.2589 | LZ77 + BWT + Range coding | — |
| Phase A | ~0.22 | Order-4 PPM for text/source/logs | ~100KB |
| Phase B | ~0.18 | Context mixing (3-4 models) | ~100KB |
| Phase C | ~0.16 | Column separation + delta for CSV | ~50KB |
| Phase D | ~0.14 | Order-8+ contexts, word models | ~50KB |
| Theoretical limit | ~0.10 | Perfect prediction of all structure | — |

### Why 0.12 Is Extremely Hard

The cmix approach that achieves 0.12:
- Uses **hundreds** of context models (byte, word, sparse, match, etc.)
- Neural-network-based mixing of model predictions  
- Multiple passes with different model configurations
- Requires thousands of lines of sophisticated code
- Runs at ~2 KB/s

To get close in a stdlib-only Rust implementation, we need a **simplified but principled** version:

1. **Adaptive order-N context model** (PPM-style): Maintain hash tables for orders 1-8. For each byte, look up the longest matching context, predict the next byte distribution, encode with arithmetic/range coding. Fall back to shorter contexts when long contexts have insufficient statistics (escape mechanism).

2. **Secondary match model:** Like LZ77 but used as a prediction source. "The longest recent match suggests the next byte is X" — weight this alongside the context model.

3. **Context mixing:** Given predictions from (1) and (2), combine them using logistic mixing: convert probabilities to log-odds, weighted sum, convert back. Update weights based on which model predicted better.

This is essentially a minimal PAQ-style compressor. It's feasible in stdlib-only Rust but would be a significant implementation effort.

---

## Key Insight: Compression = Prediction

The gap between our current ratio and the theoretical limit is exactly the gap between our prediction accuracy and perfect prediction. For each file:

| File | What we predict well | What we miss |
|------|---------------------|--------------|
| Moby Dick | 2-byte patterns, repeated phrases | Word completions, grammar, topic |
| JSON | Template structure | Nothing significant |
| Repetitive | The repeating pattern | Nothing |
| Random | Nothing (correctly) | Nothing (correctly) |
| Source | Keyword patterns, indentation | Identifier names, code structure |
| CSV | Header/delimiter patterns | Numeric sequences, column correlations |
| Logs | Template phrases | Timestamp increments, value patterns |

**The single most impactful change:** Replace or augment LZ77/BWT with a PPM-style context model for the entropy coding backend. This would improve Moby Dick, source code, CSV, and logs simultaneously — the four files that together contribute 84% of compressed output.

---

## Appendix: Raw Entropy Measurements

### Methodology
- Order-0: H₀ = -Σ p(x) log₂ p(x), where p(x) = count(x)/N
- Order-1: H₁ = -Σ p(c) Σ p(x|c) log₂ p(x|c), conditional on 1-byte context
- Order-2: H₂ = conditional on 2-byte context
- Order-3: H₃ = conditional on 3-byte context
- Min ratio = H / 8.0 (entropy per byte / bits per byte)

### Caveats
- Higher-order estimates are biased downward on small files due to context sparsity
- For random.bin, order-2+ estimates are unreliable (most contexts seen only 1-2 times) — true entropy ≈ 8.0 at all orders
- Empirical entropy is an upper bound on compressibility; the true compressibility can be lower if there are patterns beyond the measured order
- These measurements don't account for adaptive models that can outperform fixed-order entropy

### Full Table

| File | H₀ | H₁ | H₂ | H₃ | Unique Bytes |
|------|-----|-----|-----|-----|-------------|
| Moby Dick | 4.598 | 3.474 | 2.831 | 2.238 | 115 |
| JSON | 4.130 | 1.672 | 0.863 | 0.695 | 41 |
| Repetitive | 3.322 | 0.000 | 0.000 | 0.000 | 10 |
| Random | 7.999 | ~8.0* | ~8.0* | ~8.0* | 256 |
| Source | 5.207 | 3.702 | 2.422 | 1.547 | 96 |
| CSV | 3.521 | 3.057 | 2.572 | 2.032 | 29 |
| Logs | 5.050 | 1.974 | 0.810 | 0.472 | 50 |

\* Random data estimates at order-2+ are corrected from biased measurements. True conditional entropy ≈ order-0 entropy for independent random bytes.

### File Characteristics

| File | Structure Type | LZ77 Advantage | Context Model Advantage |
|------|---------------|----------------|------------------------|
| Moby Dick | Natural language | Good (repeated phrases) | **Excellent** (word/grammar prediction) |
| JSON | Rigid template | **Excellent** (record copying) | Moderate |
| Repetitive | Pure repetition | **Perfect** | Perfect |
| Random | No structure | None | None |
| Source | Semi-structured | Good (identifier reuse) | **Very good** (syntax prediction) |
| CSV | Columnar numeric | Moderate (header/delimiter) | **Very good** (numeric prediction) |
| Logs | Template + variation | **Very good** (phrase reuse) | Good |

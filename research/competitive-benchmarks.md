# Competitive Benchmarks: autosqueeze vs standard compressors

Benchmark goal: compare **autosqueeze** against established general-purpose compressors on the exact project corpus, using each compressor at a high-compression setting.

- autosqueeze: current `cargo run --release --bin benchmark`
- gzip: `gzip -9`
- bzip2: `bzip2 -9`
- xz: `xz -9`
- zstd: `zstd -19`
- Ratio = `compressed_size / original_size` (**lower is better**)
- Corpus path: `/Users/lorikeck/github/autosqueeze/corpus/`
- Date: 2026-03-15

## Corpus overview

| File | Type | Size |
|---|---|---:|
| `01_moby_dick.txt` | English prose text | 1,276,266 B |
| `02_structured.json` | Structured JSON | 170,239 B |
| `03_repetitive.bin` | Extremely repetitive synthetic data | 100,000 B |
| `04_random.bin` | Random / incompressible bytes | 100,000 B |
| `05_source_code.c` | C source code | 292,843 B |
| `06_sensor_data.csv` | Numeric CSV / tabular data | 331,265 B |
| `07_logs.txt` | Repetitive structured logs | 328,826 B |

## Per-file comparison

| File | autosqueeze | gzip -9 | bzip2 -9 | xz -9 | zstd -19 | Winner |
|---|---:|---:|---:|---:|---:|---|
| `01_moby_dick.txt` | **0.3015** | 0.4008 | 0.3049 | 0.3275 | 0.3308 | autosqueeze |
| `02_structured.json` | 0.0403 | 0.0611 | 0.0287 | **0.0172** | 0.0217 | xz |
| `03_repetitive.bin` | 0.0009 | 0.0026 | 0.0007 | 0.0017 | **0.0003** | zstd |
| `04_random.bin` | **1.0001** | 1.0007 | 1.0066 | 1.0007 | 1.0002 | autosqueeze |
| `05_source_code.c` | 0.2357 | 0.2871 | **0.2297** | 0.2436 | 0.2483 | bzip2 |
| `06_sensor_data.csv` | 0.2724 | 0.3414 | **0.2611** | 0.2784 | 0.3221 | bzip2 |
| `07_logs.txt` | 0.0670 | 0.1111 | **0.0591** | 0.0869 | 0.0894 | bzip2 |

## Head-to-head takeaways

### Where autosqueeze is competitive

**Clearly competitive / winning:**

1. **English text (`01_moby_dick.txt`)**
   - autosqueeze is the best result in the whole field here: **0.3015**
   - Slightly beats bzip2 (**0.3049**) and more clearly beats xz/zstd/gzip.
   - That matters because plain natural-language text is a classic benchmark domain.

2. **Random / incompressible data (`04_random.bin`)**
   - autosqueeze is effectively tied for best at **not making random data bigger**: **1.0001**.
   - Everyone loses a tiny bit due to headers/metadata, but autosqueeze is basically the least bad here.

3. **Source code (`05_source_code.c`)**
   - autosqueeze: **0.2357**
   - best competitor: bzip2 at **0.2297**
   - Gap is small: only about **2.6% worse relative to bzip2** and better than gzip/xz/zstd.
   - This is firmly in “competitive” territory.

4. **CSV / sensor data (`06_sensor_data.csv`)**
   - autosqueeze: **0.2724**
   - best competitor: bzip2 at **0.2611**
   - Gap is modest: about **4.3% worse relative to bzip2**.
   - Better than gzip, xz, and zstd.

5. **Logs (`07_logs.txt`)**
   - autosqueeze: **0.0670**
   - best competitor: bzip2 at **0.0591**
   - Gap is noticeable but not catastrophic: about **13.3% worse relative to bzip2**.
   - Still beats gzip/xz/zstd comfortably.

### Where autosqueeze loses badly

1. **Structured JSON (`02_structured.json`)**
   - autosqueeze: **0.0403**
   - xz: **0.0172**
   - zstd: **0.0217**
   - bzip2: **0.0287**
   - autosqueeze is about **2.35× larger than xz** on this file.
   - This is the single worst miss in the corpus.

2. **Extremely repetitive synthetic data (`03_repetitive.bin`)**
   - autosqueeze: **0.0009**
   - zstd: **0.0003**
   - bzip2: **0.0007**
   - autosqueeze is still excellent in absolute terms, but it is about **2.65× larger than zstd**.
   - So this is a “lose badly on an edge case” situation, even though the end result is still tiny.

## Overall pattern

autosqueeze currently looks **strong on mixed real-world text-like material** and **surprisingly respectable as a general-purpose compressor**, especially considering it is still a research system.

If the question is “can it hang with classic tools on normal corpora?” the answer is **yes**.

If the question is “is it state of the art on highly structured and ultra-redundant data?” the answer is **not yet**.

## What each competitor does that autosqueeze currently doesn’t

This is the real competitive gap analysis.

### gzip

**#1 thing gzip does that autosqueeze doesn’t:**
- **Very cheap, robust LZ77-style match finding with a format optimized around backreferences.**

Why it matters:
- gzip is old and not ratio-optimal, but it has a brutally effective baseline: find repeated byte substrings in a sliding window and encode them cheaply.
- autosqueeze already beats gzip on every corpus file here, so gzip is not the main threat anymore.
- Still, gzip’s lesson is that a simple backreference engine sets a hard floor for general-purpose usefulness.

Net: **autosqueeze has already cleared gzip territory.**

### bzip2

**#1 thing bzip2 does that autosqueeze doesn’t:**
- **Burrows–Wheeler Transform (BWT) style block sorting that groups similar contexts together before entropy coding.**

Why it matters:
- BWT is extremely good on source, logs, CSV, and many structured text files because it converts medium-range redundancy into long runs and low-entropy symbol neighborhoods.
- That maps almost perfectly to the places where bzip2 beats autosqueeze: code, CSV, and logs.
- bzip2’s win profile suggests autosqueeze is still leaving money on the table when the redundancy is more about **reordered context clustering** than raw repeated phrases.

Net: **bzip2 is the strongest practical ratio competitor in this corpus overall.**

### xz

**#1 thing xz does that autosqueeze doesn’t:**
- **Large-window LZMA-style dictionary compression with very strong range-coded modeling of literals and matches.**

Why it matters:
- xz absolutely crushes the JSON file because JSON has lots of repeated keys, punctuation patterns, and long-distance structural redundancy.
- LZMA’s large dictionary plus sophisticated literal/match context modeling is exactly the kind of thing that turns repetitive structured syntax into tiny output.
- autosqueeze’s JSON weakness strongly suggests it is not yet exploiting **long repeated structural templates** as aggressively as LZMA.

Net: **xz exposes the biggest “structured redundancy” gap.**

### zstd

**#1 thing zstd does that autosqueeze doesn’t:**
- **Modern sequence coding around strong match finding + entropy coding tuned for repeated patterns and small literals, with excellent handling of highly redundant blocks.**

Why it matters:
- zstd’s standout win here is the repetitive synthetic file.
- That usually means its parser and sequence encoder are recognizing and coding repeated sequences extremely efficiently, with minimal framing overhead.
- Even when zstd does not win on ratio here, it remains the benchmark for **practical engineering quality**: excellent parser, entropy coding, block design, and dictionaries.

Net: **zstd’s biggest lesson is sequence coding efficiency on obvious redundancy.**

## Bottom line

### Where autosqueeze is already legit
- **Beats gzip everywhere in this corpus.** That’s a real milestone, not fluff.
- **Wins outright on large English text.**
- **Very competitive on code and CSV.**
- **Reasonable on logs.**
- **Handles random data cleanly with almost no blow-up.**

### Where autosqueeze needs work
- **Structured JSON is the biggest weakness by far.**
- **Hyper-repetitive data is not as compact as zstd/bzip2 can make it.**
- The pattern says autosqueeze needs better machinery for:
  1. **long-range structural matches**
  2. **context clustering / transforms**
  3. **more efficient coding of repetitive sequences**

## Strategic read

If I were prioritizing research from this benchmark alone:

1. **Attack structured data first**
   - JSON is the ugliest loss and the clearest opportunity.
   - Anything that better captures repeated field names, punctuation scaffolding, and record templates could close a massive gap fast.

2. **Steal ideas from BWT/LZMA/zstd, not gzip**
   - gzip is already handled.
   - The live competition is really **bzip2 on text-like structured files**, **xz on deeply structured syntax**, and **zstd on ultra-repetitive sequences**.

3. **Preserve the current strength on prose/code**
   - autosqueeze already has a genuinely good profile there.
   - Don’t wreck the text/code wins chasing one synthetic benchmark.

## Raw source measurements

### autosqueeze benchmark output used

| File | Ratio |
|---|---:|
| `01_moby_dick.txt` | 0.3015 |
| `02_structured.json` | 0.0403 |
| `03_repetitive.bin` | 0.0009 |
| `04_random.bin` | 1.0001 |
| `05_source_code.c` | 0.2357 |
| `06_sensor_data.csv` | 0.2724 |
| `07_logs.txt` | 0.0670 |
| **overall** | **0.258911** |

### External compressor measurements used

| File | gzip -9 | bzip2 -9 | xz -9 | zstd -19 |
|---|---:|---:|---:|---:|
| `01_moby_dick.txt` | 0.400782 | 0.304870 | 0.327527 | 0.330833 |
| `02_structured.json` | 0.061061 | 0.028671 | 0.017176 | 0.021699 |
| `03_repetitive.bin` | 0.002580 | 0.000690 | 0.001680 | 0.000340 |
| `04_random.bin` | 1.000670 | 1.006560 | 1.000720 | 1.000160 |
| `05_source_code.c` | 0.287092 | 0.229720 | 0.243625 | 0.248317 |
| `06_sensor_data.csv` | 0.341428 | 0.261123 | 0.278363 | 0.322138 |
| `07_logs.txt` | 0.111123 | 0.059122 | 0.086854 | 0.089397 |

## Verdict

autosqueeze is **not a toy anymore**. It already beats gzip cleanly and can win on natural-language text. But the benchmark also makes the next frontier obvious: **structured redundancy modeling**. Right now, xz and bzip2 are eating its lunch there.
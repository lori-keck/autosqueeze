#!/bin/bash
# prepare.sh — Create a test corpus for compression benchmarking
# Run this once to set up the corpus/ directory

set -e

mkdir -p corpus

echo "Creating test corpus..."

# 1. English text — the classic compression test
echo "  → Downloading Project Gutenberg text (Moby Dick)..."
curl -sL "https://www.gutenberg.org/cache/epub/2701/pg2701.txt" -o corpus/01_moby_dick.txt

# 2. Structured data — JSON
echo "  → Generating JSON data..."
python3 -c "
import json
data = []
for i in range(1000):
    data.append({
        'id': i,
        'name': f'user_{i}',
        'email': f'user{i}@example.com',
        'score': i * 17 % 100,
        'active': i % 3 == 0,
        'tags': ['tag_a', 'tag_b', 'tag_c'] if i % 2 == 0 else ['tag_d']
    })
print(json.dumps(data, indent=2))
" > corpus/02_structured.json

# 3. Repetitive data — lots of repeated patterns
echo "  → Generating repetitive data..."
python3 -c "
import sys
pattern = b'ABCDEFGHIJ' * 100
sys.stdout.buffer.write(pattern * 100)
" > corpus/03_repetitive.bin

# 4. Random-ish binary — hard to compress
echo "  → Generating pseudo-random binary..."
python3 -c "
import sys
state = 42
data = bytearray(100000)
for i in range(100000):
    state = (state * 1103515245 + 12345) & 0x7fffffff
    data[i] = (state >> 16) & 0xff
sys.stdout.buffer.write(data)
" > corpus/04_random.bin

# 5. Source code — structured text with keywords
echo "  → Generating source code sample..."
curl -sL "https://raw.githubusercontent.com/torvalds/linux/master/kernel/sched/core.c" -o corpus/05_source_code.c 2>/dev/null || \
python3 -c "
# Fallback: generate synthetic source code
lines = []
for i in range(2000):
    indent = '    ' * (i % 4)
    if i % 10 == 0:
        lines.append(f'fn function_{i}(arg: &[u8]) -> Vec<u8> {{')
    elif i % 10 == 9:
        lines.append('}')
    elif i % 5 == 0:
        lines.append(f'{indent}let result = process(input, {i});')
    elif i % 7 == 0:
        lines.append(f'{indent}// TODO: optimize this section')
    else:
        lines.append(f'{indent}data.push(value + {i % 256});')
print('\n'.join(lines))
" > corpus/05_source_code.c

# 6. CSV data — tabular
echo "  → Generating CSV data..."
python3 -c "
import random
random.seed(42)
print('timestamp,sensor_id,temperature,humidity,pressure')
for i in range(10000):
    ts = 1700000000 + i * 60
    sid = random.randint(1, 20)
    temp = round(20 + random.gauss(0, 5), 2)
    hum = round(50 + random.gauss(0, 15), 2)
    pres = round(1013 + random.gauss(0, 10), 2)
    print(f'{ts},{sid},{temp},{hum},{pres}')
" > corpus/06_sensor_data.csv

# 7. Log file — semi-structured, repetitive prefixes
echo "  → Generating log data..."
python3 -c "
import random
random.seed(123)
levels = ['INFO', 'DEBUG', 'WARN', 'ERROR']
modules = ['auth', 'api', 'db', 'cache', 'worker']
for i in range(5000):
    ts = f'2024-01-{(i % 28) + 1:02d}T{i % 24:02d}:{i % 60:02d}:00Z'
    level = random.choice(levels)
    mod = random.choice(modules)
    msg = f'Request processed in {random.randint(1, 500)}ms'
    if level == 'ERROR':
        msg = f'Connection timeout after {random.randint(5000, 30000)}ms'
    print(f'[{ts}] [{level}] [{mod}] {msg}')
" > corpus/07_logs.txt

echo ""
echo "Corpus created:"
echo "─────────────────────────────────────"
du -sh corpus/*
echo "─────────────────────────────────────"
total=$(du -sh corpus | cut -f1)
echo "Total: $total"
echo ""
echo "Ready! Run: cargo run --release --bin benchmark"

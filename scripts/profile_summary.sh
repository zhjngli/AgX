#!/usr/bin/env bash
set -euo pipefail

# Summarize profiling results from profile.sh output.
# Groups by image+preset, computes median of each stage, prints a table.
#
# Usage: ./scripts/profile_summary.sh [input_file]
#   input_file: JSON file from profile.sh (default: profile_results.json)
#
# Requires: python3 (for JSON processing)

INPUT="${1:-profile_results.json}"

if [ ! -f "$INPUT" ]; then
    echo "Error: $INPUT not found. Run ./scripts/profile.sh first."
    exit 1
fi

python3 -c "
import json, sys
from collections import defaultdict
from statistics import median

with open('$INPUT') as f:
    data = json.load(f)

# Group by (image, preset)
groups = defaultdict(list)
for entry in data:
    key = (entry['image'], entry['preset'])
    groups[key].append(entry)

# Compute medians
results = []
for (image, preset), entries in groups.items():
    stages = defaultdict(list)
    totals = []
    for e in entries:
        totals.append(e['total_ms'])
        for stage, ms in e['stages'].items():
            stages[stage].append(ms)

    med_total = median(totals)
    med_stages = {s: median(vals) for s, vals in stages.items()}
    results.append((image, preset, med_total, med_stages))

# Sort by total time descending
results.sort(key=lambda x: -x[2])

# Collect all stage names in consistent order
stage_order = ['decode', 'white_balance_exposure', 'dehaze', 'denoise',
               'linear_to_srgb_and_per_pixel', 'detail', 'grain',
               'vignette_and_srgb_to_linear', 'encode']

# Print summary
print()
print('=' * 80)
print('RENDER PERFORMANCE PROFILE SUMMARY (median of runs)')
print('=' * 80)

for image, preset, total, stages in results:
    dims = groups[(image, preset)][0].get('dimensions', ['?', '?'])
    print(f'\n--- {image} + {preset} ({dims[0]}x{dims[1]}) ---')
    print(f'  Total: {total:.1f} ms')
    print()
    for stage in stage_order:
        ms = stages.get(stage, 0.0)
        if ms > 0.0:
            pct = (ms / total * 100) if total > 0 else 0
            bar = '#' * int(pct / 2)
            print(f'  {stage:>35s}: {ms:8.1f} ms  ({pct:5.1f}%)  {bar}')
    # Print any stages not in the standard order
    for stage, ms in sorted(stages.items()):
        if stage not in stage_order and ms > 0.0:
            pct = (ms / total * 100) if total > 0 else 0
            print(f'  {stage:>35s}: {ms:8.1f} ms  ({pct:5.1f}%)')

print()
print('=' * 80)
print('TOP BOTTLENECKS (stages consuming >15% of total)')
print('=' * 80)
bottlenecks = defaultdict(list)
for image, preset, total, stages in results:
    for stage, ms in stages.items():
        pct = (ms / total * 100) if total > 0 else 0
        if pct > 15:
            bottlenecks[stage].append((image, preset, ms, pct))

for stage, occurrences in sorted(bottlenecks.items(), key=lambda x: -len(x[1])):
    avg_pct = sum(p for _, _, _, p in occurrences) / len(occurrences)
    print(f'\n  {stage} (appears in {len(occurrences)}/{len(results)} combos, avg {avg_pct:.0f}%)')
    for image, preset, ms, pct in sorted(occurrences, key=lambda x: -x[3])[:3]:
        print(f'    {image} + {preset}: {ms:.1f} ms ({pct:.1f}%)')

print()
"

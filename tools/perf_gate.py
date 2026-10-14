"""Compares a release build against tools/perf_baseline.json.

Binary size growth over 5% fails; decode p95 or peak RSS growth over 10% only warns,
because shared CI runners are too noisy to gate timings. Update the baseline by
copying an artifact's perf-<platform>.json into tools/perf_baseline.json on purpose.

Usage: perf_gate.py <platform> <binary> <bench.json> <out.json>
"""
import json
import os
import sys
from pathlib import Path

platform, binary, bench, out = sys.argv[1:5]
current = {'binary_bytes': Path(binary).stat().st_size, **json.loads(Path(bench).read_text())}
Path(out).write_text(json.dumps(current, indent=2) + '\n')
baseline = json.loads((Path(__file__).parent / 'perf_baseline.json').read_text()).get(platform)

rows, failed = [], False
for key, limit, fatal in [('binary_bytes', 5, True), ('decode_ms_p95', 10, False), ('peak_rss_mib', 10, False)]:
    now = current[key]
    if baseline is None or key not in baseline:
        rows.append(f'| {key} | {now:.6g} | – | – |')
        continue
    change = (now - baseline[key]) / baseline[key] * 100
    rows.append(f'| {key} | {now:.6g} | {baseline[key]:.6g} | {change:+.1f}% |')
    if change > limit:
        print(f'::{"error" if fatal else "warning"}::{platform} {key} grew {change:.1f}% (limit {limit}%)')
        failed |= fatal
if baseline is None:
    print(f'::notice::No {platform} baseline; add {out} under "{platform}" in tools/perf_baseline.json to start gating')

summary = os.environ.get('GITHUB_STEP_SUMMARY')
if summary:
    with open(summary, 'a', encoding='utf-8') as f:
        f.write(f'### Performance ({platform})\n\n| Metric | Current | Baseline | Change |\n|---|---|---|---|\n')
        f.write('\n'.join(rows) + '\n\n')
sys.exit(1 if failed else 0)

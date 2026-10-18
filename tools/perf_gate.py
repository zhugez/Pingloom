"""Release regression gate. Sizes/counters fail above 5%; timing/RAM warn above 10%.

Usage: perf_gate.py <platform> <binary> <bench.json> <out.json> [extra.json ...]
Baseline changes are deliberate: copy verified CI metrics, never auto-rebaseline.
"""
import json
import math
import os
import sys
from pathlib import Path

ARTIFACTS = {
    'windows': {'installer_bytes': '*-setup.exe', 'portable_bytes': 'pingloom.exe'},
    'macos': {'macos_zip_bytes': 'pingloom-macos-universal.zip'},
    'linux': {'deb_bytes': '*.deb', 'tarball_bytes': 'pingloom-linux-x64.tar.gz'},
}


def compare(platform, current, baseline):
    rows, messages, failed = [], [], False
    for key in sorted(current.keys() | baseline.keys()):
        now, before = current.get(key), baseline.get(key)
        if key == 'bench_profile':
            if now not in ('release', 'bench-ci'):
                raise ValueError('unknown benchmark profile')
            rows.append(f'| {key} | {now} | {before or "–"} | metadata |')
            continue
        if now is None:
            messages.append(f'::error::{platform} baseline metric {key} is missing from current run')
            rows.append(f'| {key} | missing | {before} | error |')
            failed = True
            continue
        if not isinstance(now, (int, float)) or isinstance(now, bool) or not math.isfinite(now) or now < 0:
            raise ValueError(f'{key} must be a finite non-negative number')
        if before is None:
            rows.append(f'| {key} | {now:.6g} | – | unbaselined |')
            continue
        if not isinstance(before, (int, float)) or isinstance(before, bool) or not math.isfinite(before) or before < 0:
            raise ValueError(f'baseline {key} must be a finite non-negative number')
        if before == 0:
            messages.append(f'::notice::{platform} {key} has a zero baseline; comparison skipped')
            rows.append(f'| {key} | {now:.6g} | 0 | skipped |')
            continue
        change = (now - before) / before * 100
        rows.append(f'| {key} | {now:.6g} | {before:.6g} | {change:+.1f}% |')
        fatal = key.endswith(('_bytes', '_count'))
        limit = 5 if fatal else 10
        if change > limit:
            messages.append(f'::{"error" if fatal else "warning"}::{platform} {key} grew {change:.1f}% (limit {limit}%)')
            failed |= fatal
    return rows, messages, failed


def collect(platform, binary, bench, out, extras):
    current = json.loads(Path(bench).read_text(encoding='utf-8'))
    for extra in extras:
        metrics = json.loads(Path(extra).read_text(encoding='utf-8'))
        if current.keys() & metrics.keys():
            raise ValueError(f'duplicate metrics in {extra}')
        current.update(metrics)
    sizes = {'binary_bytes': Path(binary).stat().st_size}
    for key, pattern in ARTIFACTS[platform].items():
        files = list(Path(out).parent.glob(pattern))
        if len(files) != 1:
            raise ValueError(f'{key}: expected one {pattern} in {Path(out).parent}, found {len(files)}')
        sizes[key] = files[0].stat().st_size
    for key, directory in [('ui_dist_bytes', Path('ui/dist')), ('asset_bytes', Path('assets'))]:
        if not directory.is_dir():
            raise ValueError(f'{directory} directory is missing')
        sizes[key] = sum(f.stat().st_size for f in directory.rglob('*') if f.is_file())
    if current.keys() & sizes.keys():
        raise ValueError('benchmark/extra JSON cannot override measured sizes')
    return {**current, **sizes, 'bench_profile': os.environ.get('PINGLOOM_BENCH_PROFILE', 'release')}


def main(args):
    if len(args) < 4 or args[0] not in ARTIFACTS:
        raise ValueError(__doc__)
    platform, binary, bench, out, *extras = args
    current = collect(platform, binary, bench, out, extras)
    baseline = json.loads(Path(__file__).with_name('perf_baseline.json').read_text(encoding='utf-8')).get(platform, {})
    rows, messages, failed = compare(platform, current, baseline)
    Path(out).write_text(json.dumps(current, indent=2) + '\n', encoding='utf-8')
    if not baseline:
        messages.append(f'::notice::No {platform} baseline; regression thresholds are not enforced')
    print('\n'.join(messages))
    summary = f'### Performance ({platform})\n\n| Metric | Current | Baseline | Change |\n|---|---|---|---|\n' + '\n'.join(rows) + '\n\n'
    print(summary)
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf-8') as stream:
            stream.write(summary)
    return int(failed)


if __name__ == '__main__':
    try:
        sys.exit(main(sys.argv[1:]))
    except (OSError, ValueError, TypeError, AttributeError) as error:
        print(f'::error::Performance gate: {error}')
        sys.exit(1)

"""Run: python3 -m unittest discover -s tools -p 'test_perf_gate.py'."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import perf_gate


class PerfGateTest(unittest.TestCase):
    def test_limits_missing_and_zero_baselines(self):
        baseline = {'binary_bytes': 100, 'decode_ms_p95': 100, 'peak_rss_mib': 100, 'render_resize_count': 100}
        rows, messages, failed = perf_gate.compare('windows', {**baseline, 'binary_bytes': 105}, baseline)
        self.assertFalse(failed)
        self.assertTrue(perf_gate.compare('windows', {**baseline, 'binary_bytes': 106}, baseline)[2])
        self.assertTrue(perf_gate.compare('macos', {**baseline, 'render_resize_count': 106}, baseline)[2])
        _, messages, failed = perf_gate.compare('macos', {**baseline, 'decode_ms_p95': 111, 'peak_rss_mib': 111}, baseline)
        self.assertFalse(failed)
        self.assertEqual(sum('::warning::' in m for m in messages), 2)
        _, messages, failed = perf_gate.compare('windows', {}, {'binary_bytes': 100})
        self.assertTrue(failed)
        self.assertIn('missing', messages[0])
        _, messages, failed = perf_gate.compare('windows', {'binary_bytes': 5}, {'binary_bytes': 0})
        self.assertFalse(failed)
        self.assertIn('::notice::', messages[0])
        self.assertFalse(perf_gate.compare('macos', {'new_ms': 2}, {})[2])
        for value in [float('nan'), float('inf'), -1, True, '100']:
            with self.assertRaises(ValueError):
                perf_gate.compare('windows', {'binary_bytes': value}, {})

    def test_collects_packages_merges_extras_and_detects_asset_growth(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            previous = Path.cwd()
            try:
                os.chdir(root)
                Path('release').mkdir()
                Path('ui/dist').mkdir(parents=True)
                Path('assets').mkdir()
                Path('release/pingloom.exe').write_bytes(b'x' * 100)
                Path('release/test-setup.exe').write_bytes(b'x' * 50)
                Path('assets/art').write_bytes(b'x' * 100)
                Path('ui/dist/index.html').write_bytes(b'ui')
                Path('bench.json').write_text(json.dumps({'decode_ms_p95': 5}))
                Path('idle.json').write_text(json.dumps({'idle_ws_mib': 10}))
                current = perf_gate.collect('windows', 'release/pingloom.exe', 'bench.json', 'release/perf.json', ['idle.json'])
                self.assertEqual(current['installer_bytes'], 50)
                self.assertEqual(current['ui_dist_bytes'], 2)
                self.assertEqual(current['idle_ws_mib'], 10)
                Path('assets/art').write_bytes(b'x' * 106)
                grown = perf_gate.collect('windows', 'release/pingloom.exe', 'bench.json', 'release/perf.json', [])
                baseline = {k: v for k, v in current.items() if k != 'idle_ws_mib'}
                _, messages, failed = perf_gate.compare('windows', grown, baseline)
                self.assertTrue(failed)
                self.assertTrue(any('asset_bytes grew' in m for m in messages))
                Path('idle.json').write_text(json.dumps({'binary_bytes': 1}))
                with self.assertRaises(ValueError):
                    perf_gate.collect('windows', 'release/pingloom.exe', 'bench.json', 'release/perf.json', ['idle.json'])
                Path('idle.json').write_text(json.dumps({'decode_ms_p95': 1}))
                with self.assertRaises(ValueError):
                    perf_gate.collect('windows', 'release/pingloom.exe', 'bench.json', 'release/perf.json', ['idle.json'])
                Path('release/test-setup.exe').unlink()
                with self.assertRaises(ValueError):
                    perf_gate.collect('windows', 'release/pingloom.exe', 'bench.json', 'release/perf.json', [])
            finally:
                os.chdir(previous)

    def test_five_megabyte_asset_regression_fails_both_release_platforms(self):
        baselines = json.loads(Path(perf_gate.__file__).with_name('perf_baseline.json').read_text())
        for platform in ['windows', 'macos']:
            baseline = baselines[platform]
            current = {**baseline, 'asset_bytes': baseline['asset_bytes'] + 5_000_000}
            _, messages, failed = perf_gate.compare(platform, current, baseline)
            self.assertTrue(failed)
            self.assertTrue(any('::error::' in m and 'asset_bytes grew' in m for m in messages))

    def test_cli_writes_metrics_and_job_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            out = root / 'perf.json'
            summary = root / 'summary.md'
            with patch.object(perf_gate, 'collect', return_value={'binary_bytes': 1}), patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(perf_gate.main(['windows', 'binary', 'bench', str(out)]), 1)
            self.assertEqual(json.loads(out.read_text())['binary_bytes'], 1)
            self.assertIn('missing', summary.read_text())


if __name__ == '__main__':
    unittest.main()

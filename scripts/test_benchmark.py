"""Regression checks for baseline compatibility and performance budgets."""

import copy
import unittest

from benchmark import CASES, compare, validate_metrics


class BenchmarkComparisonTests(unittest.TestCase):
    def setUp(self):
        self.baseline = {
            "schema": 1, "environment": {"cpu": "fixture", "profile": "release"},
            "fixtures": {"source": "sha256"},
            "metrics": [{"case": name, "samples_ms": [10, 10, 10], "median_ms": 10, "p90_ms": 10}
                        for name in sorted(CASES)],
        }
        self.current = copy.deepcopy(self.baseline)

    def test_equal_results_pass(self):
        self.assertEqual(compare(self.current, self.baseline, 0.25, 1), [])

    def test_median_and_p90_regressions_fail(self):
        metric = self.current["metrics"][0]
        metric.update(samples_ms=[13, 13, 20], median_ms=13, p90_ms=20)
        self.assertEqual(len(compare(self.current, self.baseline, 0.25, 1)), 2)

    def test_noise_floor_avoids_submillisecond_false_alarm(self):
        for metric in self.current["metrics"]:
            metric.update(samples_ms=[10.5] * 3, median_ms=10.5, p90_ms=10.5)
        self.assertEqual(compare(self.current, self.baseline, 0.01, 1), [])

    def test_incompatible_baseline_is_rejected(self):
        for field in ("schema", "environment", "fixtures"):
            with self.subTest(field=field):
                bad = copy.deepcopy(self.baseline)
                bad[field] = None
                with self.assertRaises(ValueError):
                    compare(self.current, bad, 0.25, 1)

    def test_missing_duplicate_or_invalid_metrics_are_rejected(self):
        metrics = self.current["metrics"]
        bad_inputs = [[], metrics[:-1], metrics + metrics[:1]]
        for invalid in (float("nan"), -1, float("inf")):
            bad = copy.deepcopy(metrics)
            bad[0]["samples_ms"] = [invalid]
            bad_inputs.append(bad)
        bad = copy.deepcopy(metrics)
        bad[0]["median_ms"] = 0
        bad_inputs.append(bad)
        for bad in bad_inputs:
            with self.assertRaises(ValueError):
                validate_metrics(bad)

    def test_different_sample_counts_are_rejected(self):
        self.current["metrics"][0]["samples_ms"].append(10)
        with self.assertRaisesRegex(ValueError, "sample count"):
            compare(self.current, self.baseline, 0.25, 1)


if __name__ == "__main__":
    unittest.main()

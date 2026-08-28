#!/usr/bin/env python3

from __future__ import annotations

import json
import itertools
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import benchmark_record


def run_record(**updates: object) -> dict[str, object]:
    record: dict[str, object] = {
        "schema_version": 1,
        "run_id": "run-001",
        "created_at": "2026-08-28T12:00:00Z",
        "stage": "raw_asr",
        "source_run_id": "",
        "configuration": "parakeet-control",
        "app_commit": "a" * 40,
        "adapter": "parakeet",
        "adapter_revision": "a" * 40,
        "runtime": "test-runtime",
        "model": "test-model",
        "model_revision": "b" * 40,
        "decoder": "greedy",
        "decoder_revision": "test-decoder-v1",
        "formatter": "none",
        "formatter_revision": "none",
        "formatting_profile": "none",
        "normalizer_revision": "koett-words-v1",
        "platform": "linux",
        "os_version": "test-os",
        "hardware": "test-hardware",
        "power_mode": "ac",
        "target": "headless",
        "fixture_id": "technical-001",
        "fixture_sha256": "c" * 64,
        "repeat": 1,
        "success": True,
        "failure_code": "",
        "failure_message": "",
        "audio_seconds": 5.0,
        "download_ms": None,
        "cold_load_ms": None,
        "prewarm_ms": None,
        "first_prediction_ms": None,
        "engine_ms": 80.0,
        "stop_to_final_ms": 90.0,
        "stop_to_visible_ms": 110.0,
        "final_sample_to_visible_ms": 130.0,
        "peak_rss_bytes": 100_000_000,
        "energy_joules": None,
        "installed_footprint_bytes": 200_000_000,
        "cpu_seconds": None,
        "timing_scope": "target_visible",
        "hypothesis": "FluidAudio converts forty eight kilohertz audio to sixteen kilohertz.",
    }
    record.update(updates)
    return record


def fixture(**updates: object) -> dict[str, object]:
    item: dict[str, object] = {
        "id": "technical-001",
        "category": "technical",
        "speaker_id": "local-speaker-001",
        "publishable": False,
        "expected_empty": False,
        "reference": "FluidAudio converts forty-eight kilohertz audio to sixteen kilohertz.",
        "sha256": "c" * 64,
        "protected_terms": ["FluidAudio", "forty-eight kilohertz", "sixteen kilohertz"],
    }
    item.update(updates)
    return item


class BenchmarkRecordTests(unittest.TestCase):
    def test_alignment_keeps_the_legacy_edit_total(self) -> None:
        def legacy_distance(reference: list[str], hypothesis: list[str]) -> int:
            row = list(range(len(hypothesis) + 1))
            for reference_index, reference_word in enumerate(reference, 1):
                next_row = [reference_index]
                for hypothesis_index, hypothesis_word in enumerate(hypothesis, 1):
                    next_row.append(
                        min(
                            next_row[-1] + 1,
                            row[hypothesis_index] + 1,
                            row[hypothesis_index - 1]
                            + (reference_word != hypothesis_word),
                        )
                    )
                row = next_row
            return row[-1]

        sequences = [
            list(sequence)
            for length in range(5)
            for sequence in itertools.product(("A", "B"), repeat=length)
        ]
        for reference in sequences:
            for hypothesis in sequences:
                self.assertEqual(
                    benchmark_record.alignment(reference, hypothesis)["errors"],
                    legacy_distance(reference, hypothesis),
                )

    def test_alignment_reports_each_word_error_type(self) -> None:
        result = benchmark_record.alignment(
            ["KEEP", "DELETE", "CHANGE"],
            ["KEEP", "INSERT", "ALTER"],
        )

        self.assertEqual(result["substitutions"], 2)
        self.assertEqual(result["deletions"], 0)
        self.assertEqual(result["insertions"], 0)
        self.assertEqual(result["errors"], 2)

        deletion = benchmark_record.alignment(["KEEP", "DELETE"], ["KEEP"])
        insertion = benchmark_record.alignment(["KEEP"], ["KEEP", "INSERT"])
        self.assertEqual(deletion["deletions"], 1)
        self.assertEqual(insertion["insertions"], 1)

    def test_score_keeps_protected_phrases_and_boundary_words_separate(self) -> None:
        result = benchmark_record.score_record(run_record(), fixture())

        self.assertEqual(result["wer"], 0.0)
        self.assertEqual(result["protected_total"], 3)
        self.assertEqual(result["protected_missed"], 0)
        self.assertEqual(result["protected_error"], 0.0)
        self.assertEqual(result["first_word_expected"], "FLUIDAUDIO")
        self.assertEqual(result["first_word_actual"], "FLUIDAUDIO")
        self.assertTrue(result["first_word_match"])
        self.assertEqual(result["final_word_expected"], "KILOHERTZ")
        self.assertTrue(result["final_word_match"])

    def test_visible_latencies_are_derived_from_one_monotonic_trace(self) -> None:
        result = benchmark_record.score_record(
            run_record(
                stop_to_final_ms=None,
                stop_to_visible_ms=None,
                final_sample_to_visible_ms=None,
                stop_request_ns=1_000_000_000,
                last_audio_callback_ns=1_010_000_000,
                model_final_result_ns=1_090_000_000,
                stable_text_ns=1_120_000_000,
            ),
            fixture(),
        )

        self.assertEqual(result["stop_to_final_ms"], 90.0)
        self.assertEqual(result["stop_to_visible_ms"], 120.0)
        self.assertEqual(result["final_sample_to_visible_ms"], 110.0)

    def test_conflicting_or_reversed_latency_evidence_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "conflicts with monotonic trace"):
            benchmark_record.score_record(
                run_record(
                    stop_to_visible_ms=100.0,
                    stop_request_ns=1_000_000_000,
                    stable_text_ns=1_120_000_000,
                ),
                fixture(),
            )
        with self.assertRaisesRegex(ValueError, "before"):
            benchmark_record.score_record(
                run_record(
                    stop_to_visible_ms=None,
                    stop_request_ns=1_120_000_000,
                    stable_text_ns=1_000_000_000,
                ),
                fixture(),
            )
        with self.assertRaisesRegex(ValueError, "stable_text_ns.*model_final_result_ns"):
            benchmark_record.score_record(
                run_record(
                    stop_to_final_ms=200.0,
                    stop_to_visible_ms=100.0,
                    final_sample_to_visible_ms=90.0,
                    stop_request_ns=1_000_000_000,
                    last_audio_callback_ns=1_010_000_000,
                    model_final_result_ns=1_200_000_000,
                    stable_text_ns=1_100_000_000,
                ),
                fixture(),
            )

    def test_failed_run_is_preserved_and_cannot_look_like_valid_silence(self) -> None:
        result = benchmark_record.score_record(
            run_record(
                success=False,
                failure_stage="capture",
                failure_code="capture_interrupted",
                failure_message="device changed",
                hypothesis="",
            ),
            fixture(expected_empty=True, reference="", protected_terms=[]),
        )

        self.assertFalse(result["success"])
        self.assertEqual(result["failure_code"], "capture_interrupted")
        self.assertFalse(result["safety_passed"])
        self.assertTrue(result["empty_match"])

    def test_formatting_run_requires_a_raw_source_run(self) -> None:
        with self.assertRaisesRegex(ValueError, "source_run_id"):
            benchmark_record.score_record(
                run_record(stage="formatting", source_run_id=""),
                fixture(),
            )

    def test_jsonl_and_tsv_use_the_same_record_fields(self) -> None:
        scored = benchmark_record.score_record(run_record(), fixture())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            jsonl = root / "runs.jsonl"
            tsv = root / "runs.tsv"

            benchmark_record.write_records(jsonl, [scored])
            benchmark_record.write_records(tsv, [scored])

            self.assertEqual(benchmark_record.read_records(jsonl), [scored])
            self.assertEqual(benchmark_record.read_records(tsv), [scored])

    def test_summary_keeps_raw_and_formatting_stages_separate(self) -> None:
        raw = benchmark_record.score_record(run_record(), fixture())
        formatted = benchmark_record.score_record(
            run_record(
                run_id="run-002",
                stage="formatting",
                source_run_id="run-001",
                configuration="parakeet-formatted",
                formatter="s1-mini",
                formatter_revision="formatter-v1",
                formatting_profile="light",
                stop_to_visible_ms=220.0,
            ),
            fixture(),
        )

        summary = benchmark_record.summarize([raw, formatted])

        raw_summary = summary["stages"]["raw_asr"]["configurations"]["parakeet-control"]
        formatted_summary = summary["stages"]["formatting"]["configurations"]["parakeet-formatted"]
        self.assertEqual(set(summary["stages"]), {"raw_asr", "formatting"})
        self.assertEqual(raw_summary["metrics"]["runs"], 1)
        self.assertEqual(formatted_summary["metrics"]["runs"], 1)
        self.assertEqual(
            formatted_summary["metrics"]["stop_to_visible_ms"]["p50"],
            220.0,
        )

    def test_summary_never_mixes_different_model_or_hardware_identity(self) -> None:
        first = benchmark_record.score_record(run_record(), fixture())
        second = benchmark_record.score_record(
            run_record(run_id="run-002", model="another-model"),
            fixture(),
        )

        with self.assertRaisesRegex(ValueError, "configuration.*identity"):
            benchmark_record.summarize([first, second])

        separate = benchmark_record.score_record(
            run_record(
                run_id="run-003",
                configuration="other-model",
                model="another-model",
            ),
            fixture(),
        )
        configurations = benchmark_record.summarize([first, separate])["stages"][
            "raw_asr"
        ]["configurations"]
        self.assertEqual(set(configurations), {"parakeet-control", "other-model"})

    def test_summary_rejects_a_formatting_source_that_is_not_present_raw_asr(self) -> None:
        formatted = benchmark_record.score_record(
            run_record(
                run_id="run-002",
                stage="formatting",
                source_run_id="missing-run",
                configuration="parakeet-formatted",
                formatter="s1-mini",
                formatter_revision="formatter-v1",
                formatting_profile="light",
            ),
            fixture(),
        )

        with self.assertRaisesRegex(ValueError, "raw_asr"):
            benchmark_record.summarize([formatted])

    def test_silence_false_paste_is_not_folded_into_spoken_wer(self) -> None:
        spoken = benchmark_record.score_record(run_record(), fixture())
        silence = benchmark_record.score_record(
            run_record(
                run_id="run-silence",
                fixture_id="silence",
                fixture_sha256="d" * 64,
                hypothesis="background words",
            ),
            fixture(
                id="silence",
                sha256="d" * 64,
                expected_empty=True,
                reference="",
                protected_terms=[],
            ),
        )

        summary = benchmark_record.summarize([spoken, silence])["stages"]["raw_asr"][
            "configurations"
        ]["parakeet-control"]["metrics"]

        self.assertEqual(summary["wer"], 0.0)
        self.assertEqual(summary["non_speech_false_pastes"], 1)

    def test_summary_reports_missing_latency_resources_and_safety_coverage(self) -> None:
        scored = benchmark_record.score_record(
            run_record(
                timing_scope="engine",
                stop_to_final_ms=None,
                stop_to_visible_ms=None,
                final_sample_to_visible_ms=None,
                peak_rss_bytes=None,
                installed_footprint_bytes=None,
            ),
            fixture(),
        )

        metrics = benchmark_record.summarize([scored])["stages"]["raw_asr"][
            "configurations"
        ]["parakeet-control"]["metrics"]

        self.assertEqual(metrics["stop_to_visible_ms"]["missing"], 1)
        self.assertEqual(metrics["peak_rss_bytes"]["missing"], 1)
        self.assertFalse(metrics["locked_safety_complete"])
        self.assertEqual(
            metrics["missing_locked_safety_categories"],
            ["code", "negation", "number", "path"],
        )

    def test_paired_bootstrap_compares_matching_fixture_repeats(self) -> None:
        control = [
            benchmark_record.score_record(
                run_record(run_id=f"control-{repeat}", repeat=repeat), fixture()
            )
            for repeat in (1, 2)
        ]
        candidate = [
            benchmark_record.score_record(
                run_record(
                    run_id=f"candidate-{repeat}",
                    configuration="candidate",
                    model="candidate-model",
                    repeat=repeat,
                    hypothesis="FluidAudio converts audio to sixteen kilohertz.",
                ),
                fixture(),
            )
            for repeat in (1, 2)
        ]

        comparison = benchmark_record.compare_configurations(
            control,
            candidate,
            bootstrap_samples=200,
            bootstrap_seed=7,
        )

        self.assertEqual(comparison["paired_runs"], 2)
        self.assertEqual(comparison["bootstrap_unit"], "speaker_id")
        self.assertGreater(comparison["wer_difference_points"], 0)
        self.assertEqual(
            comparison,
            benchmark_record.compare_configurations(
                control,
                candidate,
                bootstrap_samples=200,
                bootstrap_seed=7,
            ),
        )

        with self.assertRaisesRegex(ValueError, "paired fixture/repeat"):
            benchmark_record.compare_configurations(control, candidate[:1])

    def test_locked_safety_compares_only_locked_category_and_boundary_regressions(self) -> None:
        code_fixture = fixture(category="code", protected_terms=[])
        control = benchmark_record.score_record(run_record(), code_fixture)
        candidate = benchmark_record.score_record(
            run_record(
                run_id="candidate",
                configuration="candidate",
                model="candidate-model",
                hypothesis="FluidAudio converts forty eight loud audio to sixteen kilohertz.",
            ),
            code_fixture,
        )

        comparison = benchmark_record.compare_configurations(
            [control], [candidate], bootstrap_samples=20
        )

        self.assertEqual(
            comparison["new_locked_safety_failures"],
            [
                {
                    "fixture_id": "technical-001",
                    "repeat": 1,
                    "reasons": ["code"],
                }
            ],
        )
        self.assertFalse(comparison["locked_safety_passed"])

        failed_candidate = benchmark_record.score_record(
            run_record(
                run_id="failed-candidate",
                configuration="candidate",
                model="candidate-model",
                success=False,
                failure_stage="delivery",
                failure_code="paste_failed",
            ),
            code_fixture,
        )
        failed = benchmark_record.compare_configurations(
            [control], [failed_candidate], bootstrap_samples=20
        )
        self.assertEqual(
            failed["new_locked_safety_failures"][0]["reasons"], ["run_failure"]
        )

        ordinary_fixture = fixture(category="normal", protected_terms=[])
        ordinary_candidate = benchmark_record.score_record(
            run_record(
                run_id="ordinary-candidate",
                configuration="candidate",
                model="candidate-model",
                hypothesis="FluidAudio converts forty eight loud audio to sixteen kilohertz.",
            ),
            ordinary_fixture,
        )
        ordinary_control = benchmark_record.score_record(
            run_record(run_id="ordinary-control"), ordinary_fixture
        )
        ordinary = benchmark_record.compare_configurations(
            [ordinary_control], [ordinary_candidate], bootstrap_samples=20
        )
        self.assertEqual(ordinary["new_locked_safety_failures"], [])

    def test_comparison_rejects_different_measurement_scopes(self) -> None:
        control = benchmark_record.score_record(
            run_record(timing_scope="engine"), fixture()
        )
        candidate = benchmark_record.score_record(
            run_record(
                run_id="candidate",
                configuration="candidate",
                model="candidate-model",
            ),
            fixture(),
        )

        with self.assertRaisesRegex(ValueError, "timing_scope"):
            benchmark_record.compare_configurations([control], [candidate])

    def test_current_corpus_loads_protected_terms_as_json(self) -> None:
        corpus_path = Path(__file__).with_name("corpus.tsv")
        corpus = benchmark_record.load_corpus(corpus_path)

        self.assertEqual(len(corpus), 11)
        self.assertIn("technical-001", corpus)
        self.assertEqual(corpus["technical-001"]["category"], "technical")
        self.assertFalse(corpus["technical-001"]["publishable"])
        self.assertIsInstance(corpus["technical-001"]["protected_terms"], list)

    def test_invalid_record_and_corpus_states_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "failed run requires failure_stage"):
            benchmark_record.score_record(
                run_record(success=False, failure_code="capture", failure_stage=""),
                fixture(),
            )
        with self.assertRaisesRegex(ValueError, "successful run cannot"):
            benchmark_record.score_record(
                run_record(failure_message="stale error"),
                fixture(),
            )
        with self.assertRaisesRegex(ValueError, "timing_scope"):
            benchmark_record.score_record(
                run_record(timing_scope="made-up"),
                fixture(),
            )
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            benchmark_record.score_record(
                run_record(fixture_sha256="not-a-hash"),
                fixture(sha256="not-a-hash"),
            )

    def test_corpus_rejects_duplicate_audio_hashes(self) -> None:
        header = (
            "id\tcategory\tspeaker_id\tpublishable\texpected_empty\treference\t"
            "sha256\tprotected_terms\n"
        )
        digest = "d" * 64
        rows = (
            f"first\tnormal\tspeaker\ttrue\tfalse\tHello\t{digest}\t[]\n"
            f"second\tnormal\tspeaker\ttrue\tfalse\tHello\t{digest}\t[]\n"
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "corpus.tsv"
            path.write_text(header + rows, encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "duplicate audio SHA-256"):
                benchmark_record.load_corpus(path)

    def test_sha256_values_must_use_canonical_lowercase(self) -> None:
        with self.assertRaisesRegex(ValueError, "lowercase"):
            benchmark_record.score_record(
                run_record(fixture_sha256="A" * 64),
                fixture(sha256="A" * 64),
            )

    def test_invalid_tsv_boolean_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "bad.tsv"
            path.write_text("schema_version\tsuccess\n1\tmaybe\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "success"):
                benchmark_record.read_records(path)

    def test_unknown_run_field_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown fields"):
            benchmark_record.score_record(
                run_record(invented_metric=1),
                fixture(),
            )

    def test_cli_scores_jsonl_and_writes_tsv_plus_summary(self) -> None:
        corpus = Path(__file__).with_name("corpus.tsv")
        corpus_fixture = benchmark_record.load_corpus(corpus)["technical-001"]
        raw = run_record(fixture_sha256=corpus_fixture["sha256"])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            input_path = root / "runs.jsonl"
            output_path = root / "scored.tsv"
            summary_path = root / "summary.json"
            benchmark_record.write_records(input_path, [raw])

            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("benchmark_record.py")),
                    str(input_path),
                    "--corpus",
                    str(corpus),
                    "--output",
                    str(output_path),
                    "--summary",
                    str(summary_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(len(benchmark_record.read_records(output_path)), 1)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            self.assertEqual(summary["records"], 1)

    def test_cli_writes_an_explicit_control_candidate_comparison(self) -> None:
        corpus = Path(__file__).with_name("corpus.tsv")
        corpus_fixture = benchmark_record.load_corpus(corpus)["technical-001"]
        control = run_record(fixture_sha256=corpus_fixture["sha256"])
        candidate = run_record(
            run_id="candidate-001",
            configuration="candidate",
            model="candidate-model",
            fixture_sha256=corpus_fixture["sha256"],
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            input_path = root / "runs.jsonl"
            output_path = root / "scored.tsv"
            summary_path = root / "summary.json"
            benchmark_record.write_records(input_path, [control, candidate])

            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("benchmark_record.py")),
                    str(input_path),
                    "--corpus",
                    str(corpus),
                    "--output",
                    str(output_path),
                    "--summary",
                    str(summary_path),
                    "--control",
                    "parakeet-control",
                    "--candidate",
                    "candidate",
                    "--bootstrap-samples",
                    "20",
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            self.assertEqual(summary["comparison"]["paired_runs"], 1)
            self.assertNotIn("promotion_gate_passed", summary["comparison"])

    def test_summary_json_is_deterministic(self) -> None:
        scored = benchmark_record.score_record(run_record(), fixture())
        first = json.dumps(benchmark_record.summarize([scored]), sort_keys=True)
        second = json.dumps(benchmark_record.summarize([scored]), sort_keys=True)

        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()

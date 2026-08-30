#!/usr/bin/env python3

import csv
import hashlib
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import bakeoff


class SwedishBakeoffTests(unittest.TestCase):
    def test_time_receipt_uses_real_wall_and_peak_rss(self) -> None:
        receipt = bakeoff.parse_time_receipt(
            "  1.25 real  0.75 user  0.20 sys\n"
            "  123456789 maximum resident set size\n"
        )

        self.assertEqual(receipt["wall_seconds"], 1.25)
        self.assertEqual(receipt["user_seconds"], 0.75)
        self.assertEqual(receipt["peak_rss_bytes"], 123456789)

    def test_whisper_text_joins_all_segments_without_inventing_words(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.json"
            path.write_text(
                json.dumps({"transcription": [{"text": " Hej"}, {"text": " världen."}]}),
                encoding="utf-8",
            )

            self.assertEqual(bakeoff.whisper_text(path), "Hej världen.")

    def test_fleurs_loader_checks_audio_and_pinned_tsv(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            audio = root / "sample.wav"
            samples = b"\0\0\0\0" * 1_600
            format_chunk = struct.pack("<HHIIHHH", 3, 1, 16_000, 64_000, 4, 32, 0)
            fact_chunk = struct.pack("<I", 1_600)
            body = (
                b"WAVE"
                + b"fmt " + struct.pack("<I", len(format_chunk)) + format_chunk
                + b"fact" + struct.pack("<I", len(fact_chunk)) + fact_chunk
                + b"data" + struct.pack("<I", len(samples)) + samples
            )
            audio.write_bytes(b"RIFF" + struct.pack("<I", len(body)) + body)
            row = ["1", audio.name, "Hej, världen.", "hej världen", "", "1600", "FEMALE"]
            tsv = root / "test.tsv"
            with tsv.open("w", encoding="utf-8", newline="") as handle:
                csv.writer(handle, delimiter="\t").writerow(row)
            original = bakeoff.FLEURS_TSV_SHA256
            bakeoff.FLEURS_TSV_SHA256 = hashlib.sha256(tsv.read_bytes()).hexdigest()
            try:
                fixtures = bakeoff.load_fleurs(tsv, root, None, expected_rows=1)
                with self.assertRaisesRegex(ValueError, "expected 2"):
                    bakeoff.load_fleurs(tsv, root, None, expected_rows=2)
            finally:
                bakeoff.FLEURS_TSV_SHA256 = original

            self.assertEqual(len(fixtures), 1)
            self.assertEqual(fixtures[0].reference, "hej världen")
            self.assertEqual(fixtures[0].audio_seconds, 0.1)

    def test_fleurs_loader_treats_quotes_as_text(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            audio = root / "quoted.wav"
            samples = b"\0\0" * 1_600
            format_chunk = struct.pack("<HHIIHH", 1, 1, 16_000, 32_000, 2, 16)
            body = (
                b"WAVE"
                + b"fmt " + struct.pack("<I", len(format_chunk)) + format_chunk
                + b"data" + struct.pack("<I", len(samples)) + samples
            )
            audio.write_bytes(b"RIFF" + struct.pack("<I", len(body)) + body)
            tsv = root / "test.tsv"
            tsv.write_text(
                "1\tquoted.wav\t\"\"\"Hej.\"\"\"\t\"\"\"hej.\"\"\t"
                "\" h e j . \"\t1600\tFEMALE\n",
                encoding="utf-8",
            )
            original = bakeoff.FLEURS_TSV_SHA256
            bakeoff.FLEURS_TSV_SHA256 = hashlib.sha256(tsv.read_bytes()).hexdigest()
            try:
                fixtures = bakeoff.load_fleurs(tsv, root, None, expected_rows=1)
            finally:
                bakeoff.FLEURS_TSV_SHA256 = original

            self.assertEqual(fixtures[0].reference, '\"\"\"hej.\"\"')

    def test_parakeet_result_rejects_unexpected_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.json"
            path.write_text(
                json.dumps({
                    "audioFile": "sample.wav",
                    "audioSeconds": 1.0,
                    "engineMilliseconds": 100.0,
                    "realTimeFactor": 0.1,
                    "confidence": 0.9,
                    "text": "hej",
                    "unexpected": True,
                }),
                encoding="utf-8",
            )
            fixture = bakeoff.Fixture(
                "id", "sample.wav", "hej", path, 1.0, "0" * 64
            )

            with self.assertRaisesRegex(ValueError, "unexpected fields"):
                bakeoff.parakeet_result(path, fixture)

    def test_report_suppresses_throughput_for_incomplete_batch(self) -> None:
        scored = [{
            "configuration": "model",
            "audio_seconds": 10.0,
            "reference_words": 2,
            "errors": 2,
            "reference_characters": 7,
            "character_errors": 7,
            "engine_ms": None,
            "success": False,
        }]
        receipts = [{
            "label": "model-1",
            "wall_seconds": 0.1,
            "peak_rss_bytes": 1,
            "model_bytes": 1,
            "batch_complete": False,
        }]

        result = bakeoff.report(scored, receipts)

        self.assertIn("| model | 100.00% | 100.00% | — | — | — | — |", result)

    def test_whisper_outputs_keep_success_before_a_missing_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixtures = [
                bakeoff.Fixture("one", "one.wav", "hej", root / "one.wav", 1.0, "0" * 64),
                bakeoff.Fixture("two", "two.wav", "då", root / "two.wav", 1.0, "1" * 64),
            ]
            (root / "one.json").write_text(
                json.dumps({"transcription": [{"text": " Hej"}]}), encoding="utf-8"
            )

            outputs, actual, expected = bakeoff.read_whisper_outputs(root, fixtures)

            self.assertEqual(outputs["one"], ("Hej", ""))
            self.assertEqual(outputs["two"], (None, "missing structured result"))
            self.assertEqual(actual, {"one.json"})
            self.assertEqual(expected, {"one.json", "two.json"})

    def test_model_directory_rejects_an_extra_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            model = root / "model.bin"
            model.write_bytes(b"model")
            manifest = bakeoff.ModelManifest(
                "owner/model", "revision",
                {"model.bin": (5, hashlib.sha256(b"model").hexdigest())},
            )
            bakeoff.verify_model_directory(root, manifest)
            (root / "stale.bin").write_bytes(b"stale")

            with self.assertRaisesRegex(ValueError, "extra=.*stale.bin"):
                bakeoff.verify_model_directory(root, manifest)

    def test_fluid_model_directory_requires_the_runtime_cache_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            wrong = Path(temporary) / "parakeet-tdt-0.6b-v3-coreml"
            wrong.mkdir()
            manifest = bakeoff.ModelManifest("owner/model", "revision", {})

            with self.assertRaisesRegex(ValueError, "FluidAudio cache name"):
                bakeoff.verify_model_directory(
                    wrong,
                    manifest,
                    expected_name="parakeet-tdt-0.6b-v3",
                )

    def test_cmake_cache_requires_an_exact_option(self) -> None:
        cache = "CMAKE_BUILD_TYPE:STRING=Release\nGGML_METAL:BOOL=ON\n"

        self.assertEqual(
            bakeoff.cmake_cache_value(cache, "CMAKE_BUILD_TYPE"),
            "Release",
        )
        self.assertEqual(bakeoff.cmake_cache_value(cache, "GGML_METAL"), "ON")
        with self.assertRaisesRegex(ValueError, "missing GGML_BLAS"):
            bakeoff.cmake_cache_value(cache, "GGML_BLAS")


if __name__ == "__main__":
    unittest.main()

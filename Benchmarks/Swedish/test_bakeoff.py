#!/usr/bin/env python3

import csv
import hashlib
import json
import sys
import tempfile
import unittest
import wave
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
            with wave.open(str(audio), "wb") as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(16_000)
                output.writeframes(b"\0\0" * 1_600)
            row = ["1", audio.name, "Hej, världen.", "hej världen", "", "1600", "FEMALE"]
            tsv = root / "test.tsv"
            with tsv.open("w", encoding="utf-8", newline="") as handle:
                csv.writer(handle, delimiter="\t").writerow(row)
            original = bakeoff.FLEURS_TSV_SHA256
            bakeoff.FLEURS_TSV_SHA256 = hashlib.sha256(tsv.read_bytes()).hexdigest()
            try:
                fixtures = bakeoff.load_fleurs(tsv, root, None)
            finally:
                bakeoff.FLEURS_TSV_SHA256 = original

            self.assertEqual(len(fixtures), 1)
            self.assertEqual(fixtures[0].reference, "hej världen")
            self.assertEqual(fixtures[0].audio_seconds, 0.1)


if __name__ == "__main__":
    unittest.main()

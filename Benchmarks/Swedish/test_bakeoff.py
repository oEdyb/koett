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
                fixtures = bakeoff.load_fleurs(tsv, root, None)
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
                fixtures = bakeoff.load_fleurs(tsv, root, None)
            finally:
                bakeoff.FLEURS_TSV_SHA256 = original

            self.assertEqual(fixtures[0].reference, '\"\"\"hej.\"\"')


if __name__ == "__main__":
    unittest.main()

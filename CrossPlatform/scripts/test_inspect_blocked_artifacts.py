import importlib.util
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("inspect_blocked_artifacts.py")
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("inspect_blocked_artifacts", MODULE_PATH)
assert SPEC and SPEC.loader
INSPECTOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INSPECTOR)


class ArtifactInspectionTests(unittest.TestCase):
    def test_identifies_only_a_complete_x86_64_pe_header(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "Koett.exe"
            header = bytearray(154)
            header[:2] = b"MZ"
            struct.pack_into("<I", header, 0x3C, 128)
            header[128:132] = b"PE\0\0"
            struct.pack_into("<H", header, 132, 0x8664)
            struct.pack_into("<H", header, 152, 0x20B)
            path.write_bytes(header)
            self.assertEqual(INSPECTOR.binary_format(path), "PE32+-x86_64")
            header[128:132] = b"NOPE"
            path.write_bytes(header)
            self.assertEqual(INSPECTOR.binary_format(path), "unknown")

    def test_parses_pe_and_elf_imports(self):
        output = """
          DLL Name: kernel32.dll
          DLL Name: user32.dll
          NEEDED       libasound.so.2
          NEEDED       libc.so.6
        """
        imports, _ = INSPECTOR.parse_objdump(output)
        self.assertEqual(
            imports, ["kernel32.dll", "libasound.so.2", "libc.so.6", "user32.dll"]
        )

    def test_finds_exact_tts_components_without_generic_tts_false_positives(self):
        evidence = INSPECTOR.signal_evidence(
            [
                "SherpaOnnxCreateOfflineTts",
                "piper_phonemize",
                "ESPEAK_DATA_PATH",
                "espeak_ng_Initialize",
                "?phonemize_eSpeak@piper@@YAX",
                "_ZTTSt14basic_ifstream",
            ]
        )
        self.assertEqual(evidence["tts"]["match_count"], 1)
        self.assertEqual(evidence["piper"]["match_count"], 2)
        self.assertEqual(evidence["espeak"]["match_count"], 3)

    def test_reads_ascii_and_utf16le_evidence(self):
        values = INSPECTOR.printable_strings(
            b"prefix\x00OfflineTts\x00E\x00S\x00P\x00E\x00A\x00K\x00_\x00D\x00A\x00T\x00A\x00_\x00P\x00A\x00T\x00H\x00"
        )
        self.assertIn("OfflineTts", values)
        self.assertIn("ESPEAK_DATA_PATH", values)

    def test_rejects_wrong_content_with_the_expected_size(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            path.write_bytes(b"wrong")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                INSPECTOR.verify(path, 5, "0" * 64, "test artifact")

    def test_rejects_archive_paths_outside_the_destination(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "output"
            root.mkdir()
            with self.assertRaisesRegex(ValueError, "escapes destination"):
                INSPECTOR.safe_destination(root, "../outside")

    def test_requires_both_binary_and_verified_source_evidence(self):
        self.assertEqual(INSPECTOR.artifact_verdict(["espeak"], True), "blocked")
        self.assertEqual(INSPECTOR.artifact_verdict(["espeak"], False), "needs-review")
        self.assertEqual(INSPECTOR.artifact_verdict([], True), "needs-review")

    def test_writes_deterministic_sorted_json(self):
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.json"
            second = Path(directory) / "second.json"
            value = {"z": [3, 2, 1], "a": {"d": 4, "b": 2}}
            INSPECTOR.write_json(first, value)
            INSPECTOR.write_json(second, json.loads(first.read_text()))
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertTrue(first.read_text().startswith('{\n  "a"'))


if __name__ == "__main__":
    unittest.main()

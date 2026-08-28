#!/usr/bin/env python3
"""Tests for Koett's package checksum and unsigned build receipt."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("write_build_receipt.py")
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("write_build_receipt", MODULE_PATH)
assert SPEC and SPEC.loader
RECEIPT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RECEIPT)


def ci_environment() -> dict[str, str]:
    return {
        "GITHUB_ACTIONS": "true",
        "GITHUB_EVENT_NAME": "pull_request",
        "GITHUB_JOB": "windows",
        "GITHUB_REF": "refs/pull/2/merge",
        "GITHUB_REPOSITORY": "oEdyb/koett",
        "GITHUB_RUN_ATTEMPT": "1",
        "GITHUB_RUN_ID": "123",
        "GITHUB_SHA": "a" * 40,
        "GITHUB_WORKFLOW": "Windows and Linux",
        "GITHUB_WORKFLOW_SHA": "b" * 40,
        "RUNNER_ARCH": "X64",
        "RUNNER_ENVIRONMENT": "github-hosted",
        "RUNNER_NAME": "GitHub Actions 1",
        "RUNNER_OS": "Windows",
    }


class BuildReceiptTests(unittest.TestCase):
    def test_writes_matching_checksum_and_subject(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "Koett.zip"
            artifact.write_bytes(b"koett-package")
            checksum, receipt_path = RECEIPT.write_evidence(
                artifact,
                root / "output",
                "windows-x86_64",
                ci_environment(),
                "rustc 1.98.0",
                "cargo 1.98.0",
            )
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            expected = RECEIPT.sha256_file(artifact)
            self.assertEqual(receipt["subject"]["sha256"], expected)
            self.assertEqual(receipt["subject"]["bytes"], len(b"koett-package"))
            self.assertEqual(checksum.read_text(), f"{expected}  Koett.zip\n")

    def test_same_inputs_produce_identical_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "Koett.tar.gz"
            artifact.write_bytes(b"same-package")
            first = RECEIPT.write_evidence(
                artifact,
                root / "first",
                "linux-arm64",
                ci_environment(),
                "rustc 1.98.0",
                "cargo 1.98.0",
            )
            second = RECEIPT.write_evidence(
                artifact,
                root / "second",
                "linux-arm64",
                ci_environment(),
                "rustc 1.98.0",
                "cargo 1.98.0",
            )
            self.assertEqual(
                [path.read_bytes() for path in first],
                [path.read_bytes() for path in second],
            )

    def test_missing_ci_identity_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "Koett.zip"
            artifact.write_bytes(b"package")
            environment = ci_environment()
            del environment["GITHUB_RUN_ID"]
            with self.assertRaisesRegex(ValueError, "GITHUB_RUN_ID"):
                RECEIPT.build_receipt(
                    artifact,
                    "windows-x86_64",
                    environment,
                    "rustc 1.98.0",
                    "cargo 1.98.0",
                )

    def test_empty_artifact_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "Koett.zip"
            artifact.touch()
            with self.assertRaisesRegex(ValueError, "nonempty"):
                RECEIPT.build_receipt(
                    artifact,
                    "windows-x86_64",
                    ci_environment(),
                    "rustc 1.98.0",
                    "cargo 1.98.0",
                )

    def test_unrelated_environment_values_are_not_copied(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "Koett.zip"
            artifact.write_bytes(b"package")
            environment = ci_environment()
            environment["NOT_FOR_THE_RECEIPT"] = "secret-value"
            receipt = RECEIPT.build_receipt(
                artifact,
                "windows-x86_64",
                environment,
                "rustc 1.98.0",
                "cargo 1.98.0",
            )
            self.assertNotIn("secret-value", json.dumps(receipt))


if __name__ == "__main__":
    unittest.main()

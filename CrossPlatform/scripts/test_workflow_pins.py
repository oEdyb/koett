#!/usr/bin/env python3
"""Check Koett's small set of immutable workflow and Rust pins."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/cross-platform.yml"
TOOLCHAIN = ROOT / "CrossPlatform/rust-toolchain.toml"
CARGO_MANIFEST = ROOT / "CrossPlatform/Cargo.toml"

ACTION_PINS = {
    "actions/checkout": "3d3c42e5aac5ba805825da76410c181273ba90b1",
    "actions/dependency-review-action": "a1d282b36b6f3519aa1f3fc636f609c47dddb294",
    "actions/upload-artifact": "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
}


def external_action_pins(workflow: str) -> list[tuple[str, str]]:
    pins = []
    uses_lines = re.findall(
        r"(?m)^\s*(?:-\s*)?uses:\s*(\S+?)(?:\s+#\s+(v[0-9.]+))?\s*$", workflow
    )
    for reference, version in uses_lines:
        if reference.startswith("./"):
            continue
        match = re.fullmatch(r"([\w.-]+/[\w.-]+)@([0-9a-f]{40})", reference)
        if match is None or not version:
            raise ValueError(f"external action is not pinned to a reviewed full commit: {reference}")
        pins.append((match.group(1), match.group(2)))
    return pins


class WorkflowPinTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")

    def test_every_action_uses_the_reviewed_full_commit(self) -> None:
        uses = external_action_pins(self.workflow)
        self.assertTrue(uses)
        for action, commit in uses:
            self.assertEqual(commit, ACTION_PINS[action])
        self.assertEqual(set(action for action, _ in uses), set(ACTION_PINS))

    def test_action_pin_check_rejects_a_branch_reference(self) -> None:
        workflow = (
            "steps:\n"
            "  - name: Checkout\n"
            "    uses: actions/checkout@main # v7.0.1\n"
        )
        with self.assertRaisesRegex(ValueError, "not pinned"):
            external_action_pins(workflow)

    def test_checkout_never_persists_credentials(self) -> None:
        checkout_steps = re.findall(
            r"uses:\s*actions/checkout@[0-9a-f]{40}[^\n]*\n"
            r"\s+with:\n\s+persist-credentials:\s*false",
            self.workflow,
        )
        self.assertEqual(len(checkout_steps), self.workflow.count("uses: actions/checkout@"))

    def test_main_and_minimum_rust_are_separate(self) -> None:
        toolchain = TOOLCHAIN.read_text(encoding="utf-8")
        package = CARGO_MANIFEST.read_text(encoding="utf-8")
        self.assertIn('channel = "1.98.0"', toolchain)
        self.assertIn('components = ["clippy", "rustfmt"]', toolchain)
        self.assertIn('profile = "minimal"', toolchain)
        self.assertRegex(package, r'(?m)^rust-version = "1\.92"$')
        self.assertIn("rustup default 1.98.0", self.workflow)
        self.assertEqual(self.workflow.count("--component clippy,rustfmt"), 3)
        self.assertIn("cargo +1.92.0 check --locked --all-targets", self.workflow)

    def test_advisory_tools_are_exact_and_read_only(self) -> None:
        self.assertIn("cargo install cargo-audit --version 0.22.2 --locked", self.workflow)
        self.assertIn("run: cargo audit", self.workflow)
        self.assertIn("permissions:\n  contents: read", self.workflow)
        self.assertNotIn("contents: write", self.workflow)
        self.assertNotIn("id-token: write", self.workflow)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Check Koett's small set of immutable workflow and Rust pins."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/cross-platform.yml"
TOOLCHAIN = ROOT / "CrossPlatform/rust-toolchain.toml"
CARGO_MANIFEST = ROOT / "CrossPlatform/Cargo.toml"

ACTION_PINS = {
    "actions/cache/restore": "55cc8345863c7cc4c66a329aec7e433d2d1c52a9",
    "actions/cache/save": "55cc8345863c7cc4c66a329aec7e433d2d1c52a9",
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
        match = re.fullmatch(r"([\w.-]+/[\w.-]+(?:/[\w.-]+)*)@([0-9a-f]{40})", reference)
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
        self.assertEqual({action for action, _ in uses}, set(ACTION_PINS))

    def test_action_pin_check_rejects_a_branch_reference(self) -> None:
        workflow = (
            "steps:\n"
            "  - name: Checkout\n"
            "    uses: actions/checkout@main # v7.0.1\n"
        )
        with self.assertRaisesRegex(ValueError, "not pinned"):
            external_action_pins(workflow)

    def test_every_non_draft_pull_request_runs_and_drafts_do_not(self) -> None:
        pull_request = self.workflow.split("  pull_request:\n", 1)[1].split("  push:\n", 1)[0]
        self.assertIn("types: [opened, reopened, synchronize, ready_for_review]", pull_request)
        self.assertIn("  push:\n    branches:\n      - main\n", self.workflow)
        jobs = self.workflow.split("jobs:\n", 1)[1]
        for job in ("dependency-review", "supply-chain", "msrv", "windows", "linux"):
            header = jobs.split(f"  {job}:\n", 1)[1].split("\n", 1)[0]
            self.assertIn("!github.event.pull_request.draft", header, job)

    def test_releases_use_no_cache_and_only_main_saves_one(self) -> None:
        restores = self.workflow.count("uses: actions/cache/restore@")
        saves = self.workflow.count("uses: actions/cache/save@")
        self.assertEqual(restores, 9)
        self.assertEqual(saves, 9)
        self.assertEqual(
            self.workflow.count("if: github.event_name != 'workflow_dispatch'\n"
                                "        uses: actions/cache/restore@"),
            restores,
        )
        self.assertEqual(
            len(re.findall(r"if: github\.event_name == 'push' && [^\n]*\n"
                           r"        uses: actions/cache/save@", self.workflow)),
            saves,
        )

    def test_cache_keys_stay_stable_between_commits(self) -> None:
        keys = re.findall(r"(?m)^\s+key: (\S+.*)$", self.workflow)
        self.assertTrue(keys)
        for key in keys:
            self.assertNotIn("github.sha", key)

    def test_the_gate_names_every_cache_main_saves(self) -> None:
        gate = self.workflow.split("          CACHE_KEYS: |\n", 1)[1].split("        run: |", 1)[0]
        listed = {line.strip() for line in gate.splitlines()}
        expansions = [
            {"${{ matrix.runner }}": "ubuntu-22.04", "${{ matrix.architecture }}": "x86_64"},
            {"${{ matrix.runner }}": "ubuntu-22.04-arm", "${{ matrix.architecture }}": "arm64"},
        ]
        saved = re.findall(r"uses: actions/cache/save@[^\n]*\n\s+with:\n\s+path:(?:[^\n]*\n(?:\s{12}[^\n]*\n)*)\s+key: ([^\n]+)", self.workflow)
        self.assertEqual(len(saved), 9)
        for key in saved:
            if key.startswith("apt-v1-"):
                self.assertIn("steps.apt_month.outputs.month", key)
                continue
            for expansion in expansions:
                expanded = key
                for name, value in expansion.items():
                    expanded = expanded.replace(name, value)
                self.assertIn(expanded, listed)

    def test_a_skipped_or_failed_gate_never_blocks_ci(self) -> None:
        jobs = self.workflow.split("jobs:\n", 1)[1]
        for job in ("supply-chain", "msrv", "windows", "linux"):
            header = jobs.split(f"  {job}:\n", 1)[1].split("\n", 2)
            self.assertIn("!cancelled()", header[0], job)
            self.assertIn("needs.decide.outputs.skip != 'true'", header[0], job)
            self.assertEqual(header[1].strip(), "needs: decide", job)
        self.assertNotIn("skip == ", self.workflow)

    def test_only_the_trim_job_can_write_to_actions(self) -> None:
        self.assertEqual(self.workflow.count("actions: write"), 1)
        trim = self.workflow.split("  cache-trim:\n", 1)[1].split("\n  supply-chain:", 1)[0]
        self.assertIn("actions: write", trim.split("  dependency-review:", 1)[0])
        self.assertNotIn("cargo", trim.split("  dependency-review:", 1)[0])

    def test_paths_skip_only_what_cannot_affect_the_build(self) -> None:
        patterns = self.workflow.split("  push:\n", 1)[1].split("paths:\n", 1)[1].split("\npermissions", 1)[0]
        patterns = re.findall(r'- "([^"]+)"', patterns)
        pull_request = re.findall(r'- "([^"]+)"', self.workflow.split("  pull_request:\n", 1)[1].split("  push:\n", 1)[0])
        self.assertEqual(patterns, pull_request)

        def matched(pattern: str, path: str) -> bool:
            regex = re.escape(pattern).replace(r"\*\*", ".*")
            return re.fullmatch(regex, path) is not None

        def triggers(path: str) -> bool:
            result = False
            for pattern in patterns:  # The last pattern to match wins.
                if pattern.startswith("!"):
                    result = False if matched(pattern[1:], path) else result
                elif matched(pattern, path):
                    result = True
            return result

        for path in ("docs/linux-updates.md", "README.md", "AGENTS.md", "Sources/Koett/Koett.swift",
                     "CrossPlatform/BENCHMARK.md", "CrossPlatform/evidence/menus-20261028/a.png",
                     "CrossPlatform/evidence/gnome-capture-20261104/other.json"):
            self.assertFalse(triggers(path), path)
        for path in ("CrossPlatform/src/lib.rs", "CrossPlatform/README.md", "CrossPlatform/scripts/x.py",
                     "CrossPlatform/evidence/supply-chain-inventory/inventory.json",
                     "CrossPlatform/evidence/blocked-604ed15/control.json",
                     "CrossPlatform/evidence/gnome-capture-20261104/capture.png",
                     "Package.swift", "Package.resolved", "LICENSE", "Packaging/Notices/Sparkle-LICENSE.txt",
                     ".gitattributes", ".github/workflows/cross-platform.yml"):
            self.assertTrue(triggers(path), path)
        # Every file the inventory hashes must run the check that catches it going stale.
        inventory = json.loads((ROOT / "CrossPlatform/evidence/supply-chain-inventory/inventory.json").read_text(encoding="utf-8"))
        for record in inventory["inputs"]:
            self.assertTrue(triggers(record["path"]), record["path"])

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

    def test_linux_artifacts_are_installed_before_upload(self) -> None:
        self.assertIn('run: python3 scripts/check_linux_notices.py', self.workflow)
        self.assertEqual(self.workflow.count('"Sources/Koett/Resources/AssistantRenderer/**"'), 2)
        linux = self.workflow.split('  linux:\n', 1)[1]
        self.assertIn('package_linux.py --flavor system-runtime --output dist', linux)
        self.assertIn("python3 -m unittest discover -s scripts -p 'test_package_linux*.py'", linux)
        self.assertIn('test_package_linux_archive.py --archive dist/Koett-Linux-*.tar.gz', linux)
        self.assertLess(linux.index('test_package_linux_archive.py'), linux.index('name: Upload Linux release candidate'))
        self.assertIn('build_asr_native.py --jobs 2', linux)
        self.assertIn('CARGO_BUILD_JOBS: 2', linux)
        self.assertIn('--bin koett-engine -- --self-test', linux)
        self.assertNotIn('continue-on-error', linux)

    def test_native_backtrace_is_only_collected_after_failed_required_inference(self) -> None:
        linux = self.workflow.split('  linux:\n', 1)[1]
        self.assertIn('id: asr_self_test', linux)
        self.assertEqual(linux.count("if: failure() && steps.asr_self_test.outcome == 'failure'"), 2)
        self.assertIn('gdb --batch --return-child-result', linux)
        self.assertIn('native-asr-failure-${{ matrix.architecture }}', linux)
        self.assertNotIn('continue-on-error', linux)

    def test_windows_native_build_uses_the_microsoft_generator(self) -> None:
        windows = self.workflow.split('  windows:\n', 1)[1].split('  linux:\n', 1)[0]
        self.assertIn('build_asr_native.py --generator "Visual Studio 18 2026"', windows)
        self.assertIn('runs-on: windows-2025', windows)
        self.assertIn('check_windows_archive.py --archive dist/Koett-Windows-x86_64.zip', windows)
        self.assertLess(windows.index('check_windows_archive.py'), windows.index('actions/upload-artifact@'))

    def test_advisory_tools_are_exact_and_read_only(self) -> None:
        self.assertIn('retry-on-snapshot-warnings: true', self.workflow)
        self.assertIn('retry-on-snapshot-warnings-timeout: 120', self.workflow)
        self.assertNotIn('warn-only: true', self.workflow)
        self.assertIn("cargo install cargo-audit --version 0.22.2 --locked", self.workflow)
        self.assertIn("run: cargo audit", self.workflow)
        self.assertIn("permissions:\n  contents: read", self.workflow)
        self.assertNotIn("contents: write", self.workflow)
        self.assertNotIn("id-token: write", self.workflow)


if __name__ == "__main__":
    unittest.main()

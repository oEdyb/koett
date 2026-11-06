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

    def test_nothing_runs_unless_someone_starts_it(self) -> None:
        triggers = self.workflow.split("\non:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertEqual(re.findall(r"(?m)^  (\w+):", triggers), ["workflow_dispatch"])

    def test_windows_only_recheck_leaves_other_jobs_off(self) -> None:
        for job in ['supply-chain', 'msrv', 'linux']:
            self.assertIn('  ' + job + ':\n    if: ${{ !inputs.windows_only }}', self.workflow)
        windows = self.workflow.split('  windows:\n', 1)[1].split('    steps:', 1)[0]
        self.assertNotIn('    if:', windows)
        dependency = self.workflow.split('  dependency-review:\n', 1)[1].split('    runs-on:', 1)[0]
        self.assertIn('!inputs.windows_only', dependency)
        trimming = self.workflow.split('  cache-trim:\n', 1)[1].split('    runs-on:', 1)[0]
        self.assertIn('!inputs.windows_only', trimming)
        inputs = self.workflow.split('      windows_only:', 1)[1].split('permissions:', 1)[0]
        self.assertIn('default: false', inputs)

    def test_releases_use_no_cache_and_only_main_saves_one(self) -> None:
        restores = self.workflow.count("uses: actions/cache/restore@")
        saves = self.workflow.count("uses: actions/cache/save@")
        self.assertEqual(restores, 9)
        self.assertEqual(saves, 9)
        self.assertEqual(
            self.workflow.count("if: ${{ !inputs.clean }}\n"
                                "        uses: actions/cache/restore@"),
            restores,
        )
        self.assertEqual(
            len(re.findall(r"if: github\.ref == 'refs/heads/main' && !inputs\.clean && [^\n]*\n"
                           r"        uses: actions/cache/save@", self.workflow)),
            saves,
        )

    def test_cache_keys_stay_stable_between_commits(self) -> None:
        keys = re.findall(r"(?m)^\s+key: (\S+.*)$", self.workflow)
        self.assertTrue(keys)
        for key in keys:
            self.assertNotIn("github.sha", key)

    def test_trimming_names_every_cache_main_saves(self) -> None:
        keys = self.workflow.split("          CACHE_KEYS: |\n", 1)[1].split("        run: |", 1)[0]
        listed = {line.strip() for line in keys.splitlines()}
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

    def test_only_the_trim_job_can_write_to_actions(self) -> None:
        self.assertEqual(self.workflow.count("actions: write"), 1)
        trim = self.workflow.split("  cache-trim:\n", 1)[1].split("\n  supply-chain:", 1)[0]
        self.assertIn("actions: write", trim.split("  dependency-review:", 1)[0])
        self.assertNotIn("run: cargo", trim.split("  dependency-review:", 1)[0])

    def test_linux_builds_carry_a_real_update_feed_and_no_lab_switch(self) -> None:
        linux = self.workflow.split("  linux:\n", 1)[1]
        self.assertIn(
            "KOETT_UPDATE_FEED: https://github.com/oEdyb/koett/releases/latest/download/"
            "appcast-linux-${{ matrix.machine }}.json\n",
            linux,
        )
        self.assertRegex(linux, r"KOETT_UPDATE_PUBLIC_KEY: [A-Za-z0-9+/]{43}=\n")
        self.assertRegex(linux, r'KOETT_UPDATE_BUILD: "[1-9][0-9]*"\n')
        self.assertEqual(linux.count("machine: x86_64\n") + linux.count("machine: aarch64\n"), 2)
        for switch in ("KOETT_UPDATE_LOOPBACK_TESTING", "KOETT_UPDATE_LAB_FAULT"):
            self.assertNotIn(switch, self.workflow)
        before = self.workflow.split("  linux:\n", 1)[0]
        self.assertNotIn("KOETT_UPDATE_", before)

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

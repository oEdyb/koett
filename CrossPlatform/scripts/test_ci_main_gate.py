#!/usr/bin/env python3
"""The main-run gate skips only on proof and runs CI on anything else."""

from __future__ import annotations

import io
import os
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

import ci_main_gate as gate

SHA, HEAD, TREE = "m" * 40, "h" * 40, "t" * 40
KEYS = ["cargo-v2-msrv-" + "a" * 64, "apt-v1-x86_64-202611"]
JOBS = ["dependency-review", "supply-chain", "msrv", "windows",
        "linux (ubuntu-22.04, x86_64, x86_64-unknown-linux-gnu)",
        "linux (ubuntu-22.04-arm, arm64, aarch64-unknown-linux-gnu)"]


def zipped(tree: str) -> bytes:
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w") as archive:
        archive.writestr("tested-tree.txt", tree + "\n")
    return data.getvalue()


class World:
    """What GitHub would answer for one clean merge. Tests break one thing each."""

    def __init__(self) -> None:
        self.pulls = [{"number": 7, "merge_commit_sha": SHA, "head": {"sha": HEAD}}]
        self.runs = [{"id": 1, "conclusion": "success", "created_at": "2026-11-06T02:00:00Z"}]
        self.jobs = [{"name": name, "conclusion": "success"} for name in JOBS]
        self.artifacts = [{"id": 9, "name": "tested-tree", "expired": False}]
        self.tree = TREE
        self.caches = list(KEYS)

    def api(self, path: str):
        if path.startswith(f"commits/{SHA}/pulls"):
            return self.pulls
        if path.startswith("actions/workflows/"):
            self.assert_head(path)
            return {"workflow_runs": self.runs}
        if path.endswith("/artifacts"):
            return {"artifacts": self.artifacts}
        if "/jobs" in path:
            return {"jobs": self.jobs}
        if path.startswith("actions/caches"):
            return {"actions_caches": [{"key": key} for key in self.caches]}
        raise AssertionError(path)

    def assert_head(self, path: str) -> None:
        assert f"head_sha={HEAD}" in path and "event=pull_request" in path, path

    def download(self, _path: str) -> bytes:
        return zipped(self.tree)

    def decide(self):
        return gate.decide(self.api, self.download, SHA, TREE, KEYS)


class DecideTests(unittest.TestCase):
    def test_skips_only_when_all_of_it_lines_up(self) -> None:
        skip, reason = World().decide()
        self.assertTrue(skip, reason)

    def test_runs_when_main_moved_during_the_pull_requests_run(self) -> None:
        world = World()
        world.tree = "x" * 40
        skip, reason = world.decide()
        self.assertFalse(skip)
        self.assertIn("main moved", reason)

    def test_runs_when_a_cache_needs_saving(self) -> None:
        world = World()
        world.caches = KEYS[1:]
        skip, reason = world.decide()
        self.assertFalse(skip)
        self.assertIn("needs saving", reason)

    def test_runs_for_a_push_that_is_not_a_merged_pull_request(self) -> None:
        for pulls in ([], [{"number": 7, "merge_commit_sha": "other", "head": {"sha": HEAD}}]):
            world = World()
            world.pulls = pulls
            self.assertFalse(world.decide()[0])

    def test_runs_when_the_newest_real_run_was_not_green(self) -> None:
        world = World()
        world.runs.append({"id": 2, "conclusion": "failure", "created_at": "2026-11-06T03:00:00Z"})
        self.assertFalse(world.decide()[0])

    def test_draft_and_replaced_runs_prove_nothing(self) -> None:
        world = World()
        world.runs = [{"id": 2, "conclusion": "skipped", "created_at": "2026-11-06T03:00:00Z"},
                      {"id": 3, "conclusion": "cancelled", "created_at": "2026-11-06T04:00:00Z"}]
        self.assertFalse(world.decide()[0])
        world.runs.append({"id": 1, "conclusion": "success", "created_at": "2026-11-06T02:00:00Z"})
        self.assertTrue(world.decide()[0])

    def test_a_job_that_was_skipped_or_missing_proves_nothing(self) -> None:
        for broken in ("skipped", None):
            world = World()
            if broken:
                world.jobs[3]["conclusion"] = broken
            else:
                del world.jobs[3]
            self.assertFalse(world.decide()[0])

    def test_runs_without_a_record_of_what_was_tested(self) -> None:
        for artifacts in ([], [{"id": 9, "name": "tested-tree", "expired": True}]):
            world = World()
            world.artifacts = artifacts
            self.assertFalse(world.decide()[0])

    def test_any_error_runs_ci_and_still_exits_cleanly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output, summary = Path(directory) / "output", Path(directory) / "summary"
            env = {"GITHUB_OUTPUT": str(output), "GITHUB_STEP_SUMMARY": str(summary),
                   "GITHUB_SHA": SHA, "GITHUB_REPOSITORY": "o/r", "CACHE_KEYS": "\n".join(KEYS)}
            with mock.patch.dict(os.environ, env), mock.patch.object(gate, "gh_api", side_effect=OSError("no network")):
                gate.run_decide()
            self.assertIn("skip=false", output.read_text())
            self.assertIn("no network", summary.read_text())

    def test_a_missing_token_or_tool_runs_ci_too(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output, summary = Path(directory) / "output", Path(directory) / "summary"
            env = {"GITHUB_OUTPUT": str(output), "GITHUB_STEP_SUMMARY": str(summary)}
            with mock.patch.dict(os.environ, env, clear=True):
                gate.run_decide()
            self.assertIn("skip=false", output.read_text())


class TrimTests(unittest.TestCase):
    def test_families_ignore_only_what_changes_between_versions(self) -> None:
        lock, sha = "a" * 64, "b" * 40
        self.assertEqual(gate.family(f"cargo-v2-linux-x86_64-{lock}"), "cargo-v2-linux-x86_64")
        self.assertEqual(gate.family(f"cargo-v1-msrv-{lock}-{sha}"), "cargo-v1-msrv")
        self.assertEqual(gate.family("apt-v1-arm64-202611"), "apt-v1-arm64")
        self.assertNotEqual(gate.family(f"native-v1-ubuntu-22.04-{lock}"), gate.family(f"native-v1-ubuntu-22.04-arm-{lock}"))

    def test_a_replaced_cache_goes_and_the_current_one_stays(self) -> None:
        old, new = "cargo-v2-msrv-" + "a" * 64, "cargo-v2-msrv-" + "b" * 64
        caches = [{"key": old, "id": 1}, {"key": new, "id": 2}]
        self.assertEqual([c["id"] for c in gate.stale_caches(caches, [new])], [1])

    def test_nothing_goes_when_its_replacement_failed_to_save(self) -> None:
        old, new = "cargo-v2-msrv-" + "a" * 64, "cargo-v2-msrv-" + "b" * 64
        self.assertEqual(gate.stale_caches([{"key": old, "id": 1}], [new]), [])

    def test_another_familys_cache_is_left_alone(self) -> None:
        arm = "native-v1-ubuntu-22.04-arm-" + "a" * 64
        x86 = "native-v1-ubuntu-22.04-" + "b" * 64
        caches = [{"key": arm, "id": 1}, {"key": x86, "id": 2}]
        self.assertEqual(gate.stale_caches(caches, [x86]), [])


if __name__ == "__main__":
    unittest.main()

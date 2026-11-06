#!/usr/bin/env python3
"""Decide whether a push to main repeats work its pull request already did.

`decide` skips the main run only on proof: the merge's tree is the tree the pull
request's last green run checked out, every job of that run passed, and every cache
main is responsible for exists. Anything else, including any error, runs CI.

`trim` deletes caches a newer key has replaced, so they don't wait to expire.
"""

from __future__ import annotations

import io
import json
import os
import re
import subprocess
import sys
import zipfile
from typing import Any, Callable

WORKFLOW = "cross-platform.yml"
ARTIFACT = "tested-tree"
REQUIRED_JOBS = ("dependency-review", "supply-chain", "msrv", "windows", "linux (ubuntu-22.04,", "linux (ubuntu-22.04-arm,")
OLD_SUFFIX = re.compile(r"(-[0-9a-f]{40,64})+$|-\d{6}$")


def family(key: str) -> str:
    """A cache key without what changes between versions of the same cache."""
    return OLD_SUFFIX.sub("", key)


def tested_tree(api: Callable[[str], Any], download: Callable[[str], bytes], head: str) -> tuple[str | None, str]:
    runs = api(f"actions/workflows/{WORKFLOW}/runs?head_sha={head}&event=pull_request&status=completed&per_page=30")
    # Skipped runs are draft pushes, and a cancelled one was replaced by a newer run.
    finished = [r for r in runs["workflow_runs"] if r["conclusion"] not in ("skipped", "cancelled")]
    if not finished:
        return None, f"no finished run for pull request head {head[:7]}"
    run = max(finished, key=lambda r: r["created_at"])
    if run["conclusion"] != "success":
        return None, f"run {run['id']} ended {run['conclusion']}"
    jobs = api(f"actions/runs/{run['id']}/jobs?per_page=50")["jobs"]
    for name in REQUIRED_JOBS:
        matching = [job for job in jobs if job["name"].startswith(name)]
        if not matching or any(job["conclusion"] != "success" for job in matching):
            return None, f"job {name} did not pass in run {run['id']}"
    artifacts = api(f"actions/runs/{run['id']}/artifacts")["artifacts"]
    found = [a for a in artifacts if a["name"] == ARTIFACT and not a["expired"]]
    if not found:
        return None, f"run {run['id']} has no {ARTIFACT} record"
    with zipfile.ZipFile(io.BytesIO(download(f"actions/artifacts/{found[0]['id']}/zip"))) as archive:
        return archive.read("tested-tree.txt").decode().strip(), f"run {run['id']}"


def decide(api: Callable[[str], Any], download: Callable[[str], bytes], sha: str, tree: str, keys: list[str]) -> tuple[bool, str]:
    pulls = [p for p in api(f"commits/{sha}/pulls") if p.get("merge_commit_sha") == sha]
    if not pulls:
        return False, "this push isn't a merged pull request"
    tested, source = tested_tree(api, download, pulls[0]["head"]["sha"])
    if tested is None:
        return False, source
    if tested != tree:
        return False, f"main moved: {source} tested {tested[:12]}, this is {tree[:12]}"
    for key in keys:
        cached = api(f"actions/caches?key={key}&ref=refs/heads/main&per_page=100")["actions_caches"]
        if not any(entry["key"] == key for entry in cached):
            return False, f"cache {key} needs saving"
    return True, f"{source} of #{pulls[0]['number']} already tested this exact tree"


def stale_caches(caches: list[dict[str, Any]], keys: list[str]) -> list[dict[str, Any]]:
    """Only a cache whose replacement exists goes, so a failed save never leaves nothing."""
    present = {entry["key"] for entry in caches}
    replaced = {family(key) for key in keys if key in present}
    return [entry for entry in caches if entry["key"] not in keys and family(entry["key"]) in replaced]


def gh_api(path: str) -> Any:
    return json.loads(subprocess.run(["gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/{path}"], check=True, capture_output=True, timeout=60).stdout)


def gh_download(path: str) -> bytes:
    return subprocess.run(["gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/{path}"], check=True, capture_output=True, timeout=120).stdout


def expected_keys() -> list[str]:
    keys = [line.strip() for line in os.environ["CACHE_KEYS"].splitlines() if line.strip()]
    return sorted(set(keys))


def run_decide() -> None:
    skip, reason = False, "the check could not finish"
    keys: list[str] = []
    try:
        keys = expected_keys()
        tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], check=True, capture_output=True, text=True).stdout.strip()
        skip, reason = decide(gh_api, gh_download, os.environ["GITHUB_SHA"], tree, keys)
    except Exception as error:  # Fails open: not knowing means running CI.
        skip, reason = False, f"the check failed, so CI runs: {error}"
    print(f"skip={str(skip).lower()}: {reason}")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"skip={str(skip).lower()}\nkeys={json.dumps(keys)}\n")
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
        summary.write(f"{'Skipped' if skip else 'Running'} the main run: {reason}\n")


def run_trim() -> None:
    try:
        keys = json.loads(os.environ["KEYS"])
        caches, page = [], 1
        while True:
            batch = gh_api(f"actions/caches?per_page=100&page={page}")["actions_caches"]
            caches += batch
            if len(batch) < 100:
                break
            page += 1
        for entry in stale_caches(caches, keys):
            subprocess.run(["gh", "api", "-X", "DELETE", f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/caches/{entry['id']}"], check=True, capture_output=True, timeout=60)
            print(f"deleted {entry['key']}")
    except Exception as error:  # A cache left behind expires by itself.
        print(f"could not trim caches: {error}")


if __name__ == "__main__":
    {"decide": run_decide, "trim": run_trim}[sys.argv[1]]()

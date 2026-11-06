#!/usr/bin/env python3
"""Delete caches a newer key has replaced, so they don't count against the 10 GB limit."""

from __future__ import annotations

import json
import os
import re
import subprocess
from typing import Any

OLD_SUFFIX = re.compile(r"(-[0-9a-f]{40,64})+$|-\d{6}$")


def family(key: str) -> str:
    """A cache key without what changes between versions of the same cache."""
    return OLD_SUFFIX.sub("", key)


def stale_caches(caches: list[dict[str, Any]], keys: list[str]) -> list[dict[str, Any]]:
    """Only a cache whose replacement exists goes, so a failed save never leaves nothing."""
    present = {entry["key"] for entry in caches}
    replaced = {family(key) for key in keys if key in present}
    return [entry for entry in caches if entry["key"] not in keys and family(entry["key"]) in replaced]


def gh(*arguments: str) -> bytes:
    return subprocess.run(["gh", "api", *arguments], check=True, capture_output=True, timeout=60).stdout


def main() -> None:
    repository = f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/caches"
    keys = [line.strip() for line in os.environ["CACHE_KEYS"].splitlines() if line.strip()]
    caches, page = [], 1
    while True:
        batch = json.loads(gh(f"{repository}?per_page=100&page={page}"))["actions_caches"]
        caches += batch
        if len(batch) < 100:
            break
        page += 1
    for entry in stale_caches(caches, keys):
        gh("-X", "DELETE", f"{repository}/{entry['id']}")
        print(f"deleted {entry['key']}")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:  # A cache left behind expires by itself.
        print(f"could not trim caches: {error}")

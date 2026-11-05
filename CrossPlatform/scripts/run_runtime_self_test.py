#!/usr/bin/env python3
"""Retry only temporary model-download HTTP failures, never inference failures."""
import re
import subprocess
import sys
import time

TEMPORARY_DOWNLOAD = re.compile(
    r"^error: could not download the Koett (?:speech model|voice detector): "
    r"http status: (?:429|502|503|504)\r?$", re.MULTILINE
)


def run(command, execute=subprocess.run, sleep=time.sleep):
    for attempt in range(3):
        result = execute(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        print(result.stdout, end="", flush=True)
        if result.returncode == 0:
            return 0
        if attempt == 2 or not TEMPORARY_DOWNLOAD.search(result.stdout):
            return result.returncode
        print(f"Temporary model download failure; retry {attempt + 1} of 2", flush=True)
        sleep(2)
    raise AssertionError("unreachable")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("usage: run_runtime_self_test.py COMMAND [ARG ...]")
    raise SystemExit(run(sys.argv[1:]))

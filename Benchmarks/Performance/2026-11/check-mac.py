"""Recompute the frozen M5 comparison without printing synthetic payloads.

Fetch audit/macos-performance-2026-11 first if this source object is absent.
The original receiver payloads remain in the Mac audit, not duplicated here.
"""

import json
import re
import statistics
import subprocess


REF = "3998053ffe7c5ee451476fcffb03cd8742796193"
PREFIX = "Benchmarks/Dictionary/performance-20261105/"


def read(name):
    return json.loads(subprocess.check_output(["git", "show", f"{REF}:{PREFIX}{name}"]))


def normalize(value):
    return re.sub("[^a-z0-9]", "", value.lower())


rows = read("native-paste.json")
coverage = {row["requestID"]: row for row in read("native-summary.json")["referenceCoverage"]}
assert len(rows) == len({row["requestID"] for row in rows}) == 24
assert len({(row["expectedAppPID"], row["captureGeneration"]) for row in rows}) == 24
for row in rows:
    assert row["guarded"] and row["done"] and row["clipboardRestored"]
    assert row["text"] == row["clipboardText"]
    assert row["eventSourcePID"] == row["expectedAppPID"]
    assert row["appClipboardGeneration"] == row["clipboardChangeCount"] == row["clipboardBefore"] + 1
    assert row["fixtureSHA256"] == row["loadedFixtureSHA256"]
    assert normalize(row["expectedAudioNonce"]) in normalize(row["text"])
    assert "AUDIT_SENTINEL_" not in row["text"]
    assert row["framesInjected"] >= round((row["seconds"] + 0.3) * row["inputSampleRate"])
    elapsed = (row["receivedUptime"] - row["stopUptime"]) * 1000
    assert elapsed > 0 and abs(row["milliseconds"] - elapsed) < 1e-6
    reference = coverage[row["requestID"]]
    assert len(row["text"]) >= 0.97 * reference["referenceCompletedCharacters"]
    for phrase in reference["tailAnchors"]:
        assert normalize(phrase) in normalize(row["text"][-250:])
    assert normalize(row["text"]).endswith(normalize(reference["tailAnchors"][-1]))

versions = ["v0.1.1-c72f834", "current-85dde51"]
for version in versions:
    for repeat in range(3):
        group = [row for row in rows if row["version"] == version and row["repeat"] == repeat]
        assert [row["seconds"] for row in group] == [5, 30, 120, 600]
        assert [row["captureGeneration"] for row in group] == [2, 3, 4, 5]
        assert len({row["expectedAppPID"] for row in group}) == 1

for seconds in [5, 30, 120, 600]:
    cell = [row for row in rows if row["seconds"] == seconds]
    assert len({row["fixtureSHA256"] for row in cell}) == 1
    lengths = [len(row["text"]) for row in cell]
    assert max(lengths) / min(lengths) <= 1.01
    medians = []
    for version in versions:
        values = [row["milliseconds"] for row in cell if row["version"] == version]
        assert len(values) == 3
        medians.append(statistics.median(values))
        print(seconds, version, "median/worst ms", round(medians[-1], 3), round(max(values), 3))
    print("median increase %", round((medians[1] / medians[0] - 1) * 100, 3))

print("24 fresh, complete target receipts passed; physical microphone/key latency excluded")

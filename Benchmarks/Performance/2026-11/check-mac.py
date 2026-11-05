"""Recompute the frozen M5 comparison without printing synthetic payloads.

Fetch PR #105 and #110 source objects first if absent (see README).
The original receiver payloads remain in the Mac audit, not duplicated here.
"""

import json
import re
import statistics
import subprocess


REF = "3998053ffe7c5ee451476fcffb03cd8742796193"
FOLLOWUP_REF = "d1b689a23b286dc5c8a40a925af6f70e88dc9c51"
PREFIX = "Benchmarks/Dictionary/performance-20261105/"


def read(name, ref=REF):
    return json.loads(subprocess.check_output(["git", "show", f"{ref}:{PREFIX}{name}"]))


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

components = read("attribution/stage-profile.json", FOLLOWUP_REF)
assert len(components) == 60
for seconds in [5, 30, 120, 600]:
    cell = [row for row in components if row["seconds"] == seconds]
    assert len({row["fixtureSHA256"] for row in cell}) == 1
    assert len({row["text"] for row in cell}) == 1
    assert {row["mode"] for row in cell} == {
        "legacy-url", "legacy-split", "current-production", "current-split", "current-no-vad"
    }
    for mode in {row["mode"] for row in cell}:
        group = [row for row in cell if row["mode"] == mode]
        assert len(group) == 3
        assert {row["repeatIndex"] for row in group} == {0, 1, 2}
    split = [row for row in cell if row["mode"] == "current-split"]
    assert all(row["samplesBefore"] == row["samplesAfter"] == seconds * 16000 for row in split)
    print(seconds, "isolated VAD median ms", statistics.median(row["milliseconds"]["VAD"] for row in split))

for filename, expected_count in [("cache-receipts.json", 8), ("production-receipts.json", 4)]:
    extra = read("attribution/" + filename, FOLLOWUP_REF)
    assert len(extra) == len({row["requestID"] for row in extra}) == expected_count
    for row in extra:
        assert row["guarded"] and row["clipboardRestored"] and row["done"]
        assert row["text"] == row["clipboardText"]
        assert row["eventSourcePID"] == row["expectedAppPID"]
        assert row["appClipboardGeneration"] == row["clipboardChangeCount"] == row["clipboardBefore"] + 1
        assert row["fixtureSHA256"] == row["loadedFixtureSHA256"]
        assert normalize(row["expectedAudioNonce"]) in normalize(row["text"])
        assert abs(row["milliseconds"] - (row["receivedUptime"] - row["stopUptime"]) * 1000) < 1e-6
        reference = next(item for item in coverage.values() if item["seconds"] == row["seconds"])
        assert len(row["text"]) >= 0.97 * reference["referenceCompletedCharacters"]
        for phrase in reference["tailAnchors"]:
            assert normalize(phrase) in normalize(row["text"][-250:])
    for seconds in {row["seconds"] for row in extra}:
        assert len({row["text"] for row in extra if row["seconds"] == seconds}) == 1
        for mode in {row["mode"] for row in extra}:
            values = [row["milliseconds"] for row in extra if row["seconds"] == seconds and row["mode"] == mode]
            if values:
                print(filename, seconds, mode, "n", len(values), "median/worst ms", statistics.median(values), max(values))
print("60 component trials and 12 targeted cache receipts passed; no universal end-to-end gain asserted")


# Fresh cohort: hashes were captured before launch, unlike the overwritten
# historical prototype. Keep its nine receipts separate from earlier cohorts.
ATTESTED_REF = "9c0a8bd08def9a3d6c312d3ca81431327b2e978b"
attested = read("attribution/attested/receipts.json", ATTESTED_REF)
provenance = read("attribution/attested/provenance.json", ATTESTED_REF)
integrity = read("attribution/attested/integrity-recheck.json", ATTESTED_REF)
assert len(attested) == len({row["requestID"] for row in attested}) == 9
assert all(all(flags.values()) for flags in integrity.values())
for row in attested:
    assert row["binding"] == provenance["bindings"][row["mode"]]
    assert row["guarded"] and row["clipboardRestored"] and row["done"]
    assert row["text"] == row["clipboardText"]
    assert row["eventSourcePID"] == row["expectedAppPID"]
    assert row["appClipboardGeneration"] == row["clipboardChangeCount"] == row["clipboardBefore"] + 1
    assert row["fixtureSHA256"] == row["loadedFixtureSHA256"]
    assert normalize(row["expectedAudioNonce"]) in normalize(row["text"])
    assert abs(row["milliseconds"] - (row["receivedUptime"] - row["stopUptime"]) * 1000) < 1e-6
for mode in ["old", "baseline", "cached", "production"]:
    for seconds in [5, 120]:
        values = [row["milliseconds"] for row in attested if row["mode"] == mode and row["seconds"] == seconds]
        if values:
            print("hash-bound", mode, seconds, "n", len(values), "median/worst ms", statistics.median(values), max(values))
assert read("attribution/attested/cleanup.json", ATTESTED_REF)["clipboardRestored"]
print("Nine hash-bound receipts passed; original 24-run matrix remains before the cache fix")

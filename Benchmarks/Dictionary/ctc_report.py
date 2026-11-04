"""Score real CTC/sidecar decodes, with a voice holdout and complete clip checks."""
import argparse
import copy
import hashlib
import json
import statistics
from pathlib import Path

import score


def timing(values):
    values = sorted(values)
    return {"median_ms": round(statistics.median(values), 3),
            "p95_ms": round(values[int(.95 * (len(values) - 1))], 3),
            "max_ms": round(max(values), 3)}


def report(bakeoff, bounded, clips):
    corpus_path = Path(__file__).with_name("corpus.json")
    corpus = json.loads(corpus_path.read_text())
    expected = {f"{clip['id']}-{voice}" for voice in corpus["voices"]
                for group in ("positive", "negative") for clip in corpus[group]}
    if set(bakeoff) != expected or set(bounded["records"]) != expected:
        raise ValueError("Every method must contain all 210 expected clips")
    methods = {name: {key: value[name] for key, value in bakeoff.items()}
               for name in ("raw", "spelling", "sidecar_plain", "sidecar_biased")}
    methods["bounded_acoustic"] = {key: value["text"] for key, value in bounded["records"].items()}
    methods["bounded_with_spelling"] = {key: value["with_spelling"] for key, value in bounded["records"].items()}
    results = {}
    for split, voices in (("development", corpus["voices"][:1]),
                          ("voice_holdout", corpus["voices"][1:]),
                          ("all", corpus["voices"])):
        score.CORPUS = copy.deepcopy(corpus)
        score.CORPUS["voices"] = voices
        negatives = [f"{clip['id']}-{voice}" for voice in voices for clip in corpus["negative"]]
        results[split] = {"voices": voices, "methods": {}}
        for name, texts in methods.items():
            measured = score.score(texts, methods["raw"])
            measured.pop("missing")
            measured.pop("false")
            measured["changed_negative_transcripts"] = sum(texts[key] != methods["raw"][key] for key in negatives)
            results[split]["methods"][name] = measured
    clip_hashes = {key: hashlib.sha256((clips / f"{key}.wav").read_bytes()).hexdigest()
                   for key in sorted(expected)}
    return {"corpus_sha256": hashlib.sha256(corpus_path.read_bytes()).hexdigest(),
            "clip_sha256": clip_hashes, "clip_count": len(expected),
            "split_limit": "Voice holdout shares sentences with development; no claim of unseen-text or microphone generalization.",
            "scores": results,
            "latency": {**{key: timing([value[key] for value in bakeoff.values()])
                            for key in ("ctc_ms", "plain_ms", "biased_ms")},
                        "bounded_pipeline": timing([value["elapsed_ms"] for value in bounded["records"].values()])},
            "cold_call_ms": bounded["cold_ms"], "deadline_probe": bounded["probe"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bakeoff", type=Path)
    parser.add_argument("bounded", type=Path)
    parser.add_argument("clips", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = report(json.loads(args.bakeoff.read_text()), json.loads(args.bounded.read_text()), args.clips)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")

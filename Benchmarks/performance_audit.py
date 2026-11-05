"""Offline ASR probe; never measures microphone, clipboard, or paste delivery."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import threading
import time
import wave


def run(binary, model, clips, dictionary):
    env = dict(os.environ)
    env.pop("KOETT_DICTIONARY", None)
    if dictionary:
        env["KOETT_DICTIONARY"] = str(dictionary)
    start = time.monotonic()
    proc = subprocess.Popen(
        [str(binary), str(model), *map(str, clips)],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    peak = [0]
    current_rss = [0]
    stopped = threading.Event()

    def sample():
        while not stopped.wait(0.01):
            try:
                status = Path(f"/proc/{proc.pid}/status").read_text()
                rss = int(re.search(r"VmRSS:\s+(\d+)", status)[1])
                current_rss[0] = rss
                peak[0] = max(peak[0], rss)
            except (OSError, TypeError):
                pass

    monitor = threading.Thread(target=sample)
    monitor.start()
    rows = []
    load_ms = None
    errors = []
    for line in proc.stderr:
        if line.startswith("model_load_ms="):
            load_ms = float(line.split("=")[1])
        match = re.search(r"audio_seconds=([\d.]+) transcribe_ms=([\d.]+)", line)
        if match:
            rows.append({"seconds": float(match[1]), "asr_ms": float(match[2]),
                         "rss_kib": current_rss[0],
                         "peak_rss_so_far_kib": peak[0]})
        elif "error:" in line:
            errors.append(line.strip())
    code = proc.wait()
    stopped.set()
    monitor.join()
    if code or len(rows) != len(clips):
        raise RuntimeError({"exit": code, "rows": len(rows), "errors": errors})
    return {"model_load_ms": load_ms, "wall_s": time.monotonic() - start,
            "peak_rss_kib": peak[0], "rows": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("binary", "model", "dictionary", "output"):
        parser.add_argument("--" + key, required=True, type=Path)
    args = parser.parse_args()
    for key in ("binary", "model", "dictionary", "output"):
        setattr(args, key, getattr(args, key).resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    with wave.open(str(args.model / "test_wavs/0.wav")) as source:
        params = source.getparams()
        frames = source.readframes(source.getnframes())
    clips = []
    for seconds in (5, 30, 120, 600):
        size = seconds * params.framerate * params.nchannels * params.sampwidth
        data = (frames * (size // len(frames) + 1))[:size]
        path = args.output / f"speech-{seconds}.wav"
        with wave.open(str(path), "wb") as dest:
            dest.setparams(params)
            dest.writeframes(data)
        clips.append(path)
    report = {"kind": "offline-asr-only", "loadavg_start": os.getloadavg(),
              "network_interfaces": Path("/proc/net/dev").read_text(),
              "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              "dictionary_sha256": hashlib.sha256(args.dictionary.read_bytes()).hexdigest(),
              "clips": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in clips},
              "cases": {}}
    path = args.output / "metrics.json"
    for name, dictionary in (("empty", None), ("seed", args.dictionary)):
        # Interleave durations, three repeats; all use one cached model.
        result = run(args.binary, args.model, clips * 3, dictionary)
        result["summary"] = {}
        for seconds in (5, 30, 120, 600):
            values = [r["asr_ms"] for r in result["rows"] if r["seconds"] == seconds]
            result["summary"][str(seconds)] = {
                "median_ms": statistics.median(values), "worst_ms": max(values),
                "runs": len(values),
            }
        report["cases"][name] = result
        path.write_text(json.dumps(report, indent=2) + "\n")
        print(name, result["summary"], flush=True)
    report["cases"]["soak_empty"] = run(args.binary, args.model, [clips[0]] * 100, None)
    report["loadavg_end"] = os.getloadavg()
    path.write_text(json.dumps(report, indent=2) + "\n")
    print("100 sequential offline inferences passed", flush=True)


if __name__ == "__main__":
    main()

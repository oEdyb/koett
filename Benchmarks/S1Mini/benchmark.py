#!/usr/bin/env python3

"""Small end-to-end S1-mini benchmark for Koett."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any
from urllib.error import URLError
from urllib.request import Request, urlopen


SYSTEM_PROMPT = (
    "You are a text normalizer for speech-to-text transcripts. The input begins "
    "with a control line specifying the styling, structure, and context settings; "
    "clean the transcript to match those settings and output only the cleaned text."
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--label", required=True)
    parser.add_argument("--cases", type=Path, default=Path(__file__).with_name("cases.json"))
    parser.add_argument("--case", action="append", dest="case_ids")
    parser.add_argument("--group", action="append", dest="groups")
    parser.add_argument("--style", choices=["casual", "semi-casual", "semi-formal", "formal"], default="semi-formal")
    parser.add_argument("--runtime", choices=["metal", "cpu"], default="metal")
    parser.add_argument("--spec", choices=["none", "ngram-simple", "ngram-mod", "ngram-cache"], default="none")
    parser.add_argument("--ngram-n", type=int, default=12)
    parser.add_argument("--ngram-m", type=int, default=48)
    parser.add_argument("--ngram-min-hits", type=int, default=1)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--flash-attn", choices=["on", "off", "auto"], default="on")
    parser.add_argument("--prompt-cache", choices=["on", "off"], default="on")
    parser.add_argument("--poll", type=int, choices=range(0, 101), default=50)
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--ubatch-size", type=int, default=512)
    parser.add_argument("--context", type=int, default=4096)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--port", type=int, default=18081)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def request_json(url: str, payload: dict[str, Any] | None = None, timeout: float = 30) -> dict[str, Any]:
    data = None if payload is None else json.dumps(payload).encode()
    request = Request(url, data=data, headers={"Content-Type": "application/json"})
    with urlopen(request, timeout=timeout) as response:
        return json.load(response)


def wait_until_ready(base_url: str, process: subprocess.Popen[bytes], timeout: float = 30) -> float:
    started = time.perf_counter()
    while time.perf_counter() - started < timeout:
        if process.poll() is not None:
            raise RuntimeError(f"llama-server exited with status {process.returncode}")
        try:
            if request_json(f"{base_url}/health", timeout=0.5).get("status") == "ok":
                return time.perf_counter() - started
        except (URLError, TimeoutError, OSError):
            pass
        time.sleep(0.025)
    raise TimeoutError("llama-server did not become ready")


def resident_bytes(pid: int) -> int:
    result = subprocess.run(
        ["/bin/ps", "-o", "rss=", "-p", str(pid)],
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        return int(result.stdout.strip()) * 1024
    except ValueError:
        return 0


class MemorySampler:
    def __init__(self, pid: int) -> None:
        self.pid = pid
        self.peak = 0
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._sample, daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> int:
        self._stop.set()
        self._thread.join()
        return self.peak

    def _sample(self) -> None:
        while not self._stop.wait(0.25):
            self.peak = max(self.peak, resident_bytes(self.pid))


def token_count(base_url: str, text: str) -> int:
    result = request_json(f"{base_url}/tokenize", {"content": text, "add_special": False})
    return len(result["tokens"])


def stream_completion(base_url: str, payload: dict[str, Any]) -> dict[str, Any]:
    request = Request(
        f"{base_url}/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    started = time.perf_counter()
    first_token_at: float | None = None
    pieces: list[str] = []
    usage: dict[str, Any] = {}
    timings: dict[str, Any] = {}
    with urlopen(request, timeout=180) as response:
        for raw_line in response:
            line = raw_line.decode().strip()
            if not line.startswith("data: ") or line == "data: [DONE]":
                continue
            event = json.loads(line[6:])
            if event.get("usage"):
                usage = event["usage"]
            if event.get("timings"):
                timings = event["timings"]
            choices = event.get("choices", [])
            if choices:
                content = choices[0].get("delta", {}).get("content", "")
                if content:
                    if first_token_at is None:
                        first_token_at = time.perf_counter()
                    pieces.append(content)
    ended = time.perf_counter()
    return {
        "output": "".join(pieces).strip(),
        "ttft_ms": None if first_token_at is None else (first_token_at - started) * 1000,
        "wall_ms": (ended - started) * 1000,
        "usage": usage,
        "timings": timings,
    }


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def text_metrics(text: str) -> dict[str, Any]:
    lowered = text.lower()
    fillers = sum(lowered.split().count(word) for word in ("um", "uh", "hmm"))
    return {
        "characters": len(text),
        "words": len(text.split()),
        "paragraphs": 0 if not text else len([part for part in text.split("\n\n") if part.strip()]),
        "fillers": fillers,
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def quality(case: dict[str, Any], output: str, enforce_exact: bool) -> dict[str, Any]:
    lowered = output.lower()
    missing = [item for item in case.get("required", []) if item.lower() not in lowered]
    present_forbidden = [item for item in case.get("forbidden", []) if item.lower() in lowered]
    expected = case.get("expected") if enforce_exact else None
    exact = None if expected is None else output == expected
    return {
        "passed": not missing and not present_forbidden and exact is not False,
        "exact_expected": exact,
        "missing_required": missing,
        "present_forbidden": present_forbidden,
    }


def load_cases(
    path: Path,
    root: Path,
    selected: set[str] | None,
    groups: set[str] | None,
) -> list[dict[str, Any]]:
    cases = json.loads(path.read_text())
    loaded: list[dict[str, Any]] = []
    for case in cases:
        if selected and case["id"] not in selected:
            continue
        if groups and case["group"] not in groups:
            continue
        item = dict(case)
        if "input_file" in item:
            text = (root / item["input_file"]).read_text().strip()
            if "word_limit" in item:
                words = text.split()
                offset = item.get("word_offset", 0)
                text = " ".join(words[offset:offset + item["word_limit"]])
            item["input"] = text
        loaded.append(item)
    if selected:
        missing = selected - {case["id"] for case in loaded}
        if missing:
            raise ValueError(f"Unknown case IDs: {', '.join(sorted(missing))}")
    return loaded


def main() -> int:
    args = parse_args()
    if args.repeats < 1 or args.threads < 1 or args.batch_size < 1 or args.ubatch_size < 1:
        raise ValueError("repeats, threads, batch size, and ubatch size must be positive")
    if args.context < 512:
        raise ValueError("context must be at least 512")
    if not args.model.is_file():
        raise FileNotFoundError(args.model)
    server = shutil.which("llama-server")
    if server is None:
        raise RuntimeError("llama-server is not installed")

    root = Path(__file__).resolve().parents[2]
    cases = load_cases(
        args.cases,
        root,
        set(args.case_ids) if args.case_ids else None,
        set(args.groups) if args.groups else None,
    )
    base_url = f"http://127.0.0.1:{args.port}"
    command = [
        server,
        "-m", str(args.model.resolve()),
        "--host", "127.0.0.1",
        "--port", str(args.port),
        "--jinja",
        "--chat-template-kwargs", '{"enable_thinking":false}',
        "--temp", "0",
        "--parallel", "1",
        "--ctx-size", str(args.context),
        "--batch-size", str(args.batch_size),
        "--ubatch-size", str(args.ubatch_size),
        "--threads", str(args.threads),
        "--threads-batch", str(args.threads),
        "--poll", str(args.poll),
        "--flash-attn", args.flash_attn,
        "--n-gpu-layers", "99" if args.runtime == "metal" else "0",
        "--no-ui",
        "--log-disable",
    ]
    command.append("--cache-prompt" if args.prompt_cache == "on" else "--no-cache-prompt")
    if args.spec != "none":
        command += ["--spec-type", args.spec]
    if args.spec == "ngram-simple":
        command += [
            "--spec-ngram-simple-size-n", str(args.ngram_n),
            "--spec-ngram-simple-size-m", str(args.ngram_m),
            "--spec-ngram-simple-min-hits", str(args.ngram_min_hits),
        ]

    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen(command, stdout=log, stderr=log)
        sampler = MemorySampler(process.pid)
        sampler.start()
        try:
            load_seconds = wait_until_ready(base_url, process)
            loaded_rss = resident_bytes(process.pid)
            runs: list[dict[str, Any]] = []
            for repeat in range(args.repeats):
                for case in cases:
                    transcript = case["input"]
                    transcript_tokens = token_count(base_url, transcript)
                    max_tokens = math.ceil(1.3 * transcript_tokens + 32)
                    styling = case.get("styling", args.style)
                    structure = case.get("structure", "prose")
                    context = case.get("context", "general")
                    enforce_exact = styling == "semi-formal" and structure == "prose" and context == "general"
                    control = f"[Styling: {styling}] [Structure: {structure}] [Context: {context}]"
                    result = stream_completion(base_url, {
                        "model": "s1-mini",
                        "messages": [
                            {"role": "system", "content": SYSTEM_PROMPT},
                            {"role": "user", "content": f"{control}\n{transcript}"},
                        ],
                        "temperature": 0,
                        "max_tokens": max_tokens,
                        "cache_prompt": args.prompt_cache == "on",
                        "chat_template_kwargs": {"enable_thinking": False},
                        "stream": True,
                        "stream_options": {"include_usage": True},
                    })
                    result.update({
                        "case": case["id"],
                        "group": case["group"],
                        "repeat": repeat + 1,
                        "input_tokens": transcript_tokens,
                        "max_tokens": max_tokens,
                        "quality": quality(case, result["output"], enforce_exact),
                        "text_metrics": text_metrics(result["output"]),
                        "output_sha256": hashlib.sha256(result["output"].encode()).hexdigest(),
                    })
                    runs.append(result)
                    print(
                        f"{case['id']} {repeat + 1}/{args.repeats}: "
                        f"{result['ttft_ms'] or 0:.1f} ms first, {result['wall_ms']:.1f} ms total, "
                        f"quality={'pass' if result['quality']['passed'] else 'FAIL'}"
                    )
        finally:
            peak_rss = sampler.stop()
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

    ttft = [run["ttft_ms"] for run in runs if run["ttft_ms"] is not None]
    wall = [run["wall_ms"] for run in runs]
    summary = {
        "runs": len(runs),
        "quality_passes": sum(run["quality"]["passed"] for run in runs),
        "stable_cases": sum(
            len({run["output_sha256"] for run in runs if run["case"] == case["id"]}) == 1
            for case in cases
        ),
        "case_count": len(cases),
        "ttft_ms": {"p50": percentile(ttft, 0.5), "p95": percentile(ttft, 0.95)},
        "wall_ms": {"p50": percentile(wall, 0.5), "p95": percentile(wall, 0.95)},
    }
    version = subprocess.run([server, "--version"], capture_output=True, text=True)
    report = {
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "machine": {
            "platform": sys.platform,
            "architecture": os.uname().machine,
        },
        "model": {
            "label": args.label,
            "path": str(args.model.resolve()),
            "bytes": args.model.stat().st_size,
            "sha256": sha256_file(args.model),
        },
        "runtime": {
            "llama_server": (version.stdout + version.stderr).strip(),
            "mode": args.runtime,
            "spec": args.spec,
            "ngram_n": args.ngram_n if args.spec == "ngram-simple" else None,
            "ngram_m": args.ngram_m if args.spec == "ngram-simple" else None,
            "ngram_min_hits": args.ngram_min_hits if args.spec == "ngram-simple" else None,
            "threads": args.threads,
            "poll": args.poll,
            "flash_attention": args.flash_attn,
            "prompt_cache": args.prompt_cache,
            "context": args.context,
            "batch_size": args.batch_size,
            "ubatch_size": args.ubatch_size,
            "style": args.style,
            "cold_load_ms": load_seconds * 1000,
            "loaded_rss_bytes": loaded_rss,
            "peak_rss_bytes": peak_rss,
        },
        "summary": summary,
        "runs": runs,
    }
    output = args.output or (
        Path(__file__).with_name("Results")
        / f"{time.strftime('%Y%m%d-%H%M%S')}-{args.label}-{args.runtime}-{args.spec}-{args.style}.json"
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(f"Saved {output}")
    return 0 if summary["quality_passes"] == summary["runs"] else 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)

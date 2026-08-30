#!/usr/bin/env python3
"""Run one honest Swedish ASR accuracy and resource bakeoff."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import platform
import random
import re
import struct
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

BENCHMARKS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(BENCHMARKS))

import benchmark_record  # noqa: E402

FLEURS_TSV_SHA256 = "55f48c5385a6e5fb8a62ea90212c04b005e2f77d7bd8fcf20bc3a5bda223aae2"
KB_MODEL_SHA256 = "aead29b356bca8840e72a8dc2286e2d69e6702639751a1e60cb3c8eacefec546"
WHISPER_REVISION = "371b5a7561823ab2bb32142d2751e35e7534727b"
FLUID_REVISION = "4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b"


@dataclass(frozen=True)
class Fixture:
    fixture_id: str
    filename: str
    reference: str
    audio_path: Path
    audio_seconds: float
    sha256: str


@dataclass(frozen=True)
class WaveInfo:
    channels: int
    sample_rate: int
    frames: int


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def wave_info(path: Path) -> WaveInfo:
    """Read the small RIFF fields shared by integer and IEEE Float PCM WAV."""
    file_size = path.stat().st_size
    with path.open("rb") as handle:
        header = handle.read(12)
        if len(header) != 12 or header[:4] != b"RIFF" or header[8:] != b"WAVE":
            raise ValueError(f"audio is not a RIFF WAVE file: {path}")
        format_fields = None
        data_bytes = None
        while handle.tell() + 8 <= file_size:
            chunk_id, chunk_size = struct.unpack("<4sI", handle.read(8))
            chunk_end = handle.tell() + chunk_size
            if chunk_end > file_size:
                raise ValueError(f"WAV chunk exceeds the file: {path}")
            if chunk_id == b"fmt ":
                if chunk_size < 16:
                    raise ValueError(f"WAV fmt chunk is too short: {path}")
                format_fields = struct.unpack("<HHIIHH", handle.read(16))
                handle.seek(chunk_end)
            elif chunk_id == b"data":
                data_bytes = chunk_size
                handle.seek(chunk_end)
            else:
                handle.seek(chunk_end)
            if chunk_size % 2:
                handle.seek(1, 1)
        if format_fields is None or data_bytes is None:
            raise ValueError(f"WAV is missing fmt or data: {path}")

    format_tag, channels, sample_rate, _, block_align, bits_per_sample = format_fields
    if format_tag not in {1, 3}:
        raise ValueError(f"WAV format {format_tag} is not uncompressed PCM: {path}")
    expected_align = channels * bits_per_sample // 8
    if channels < 1 or bits_per_sample % 8 or block_align != expected_align:
        raise ValueError(f"WAV block alignment is invalid: {path}")
    if data_bytes % block_align:
        raise ValueError(f"WAV data is not a whole number of frames: {path}")
    return WaveInfo(channels, sample_rate, data_bytes // block_align)


def load_fleurs(tsv_path: Path, audio_directory: Path, limit: int | None) -> list[Fixture]:
    if sha256_file(tsv_path) != FLEURS_TSV_SHA256:
        raise ValueError("FLEURS TSV SHA-256 does not match the pinned test set")

    fixtures = []
    with tsv_path.open(encoding="utf-8", newline="") as handle:
        for row_index, row in enumerate(
            csv.reader(handle, delimiter="\t", quoting=csv.QUOTE_NONE), 1
        ):
            if len(row) != 7:
                raise ValueError(f"FLEURS row {row_index} has {len(row)} fields, expected 7")
            _, filename, _, normalized, _, sample_count, _ = row
            audio_path = audio_directory / filename
            if not audio_path.is_file():
                raise ValueError(f"missing FLEURS audio: {audio_path}")
            audio = wave_info(audio_path)
            if audio.channels != 1 or audio.sample_rate != 16_000:
                raise ValueError(f"FLEURS audio is not mono 16 kHz: {audio_path}")
            if audio.frames != int(sample_count):
                raise ValueError(f"FLEURS sample count does not match: {audio_path}")
            fixtures.append(Fixture(
                fixture_id=f"fleurs-sv-{Path(filename).stem}",
                filename=filename,
                reference=normalized,
                audio_path=audio_path,
                audio_seconds=int(sample_count) / 16_000,
                sha256=sha256_file(audio_path),
            ))
            if limit is not None and len(fixtures) == limit:
                break
    if not fixtures:
        raise ValueError("FLEURS test set is empty")
    return fixtures


def write_corpus(path: Path, fixtures: list[Fixture]) -> None:
    fields = [
        "id", "category", "speaker_id", "publishable", "expected_empty",
        "reference", "local_file", "sha256", "protected_terms",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, delimiter="\t", fieldnames=fields)
        writer.writeheader()
        for fixture in fixtures:
            writer.writerow({
                "id": fixture.fixture_id,
                "category": "swedish-public",
                "speaker_id": "fleurs-sv-speaker-id-unavailable",
                "publishable": "true",
                "expected_empty": "false",
                "reference": fixture.reference,
                "local_file": str(fixture.audio_path),
                "sha256": fixture.sha256,
                "protected_terms": "[]",
            })


def parse_time_receipt(text: str) -> dict[str, float | int]:
    first = re.search(
        r"^\s*([0-9.]+) real\s+([0-9.]+) user\s+([0-9.]+) sys", text, re.MULTILINE
    )
    rss = re.search(r"^\s*(\d+)\s+maximum resident set size", text, re.MULTILINE)
    if first is None or rss is None:
        raise ValueError("could not parse macOS /usr/bin/time receipt")
    return {
        "wall_seconds": float(first.group(1)),
        "user_seconds": float(first.group(2)),
        "system_seconds": float(first.group(3)),
        "peak_rss_bytes": int(rss.group(1)),
    }


def run_timed(label: str, command: list[str], output: Path) -> dict[str, Any]:
    stdout_path = output / f"{label}.stdout.log"
    stderr_path = output / f"{label}.stderr.log"
    time_path = output / f"{label}.time.txt"
    started = time.monotonic()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        completed = subprocess.run(
            ["/usr/bin/time", "-l", "-o", str(time_path), *command],
            stdout=stdout,
            stderr=stderr,
            check=False,
        )
    receipt = parse_time_receipt(time_path.read_text(encoding="utf-8"))
    receipt.update({
        "label": label,
        "exit_code": completed.returncode,
        "observed_wall_seconds": time.monotonic() - started,
        "command_sha256": hashlib.sha256("\0".join(command).encode()).hexdigest(),
        "argument_count": len(command),
        "stdout": stdout_path.name,
        "stderr": stderr_path.name,
    })
    return receipt


def tree_receipt(path: Path) -> tuple[str, int]:
    entries = []
    total = 0
    for file in sorted(candidate for candidate in path.rglob("*") if candidate.is_file()):
        relative = file.relative_to(path).as_posix()
        size = file.stat().st_size
        total += size
        entries.append(f"{relative}\0{size}\0{sha256_file(file)}\n")
    if not entries:
        raise ValueError(f"model directory is empty: {path}")
    return hashlib.sha256("".join(entries).encode()).hexdigest(), total


def whisper_text(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    return "".join(segment["text"] for segment in payload["transcription"]).strip()


def system_identity() -> dict[str, str]:
    hardware = subprocess.check_output(
        ["sysctl", "-n", "machdep.cpu.brand_string"], text=True
    ).strip()
    power = subprocess.check_output(["pmset", "-g", "batt"], text=True).splitlines()[0]
    return {
        "platform": f"macOS-{platform.machine()}",
        "os_version": platform.mac_ver()[0],
        "hardware": hardware,
        "power_mode": "AC" if "AC Power" in power else "battery",
    }


def base_record(
    *, configuration: str, fixture: Fixture, repeat: int, app_commit: str,
    identity: dict[str, str], model_identity: dict[str, str], hypothesis: str,
    success: bool, engine_ms: float | None, failure_message: str = "",
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "run_id": f"{configuration}-{repeat}-{fixture.fixture_id}",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "stage": "raw_asr",
        "source_run_id": "",
        "configuration": configuration,
        "app_commit": app_commit,
        **model_identity,
        "formatter": "none",
        "formatter_revision": "none",
        "formatting_profile": "none",
        "normalizer_revision": "koett-words-v1",
        **identity,
        "target": "engine-batch",
        "fixture_id": fixture.fixture_id,
        "fixture_sha256": fixture.sha256,
        "repeat": repeat,
        "success": success,
        "failure_stage": "" if success else "engine",
        "failure_code": "" if success else "process-failed",
        "failure_message": failure_message,
        "hypothesis": hypothesis,
        "audio_seconds": fixture.audio_seconds,
        "engine_ms": engine_ms,
        "timing_scope": "accuracy",
    }


def run_parakeet(
    version: str, binary: Path, fixtures: list[Fixture], repeat: int,
    app_commit: str, identity: dict[str, str], output: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    configuration = f"parakeet-{version}"
    warmup_results = output / f"{configuration}-warmup.json"
    warmup = run_timed(
        f"{configuration}-warmup",
        [
            str(binary), "--model-version", version, "--prewarm",
            "--results-json", str(warmup_results), str(fixtures[0].audio_path),
        ],
        output,
    )
    results_path = output / f"{configuration}-{repeat}.json"
    command = [
        str(binary), "--model-version", version, "--prewarm",
        "--results-json", str(results_path),
        *(str(fixture.audio_path) for fixture in fixtures),
    ]
    receipt = run_timed(f"{configuration}-{repeat}", command, output)
    receipt["warmup"] = warmup
    receipt["runtime_binary_bytes"] = binary.stat().st_size
    receipt["runtime_binary_sha256"] = sha256_file(binary)
    model_directory = Path.home() / "Library/Application Support/FluidAudio/Models" / (
        "parakeet-tdt-0.6b-v2" if version == "v2" else "parakeet-tdt-0.6b-v3"
    )
    if model_directory.is_dir():
        tree_hash, model_bytes = tree_receipt(model_directory)
    else:
        tree_hash, model_bytes = "unavailable-after-failure", 0
    receipt.update({"model_bytes": model_bytes, "model_tree_sha256": tree_hash})
    model_identity = {
        "adapter": "Koett ParakeetBaseline",
        "adapter_revision": app_commit,
        "runtime": "FluidAudio 0.15.6",
        "model": f"FluidInference/parakeet-tdt-0.6b-{version}-coreml",
        "model_revision": tree_hash,
        "decoder": "FluidAudio TDT default",
        "decoder_revision": FLUID_REVISION,
    }
    if warmup["exit_code"] != 0 or receipt["exit_code"] != 0:
        message = (
            f"parakeet warmup exited {warmup['exit_code']}; "
            f"scored process exited {receipt['exit_code']}"
        )
        return [
            base_record(
                configuration=configuration, fixture=fixture, repeat=repeat,
                app_commit=app_commit, identity=identity, model_identity=model_identity,
                hypothesis="", success=False, engine_ms=None, failure_message=message,
            ) for fixture in fixtures
        ], receipt
    payload = json.loads(results_path.read_text(encoding="utf-8"))
    by_file = {item["audioFile"]: item for item in payload}
    records = []
    for fixture in fixtures:
        item = by_file.get(fixture.filename)
        success = item is not None
        records.append(base_record(
            configuration=configuration, fixture=fixture, repeat=repeat,
            app_commit=app_commit, identity=identity, model_identity=model_identity,
            hypothesis="" if item is None else item["text"], success=success,
            engine_ms=None if item is None else item["engineMilliseconds"],
            failure_message="missing structured result" if item is None else "",
        ))
    receipt["exposed_engine_seconds"] = sum(
        item["engineMilliseconds"] for item in payload
    ) / 1_000
    return records, receipt


def run_whisper(
    binary: Path, model: Path, fixtures: list[Fixture], repeat: int,
    app_commit: str, identity: dict[str, str], output: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    configuration = "kb-whisper-base-q5"
    transcript_root = output / f"{configuration}-{repeat}-transcripts"
    transcript_root.mkdir(exist_ok=True)
    warmup = run_timed(
        f"{configuration}-warmup",
        [
            str(binary), "-m", str(model), "-l", "sv", "-oj", "-np", "-nt",
            "-of", str(output / f"{configuration}-warmup"), str(fixtures[0].audio_path),
        ],
        output,
    )
    command = [str(binary), "-m", str(model), "-l", "sv", "-oj", "-np", "-nt"]
    for fixture in fixtures:
        command.extend(["-of", str(transcript_root / fixture.fixture_id)])
    command.extend(str(fixture.audio_path) for fixture in fixtures)
    receipt = run_timed(f"{configuration}-{repeat}", command, output)
    receipt.update({
        "warmup": warmup,
        "model_bytes": model.stat().st_size,
        "model_sha256": sha256_file(model),
        "runtime_binary_bytes": binary.stat().st_size,
        "runtime_binary_sha256": sha256_file(binary),
    })
    model_identity = {
        "adapter": "whisper.cpp CLI",
        "adapter_revision": WHISPER_REVISION,
        "runtime": "whisper.cpp 1.9.3 Metal",
        "model": "KBLab/kb-whisper-base ggml Q5_0",
        "model_revision": KB_MODEL_SHA256,
        "decoder": "whisper.cpp default beam-size 5 best-of 5",
        "decoder_revision": WHISPER_REVISION,
    }
    records = []
    for fixture in fixtures:
        transcript = transcript_root / f"{fixture.fixture_id}.json"
        success = warmup["exit_code"] == 0 and receipt["exit_code"] == 0 and transcript.is_file()
        hypothesis = ""
        failure_message = ""
        if success:
            try:
                hypothesis = whisper_text(transcript)
            except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
                success = False
                failure_message = f"invalid whisper JSON: {error}"
        if not success and not failure_message:
            failure_message = (
                f"whisper warmup exited {warmup['exit_code']}; "
                f"scored process exited {receipt['exit_code']}"
            )
        records.append(base_record(
            configuration=configuration, fixture=fixture, repeat=repeat,
            app_commit=app_commit, identity=identity, model_identity=model_identity,
            hypothesis=hypothesis, success=success,
            engine_ms=None,
            failure_message=failure_message,
        ))
    return records, receipt


def report(scored: list[dict[str, Any]], receipts: list[dict[str, Any]]) -> str:
    rows = []
    configurations = sorted({record["configuration"] for record in scored})
    for configuration in configurations:
        records = [record for record in scored if record["configuration"] == configuration]
        total_audio = sum(record["audio_seconds"] for record in records)
        reference_words = sum(record["reference_words"] for record in records)
        errors = sum(record["errors"] for record in records)
        reference_characters = sum(record["reference_characters"] for record in records)
        character_errors = sum(record["character_errors"] for record in records)
        configuration_receipts = [
            item for item in receipts if item["label"].startswith(f"{configuration}-")
        ]
        wall_seconds = sum(item["wall_seconds"] for item in configuration_receipts)
        engine_values = [
            item["exposed_engine_seconds"] for item in configuration_receipts
            if "exposed_engine_seconds" in item
        ]
        engine_seconds = sum(engine_values) if engine_values else None
        peak_rss_bytes = max(item["peak_rss_bytes"] for item in configuration_receipts)
        model_bytes = configuration_receipts[0]["model_bytes"]
        rows.append(
            f"| {configuration} | {100 * errors / reference_words:.2f}% | "
            f"{100 * character_errors / reference_characters:.2f}% | "
            f"{total_audio / wall_seconds:.1f}x | "
            f"{'—' if engine_seconds is None else f'{total_audio / engine_seconds:.1f}x'} | "
            f"{peak_rss_bytes / 1_000_000:.0f} MB | "
            f"{model_bytes / 1_000_000:.1f} MB | "
            f"{sum(not record['success'] for record in records)} |"
        )
    return "\n".join([
        "# Koett Swedish model bakeoff",
        "",
        f"Audio: {len(scored) // len(configurations)} scored FLEURS Swedish trials, "
        f"{sum(record['audio_seconds'] for record in scored) / len(configurations) / 60:.1f} "
        "minutes per model.",
        "",
        "| Configuration | WER | CER | Full-process RTFx | Exposed engine RTFx | Peak RSS | Model | Failures |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
        *rows,
        "",
        "Full-process RTFx includes model load and prewarm. FluidAudio exposes per-file engine time; "
        "whisper.cpp CLI does not, so its engine cell stays empty.",
        "",
        "FLEURS does not publish stable speaker IDs in this TSV. This run can compare aggregate "
        "accuracy, speed, memory, and size, but it cannot pass Koett's speaker-block confidence "
        "or public model-promotion gate.",
        "",
    ])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parakeet", required=True, type=Path)
    parser.add_argument("--whisper", required=True, type=Path)
    parser.add_argument("--kb-model", required=True, type=Path)
    parser.add_argument("--fleurs-tsv", required=True, type=Path)
    parser.add_argument("--fleurs-audio", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--seed", type=int, default=20260830)
    return parser.parse_args()


def main() -> int:
    arguments = parse_args()
    if arguments.limit is not None and arguments.limit < 1:
        raise ValueError("--limit must be positive")
    if arguments.repeats < 1:
        raise ValueError("--repeats must be positive")
    for path in (arguments.parakeet, arguments.whisper, arguments.kb_model):
        if not path.is_file():
            raise ValueError(f"missing runner or model: {path}")
    if sha256_file(arguments.kb_model) != KB_MODEL_SHA256:
        raise ValueError("KB-Whisper model SHA-256 does not match")

    repo = BENCHMARKS.parent
    exact_paths = [
        "Sources/ParakeetBaseline/ParakeetBaseline.swift",
        "Benchmarks/README.md",
        "Benchmarks/benchmark_record.py",
        "Benchmarks/benchmark_schema.py",
        "Benchmarks/Swedish",
    ]
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--", *exact_paths], cwd=repo, text=True
    ).strip()
    if dirty:
        raise ValueError("commit the benchmark adapter and harness before running")
    app_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True
    ).strip()
    arguments.output.mkdir(parents=True, exist_ok=True)
    fixtures = load_fleurs(arguments.fleurs_tsv, arguments.fleurs_audio, arguments.limit)
    corpus_path = arguments.output / "corpus.tsv"
    write_corpus(corpus_path, fixtures)
    identity = system_identity()

    configurations = ["parakeet-v2", "parakeet-v3", "kb-whisper-base-q5"]
    records = []
    receipts = []
    for repeat in range(1, arguments.repeats + 1):
        order = list(configurations)
        random.Random(arguments.seed + repeat).shuffle(order)
        for configuration in order:
            if configuration == "parakeet-v2":
                batch, receipt = run_parakeet(
                    "v2", arguments.parakeet, fixtures, repeat, app_commit,
                    identity, arguments.output,
                )
            elif configuration == "parakeet-v3":
                batch, receipt = run_parakeet(
                    "v3", arguments.parakeet, fixtures, repeat, app_commit,
                    identity, arguments.output,
                )
            else:
                batch, receipt = run_whisper(
                    arguments.whisper, arguments.kb_model, fixtures, repeat,
                    app_commit, identity, arguments.output,
                )
            receipt.update({
                "repeat": repeat,
                "audio_files": len(fixtures),
                "audio_seconds": sum(fixture.audio_seconds for fixture in fixtures),
            })
            records.extend(batch)
            receipts.append(receipt)

    raw_path = arguments.output / "runs.jsonl"
    benchmark_record.write_records(raw_path, records)
    corpus = benchmark_record.load_corpus(corpus_path)
    scored = [benchmark_record.score_record(record, corpus[record["fixture_id"]]) for record in records]
    benchmark_record.write_records(arguments.output / "scored.tsv", scored)
    (arguments.output / "summary.json").write_text(
        json.dumps(benchmark_record.summarize(scored), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (arguments.output / "receipts.json").write_text(
        json.dumps(receipts, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (arguments.output / "REPORT.md").write_text(report(scored, receipts), encoding="utf-8")
    print(arguments.output / "REPORT.md")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)

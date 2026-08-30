#!/usr/bin/env python3
"""Run one honest Swedish ASR accuracy and resource bakeoff."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
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
FLEURS_EXPECTED_ROWS = 759
KB_MODEL_SHA256 = "aead29b356bca8840e72a8dc2286e2d69e6702639751a1e60cb3c8eacefec546"
KB_REVISION = "1499d2d2f0c7ed545bd6f2eec85287cf8d8c8b38"
WHISPER_REVISION = "371b5a7561823ab2bb32142d2751e35e7534727b"
FLUID_REVISION = "4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b"
PARAKEET_MANIFEST = Path(__file__).with_name("parakeet-models.tsv")
PARAKEET_LOCAL_FOLDERS = {
    "v2": "parakeet-tdt-0.6b-v2",
    "v3": "parakeet-tdt-0.6b-v3",
}


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


@dataclass(frozen=True)
class ModelManifest:
    repo: str
    revision: str
    files: dict[str, tuple[int, str]]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def cmake_cache_value(text: str, key: str) -> str:
    match = re.search(rf"^{re.escape(key)}:[^=]+=(.*)$", text, re.MULTILINE)
    if match is None:
        raise ValueError(f"CMake cache is missing {key}")
    return match.group(1).strip()


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


def load_fleurs(
    tsv_path: Path, audio_directory: Path, limit: int | None,
    expected_rows: int = FLEURS_EXPECTED_ROWS,
) -> list[Fixture]:
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
    if limit is None and len(fixtures) != expected_rows:
        raise ValueError(
            f"FLEURS test set has {len(fixtures)} rows, expected {expected_rows}"
        )
    return fixtures


def load_model_manifests(path: Path) -> dict[str, ModelManifest]:
    grouped: dict[str, dict[str, Any]] = {}
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        expected = {"version", "repo", "revision", "path", "bytes", "sha256"}
        if set(reader.fieldnames or []) != expected:
            raise ValueError("Parakeet model manifest has unexpected columns")
        for row in reader:
            version = row["version"]
            group = grouped.setdefault(
                version,
                {"repo": row["repo"], "revision": row["revision"], "files": {}},
            )
            if group["repo"] != row["repo"] or group["revision"] != row["revision"]:
                raise ValueError(f"Parakeet {version} manifest identity is inconsistent")
            relative = row["path"]
            if not relative or relative.startswith("/") or ".." in Path(relative).parts:
                raise ValueError(f"Parakeet {version} manifest path is unsafe")
            if relative in group["files"]:
                raise ValueError(f"Parakeet {version} manifest repeats {relative}")
            digest = row["sha256"]
            if not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise ValueError(f"Parakeet {version} manifest SHA-256 is invalid")
            group["files"][relative] = (int(row["bytes"]), digest)
    if set(grouped) != {"v2", "v3"}:
        raise ValueError("Parakeet model manifest must contain v2 and v3")
    return {
        version: ModelManifest(group["repo"], group["revision"], group["files"])
        for version, group in grouped.items()
    }


def verify_model_directory(
    path: Path,
    manifest: ModelManifest,
    expected_name: str | None = None,
) -> tuple[str, int]:
    if expected_name is not None and path.name != expected_name:
        raise ValueError(
            f"Parakeet model directory needs FluidAudio cache name "
            f"{expected_name}, got {path.name}"
        )
    actual = {
        file.relative_to(path).as_posix()
        for file in path.rglob("*")
        if file.is_file()
    }
    expected = set(manifest.files)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise ValueError(
            f"Parakeet model directory does not match its manifest; "
            f"missing={missing}, extra={extra}"
        )
    for relative, (expected_bytes, expected_sha256) in manifest.files.items():
        file = path / relative
        if file.stat().st_size != expected_bytes or sha256_file(file) != expected_sha256:
            raise ValueError(f"Parakeet model file does not match: {file}")
    return tree_receipt(path)


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
    if not isinstance(payload, dict) or not isinstance(payload.get("transcription"), list):
        raise ValueError("whisper JSON is missing its transcription array")
    text = []
    for segment in payload["transcription"]:
        if not isinstance(segment, dict) or not isinstance(segment.get("text"), str):
            raise ValueError("whisper JSON has an invalid segment")
        text.append(segment["text"])
    return "".join(text).strip()


def parakeet_result(path: Path, fixture: Fixture) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    expected_fields = {
        "audioFile", "audioSeconds", "engineMilliseconds", "realTimeFactor",
        "confidence", "text",
    }
    if not isinstance(payload, dict) or set(payload) != expected_fields:
        raise ValueError("Parakeet result has unexpected fields")
    if payload["audioFile"] != fixture.filename or not isinstance(payload["text"], str):
        raise ValueError("Parakeet result does not match its audio file")
    for field in ("audioSeconds", "engineMilliseconds", "realTimeFactor", "confidence"):
        value = payload[field]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
            raise ValueError(f"Parakeet result has invalid {field}")
    if abs(payload["audioSeconds"] - fixture.audio_seconds) > 0.001:
        raise ValueError("Parakeet result audio duration does not match")
    if payload["engineMilliseconds"] <= 0 or payload["realTimeFactor"] <= 0:
        raise ValueError("Parakeet result timing must be positive")
    expected_rtf = payload["engineMilliseconds"] / 1_000 / payload["audioSeconds"]
    if abs(payload["realTimeFactor"] - expected_rtf) > 0.000_001:
        raise ValueError("Parakeet result timing fields disagree")
    if not 0 <= payload["confidence"] <= 1:
        raise ValueError("Parakeet confidence is outside zero through one")
    return payload


def read_parakeet_outputs(
    root: Path, fixtures: list[Fixture]
) -> tuple[dict[str, tuple[dict[str, Any] | None, str]], set[str], set[str]]:
    expected = {f"{Path(fixture.filename).stem}.json" for fixture in fixtures}
    actual = {file.name for file in root.glob("*.json")} if root.is_dir() else set()
    outputs = {}
    for fixture in fixtures:
        path = root / f"{Path(fixture.filename).stem}.json"
        if not path.is_file():
            outputs[fixture.fixture_id] = (None, "missing structured result")
            continue
        try:
            outputs[fixture.fixture_id] = (parakeet_result(path, fixture), "")
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            outputs[fixture.fixture_id] = (None, f"invalid Parakeet JSON: {error}")
    return outputs, actual, expected


def read_whisper_outputs(
    root: Path, fixtures: list[Fixture]
) -> tuple[dict[str, tuple[str | None, str]], set[str], set[str]]:
    expected = {f"{fixture.fixture_id}.json" for fixture in fixtures}
    actual = {file.name for file in root.glob("*.json")} if root.is_dir() else set()
    outputs = {}
    for fixture in fixtures:
        path = root / f"{fixture.fixture_id}.json"
        if not path.is_file():
            outputs[fixture.fixture_id] = (None, "missing structured result")
            continue
        try:
            outputs[fixture.fixture_id] = (whisper_text(path), "")
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            outputs[fixture.fixture_id] = (None, f"invalid whisper JSON: {error}")
    return outputs, actual, expected


def checkpoint(output: Path, records: list[dict[str, Any]], receipts: list[dict[str, Any]]) -> None:
    records_path = output / "runs.partial.jsonl"
    receipts_path = output / "receipts.partial.json"
    records_temporary = output / "runs.partial.new.jsonl"
    receipts_temporary = output / "receipts.partial.new.json"
    benchmark_record.write_records(records_temporary, records)
    receipts_temporary.write_text(
        json.dumps(receipts, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    records_temporary.replace(records_path)
    receipts_temporary.replace(receipts_path)


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
    version: str, binary: Path, model_directory: Path, manifest: ModelManifest,
    fixtures: list[Fixture], repeat: int, app_commit: str,
    identity: dict[str, str], output: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    configuration = f"parakeet-{version}"
    warmup_results = output / f"{configuration}-{repeat}-warmup.json"
    warmup = run_timed(
        f"{configuration}-{repeat}-warmup",
        [
            str(binary), "--model-version", version,
            "--model-directory", str(model_directory), "--prewarm",
            "--results-json", str(warmup_results), str(fixtures[0].audio_path),
        ],
        output,
    )
    results_root = output / f"{configuration}-{repeat}-results"
    command = [
        str(binary), "--model-version", version,
        "--model-directory", str(model_directory),
        "--results-directory", str(results_root),
        *(str(fixture.audio_path) for fixture in fixtures),
    ]
    receipt = run_timed(f"{configuration}-{repeat}", command, output)
    receipt["warmup"] = warmup
    receipt["runtime_binary_bytes"] = binary.stat().st_size
    binary_hash = sha256_file(binary)
    receipt["runtime_binary_sha256"] = binary_hash
    tree_hash, model_bytes = verify_model_directory(
        model_directory,
        manifest,
        expected_name=PARAKEET_LOCAL_FOLDERS[version],
    )
    receipt.update({"model_bytes": model_bytes, "model_tree_sha256": tree_hash})
    model_identity = {
        "adapter": "Koett ParakeetBaseline",
        "adapter_revision": f"sha256:{binary_hash}",
        "runtime": f"FluidAudio 0.15.6 at {FLUID_REVISION}",
        "model": manifest.repo,
        "model_revision": manifest.revision,
        "decoder": "FluidAudio TDT default",
        "decoder_revision": FLUID_REVISION,
    }
    records = []
    engine_seconds = 0.0
    outputs, actual_files, expected_files = read_parakeet_outputs(results_root, fixtures)
    for fixture in fixtures:
        item, failure_message = outputs[fixture.fixture_id]
        if item is not None:
            engine_seconds += item["engineMilliseconds"] / 1_000
        records.append(base_record(
            configuration=configuration, fixture=fixture, repeat=repeat,
            app_commit=app_commit, identity=identity, model_identity=model_identity,
            hypothesis="" if item is None else item["text"], success=item is not None,
            engine_ms=None if item is None else item["engineMilliseconds"],
            failure_message=failure_message,
        ))
    exact_output_set = actual_files == expected_files
    receipt["exposed_engine_seconds"] = engine_seconds
    receipt["batch_complete"] = (
        receipt["exit_code"] == 0
        and exact_output_set
        and all(record["success"] for record in records)
    )
    receipt["unexpected_output_files"] = sorted(actual_files - expected_files)
    receipt["missing_output_files"] = sorted(expected_files - actual_files)
    return records, receipt


def run_whisper(
    binary: Path, model: Path, fixtures: list[Fixture], repeat: int,
    app_commit: str, identity: dict[str, str], output: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    configuration = "kb-whisper-base-q5"
    transcript_root = output / f"{configuration}-{repeat}-transcripts"
    transcript_root.mkdir()
    warmup = run_timed(
        f"{configuration}-{repeat}-warmup",
        [
            str(binary), "-m", str(model), "-l", "sv", "-oj", "-np", "-nt",
            "-of", str(output / f"{configuration}-{repeat}-warmup"),
            str(fixtures[0].audio_path),
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
    binary_hash = receipt["runtime_binary_sha256"]
    model_identity = {
        "adapter": "whisper.cpp CLI",
        "adapter_revision": f"sha256:{binary_hash}",
        "runtime": f"whisper.cpp 1.9.3 Metal at {WHISPER_REVISION}",
        "model": "KBLab/kb-whisper-base ggml Q5_0",
        "model_revision": KB_REVISION,
        "decoder": "whisper.cpp default beam-size 5 best-of 5",
        "decoder_revision": WHISPER_REVISION,
    }
    records = []
    outputs, actual_files, expected_files = read_whisper_outputs(
        transcript_root, fixtures
    )
    for fixture in fixtures:
        hypothesis, failure_message = outputs[fixture.fixture_id]
        records.append(base_record(
            configuration=configuration, fixture=fixture, repeat=repeat,
            app_commit=app_commit, identity=identity, model_identity=model_identity,
            hypothesis="" if hypothesis is None else hypothesis,
            success=hypothesis is not None,
            engine_ms=None,
            failure_message=failure_message,
        ))
    receipt["batch_complete"] = (
        receipt["exit_code"] == 0
        and actual_files == expected_files
        and all(record["success"] for record in records)
    )
    receipt["unexpected_output_files"] = sorted(actual_files - expected_files)
    receipt["missing_output_files"] = sorted(expected_files - actual_files)
    return records, receipt


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


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
        complete = all(item["batch_complete"] for item in configuration_receipts)
        engine_values = [
            item["exposed_engine_seconds"] for item in configuration_receipts
            if "exposed_engine_seconds" in item
        ]
        engine_seconds = sum(engine_values) if engine_values else None
        engine_ms = [record["engine_ms"] for record in records if record["engine_ms"] is not None]
        engine_p50 = percentile(engine_ms, 0.50)
        engine_p95 = percentile(engine_ms, 0.95)
        peak_rss_bytes = max(item["peak_rss_bytes"] for item in configuration_receipts)
        model_bytes = configuration_receipts[0]["model_bytes"]
        rows.append(
            f"| {configuration} | {100 * errors / reference_words:.2f}% | "
            f"{100 * character_errors / reference_characters:.2f}% | "
            f"{'—' if not complete else f'{total_audio / wall_seconds:.1f}x'} | "
            f"{'—' if not complete or engine_seconds is None else f'{total_audio / engine_seconds:.1f}x'} | "
            f"{'—' if not complete or engine_p50 is None else f'{engine_p50:.0f} ms'} | "
            f"{'—' if not complete or engine_p95 is None else f'{engine_p95:.0f} ms'} | "
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
        "| Configuration | WER | CER | Fresh-process batch RTFx | Engine RTFx | Engine p50 | Engine p95 | Peak RSS | Model | Failures |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
        *rows,
        "",
        "Fresh-process batch RTFx includes model load, transcription, and result writing after "
        "a separate cache-prime process. The cache-prime process is excluded for every model. "
        "FluidAudio exposes per-file engine time; "
        "whisper.cpp CLI does not, so its engine cells stay empty.",
        "",
        "FLEURS does not publish stable speaker IDs in this TSV. This run can compare aggregate "
        "accuracy, speed, memory, and size, but it cannot pass Koett's speaker-block confidence "
        "or public model-promotion gate.",
        "",
    ])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parakeet", required=True, type=Path)
    parser.add_argument("--parakeet-v2-model", required=True, type=Path)
    parser.add_argument("--parakeet-v3-model", required=True, type=Path)
    parser.add_argument("--whisper", required=True, type=Path)
    parser.add_argument("--whisper-source", required=True, type=Path)
    parser.add_argument("--kb-model", required=True, type=Path)
    parser.add_argument("--fleurs-tsv", required=True, type=Path)
    parser.add_argument("--fleurs-audio", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--seed", type=int, default=20260830)
    return parser.parse_args()


def verify_source_builds(arguments: argparse.Namespace, repo: Path) -> dict[str, str]:
    package = json.loads((repo / "Package.resolved").read_text(encoding="utf-8"))
    fluid_pins = [
        pin for pin in package.get("pins", []) if pin.get("identity") == "fluidaudio"
    ]
    if len(fluid_pins) != 1 or fluid_pins[0].get("state", {}).get("revision") != FLUID_REVISION:
        raise ValueError("Package.resolved does not contain the expected FluidAudio revision")
    fluid_source = repo / ".build/checkouts/FluidAudio"
    fluid_head = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=fluid_source, text=True
    ).strip()
    if fluid_head != FLUID_REVISION:
        raise ValueError("FluidAudio checkout is not at the pinned revision")
    fluid_dirty = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=fluid_source, text=True
    ).strip()
    if fluid_dirty:
        raise ValueError("FluidAudio checkout is dirty")

    expected_parakeet = (repo / ".build/release/parakeet-baseline").resolve()
    if arguments.parakeet.resolve() != expected_parakeet:
        raise ValueError("--parakeet must be this repository's release build")
    subprocess.run(
        ["swift", "build", "-c", "release", "--product", "parakeet-baseline"],
        cwd=repo,
        check=True,
    )

    whisper_source = arguments.whisper_source.resolve()
    whisper_head = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=whisper_source, text=True
    ).strip()
    if whisper_head != WHISPER_REVISION:
        raise ValueError("whisper.cpp source is not at the pinned revision")
    whisper_dirty = subprocess.check_output(
        [
            "git", "status", "--porcelain", "--", "CMakeLists.txt", "cmake",
            "examples", "ggml", "include", "src",
        ],
        cwd=whisper_source,
        text=True,
    ).strip()
    if whisper_dirty:
        raise ValueError("whisper.cpp inference source is dirty")
    expected_whisper = (whisper_source / "build/bin/whisper-cli").resolve()
    if arguments.whisper.resolve() != expected_whisper:
        raise ValueError("--whisper must be the pinned source tree's release build")
    subprocess.run(
        ["cmake", "--build", str(whisper_source / "build"), "--config", "Release", "--target", "whisper-cli"],
        check=True,
    )
    cmake_cache_path = whisper_source / "build/CMakeCache.txt"
    cmake_cache = cmake_cache_path.read_text(encoding="utf-8")
    build_type = cmake_cache_value(cmake_cache, "CMAKE_BUILD_TYPE")
    metal = cmake_cache_value(cmake_cache, "GGML_METAL")
    if build_type != "Release":
        raise ValueError(f"whisper.cpp CMAKE_BUILD_TYPE is {build_type}, expected Release")
    if metal != "ON":
        raise ValueError(f"whisper.cpp GGML_METAL is {metal}, expected ON")
    return {
        "swift": subprocess.check_output(["swift", "--version"], text=True).strip(),
        "clang": subprocess.check_output(
            ["xcrun", "clang", "--version"], text=True
        ).splitlines()[0],
        "cmake": subprocess.check_output(
            ["cmake", "--version"], text=True
        ).splitlines()[0],
        "whisper_cmake_build_type": build_type,
        "whisper_ggml_metal": metal,
        "whisper_cmake_cache_sha256": sha256_file(cmake_cache_path),
    }


def main() -> int:
    arguments = parse_args()
    if arguments.limit is not None and arguments.limit < 1:
        raise ValueError("--limit must be positive")
    if arguments.repeats < 1:
        raise ValueError("--repeats must be positive")
    for path in (arguments.parakeet, arguments.whisper, arguments.kb_model):
        if not path.is_file():
            raise ValueError(f"missing runner or model: {path}")
    for path in (arguments.parakeet_v2_model, arguments.parakeet_v3_model):
        if not path.is_dir():
            raise ValueError(f"missing Parakeet model directory: {path}")
    if sha256_file(arguments.kb_model) != KB_MODEL_SHA256:
        raise ValueError("KB-Whisper model SHA-256 does not match")
    if arguments.output.exists():
        raise ValueError("--output must be a new path")

    repo = BENCHMARKS.parent
    exact_paths = [
        "Sources/ParakeetBaseline/ParakeetBaseline.swift",
        "Benchmarks/README.md",
        "Benchmarks/benchmark_record.py",
        "Benchmarks/benchmark_schema.py",
        "Benchmarks/Swedish",
        "Package.swift",
        "Package.resolved",
    ]
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--", *exact_paths], cwd=repo, text=True
    ).strip()
    if dirty:
        raise ValueError("commit the benchmark adapter and harness before running")
    toolchain = verify_source_builds(arguments, repo)
    manifests = load_model_manifests(PARAKEET_MANIFEST)
    verify_model_directory(
        arguments.parakeet_v2_model,
        manifests["v2"],
        expected_name=PARAKEET_LOCAL_FOLDERS["v2"],
    )
    verify_model_directory(
        arguments.parakeet_v3_model,
        manifests["v3"],
        expected_name=PARAKEET_LOCAL_FOLDERS["v3"],
    )
    app_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True
    ).strip()
    fixtures = load_fleurs(arguments.fleurs_tsv, arguments.fleurs_audio, arguments.limit)
    arguments.output.mkdir(parents=True)
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
                    "v2", arguments.parakeet, arguments.parakeet_v2_model,
                    manifests["v2"], fixtures, repeat, app_commit, identity,
                    arguments.output,
                )
            elif configuration == "parakeet-v3":
                batch, receipt = run_parakeet(
                    "v3", arguments.parakeet, arguments.parakeet_v3_model,
                    manifests["v3"], fixtures, repeat, app_commit, identity,
                    arguments.output,
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
                "toolchain": toolchain,
            })
            records.extend(batch)
            receipts.append(receipt)
            checkpoint(arguments.output, records, receipts)

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

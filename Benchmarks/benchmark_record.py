#!/usr/bin/env python3
"""Validate, score, and summarize Koett benchmark run records."""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import unicodedata
from pathlib import Path
from typing import Any, Iterable

from benchmark_schema import (
    BOOLEAN_FIELDS,
    FLOAT_FIELDS,
    INTEGER_FIELDS,
    LIST_FIELDS,
    MEASUREMENT_FIELDS,
    RECORD_FIELDS,
    REQUIRED_INPUT_FIELDS,
    SCHEMA_VERSION,
    STAGES,
    TIMING_SCOPES,
    TRACE_FIELDS,
)


def normalize_words(text: str) -> list[str]:
    text = text.upper().replace("’", "'")
    normalized = []
    for character in text:
        if character == "'" or character.isalnum() or character.isspace():
            normalized.append(character)
        elif unicodedata.category(character).startswith("P"):
            normalized.append(" ")
        else:
            normalized.append(" ")
    return "".join(normalized).split()


def alignment(reference: list[str], hypothesis: list[str]) -> dict[str, int]:
    """Return one deterministic minimum-edit word alignment."""
    previous = [(index, 0, 0, index) for index in range(len(hypothesis) + 1)]
    for reference_index, reference_word in enumerate(reference, 1):
        current = [(reference_index, 0, reference_index, 0)]
        for hypothesis_index, hypothesis_word in enumerate(hypothesis, 1):
            if reference_word == hypothesis_word:
                diagonal = previous[hypothesis_index - 1]
            else:
                cost, substitutions, deletions, insertions = previous[
                    hypothesis_index - 1
                ]
                diagonal = (cost + 1, substitutions + 1, deletions, insertions)
            cost, substitutions, deletions, insertions = previous[hypothesis_index]
            deletion = (cost + 1, substitutions, deletions + 1, insertions)
            cost, substitutions, deletions, insertions = current[-1]
            insertion = (cost + 1, substitutions, deletions, insertions + 1)
            current.append(min((diagonal, deletion, insertion)))
        previous = current
    errors, substitutions, deletions, insertions = previous[-1]
    return {
        "substitutions": substitutions,
        "deletions": deletions,
        "insertions": insertions,
        "errors": errors,
    }


def load_corpus(path: Path) -> dict[str, dict[str, Any]]:
    fixtures: dict[str, dict[str, Any]] = {}
    fixture_ids_by_sha256: dict[str, str] = {}
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        required = {
            "id",
            "category",
            "speaker_id",
            "publishable",
            "expected_empty",
            "reference",
            "sha256",
            "protected_terms",
        }
        if not reader.fieldnames or not required.issubset(reader.fieldnames):
            missing = sorted(required - set(reader.fieldnames or []))
            raise ValueError(f"corpus is missing columns: {', '.join(missing)}")
        for row in reader:
            fixture_id = row["id"].strip()
            if not fixture_id or fixture_id in fixtures:
                raise ValueError(f"corpus has an empty or repeated fixture id: {fixture_id}")
            protected_terms = _parse_json_list(row["protected_terms"], "protected_terms")
            fixture = dict(row)
            fixture["id"] = fixture_id
            fixture["category"] = str(row["category"]).strip()
            fixture["speaker_id"] = str(row["speaker_id"]).strip()
            fixture["publishable"] = _parse_bool(row["publishable"], "publishable")
            fixture["expected_empty"] = _parse_bool(row["expected_empty"], "expected_empty")
            fixture["protected_terms"] = protected_terms
            if not fixture["category"]:
                raise ValueError(f"fixture {fixture_id} has an empty category")
            if not fixture["speaker_id"]:
                raise ValueError(f"fixture {fixture_id} has an empty speaker_id")
            _validate_sha256(str(fixture["sha256"]), f"fixture {fixture_id} SHA-256")
            duplicate_id = fixture_ids_by_sha256.get(str(fixture["sha256"]))
            if duplicate_id is not None:
                raise ValueError(
                    f"duplicate audio SHA-256 for fixtures {duplicate_id} and {fixture_id}"
                )
            fixture_ids_by_sha256[str(fixture["sha256"])] = fixture_id
            if fixture["expected_empty"] and normalize_words(str(fixture["reference"])):
                raise ValueError(f"empty fixture {fixture_id} has a nonempty reference")
            if not fixture["expected_empty"] and not normalize_words(
                str(fixture["reference"])
            ):
                raise ValueError(f"spoken fixture {fixture_id} has an empty reference")
            if any(not normalize_words(term) for term in protected_terms):
                raise ValueError(f"fixture {fixture_id} has an empty protected term")
            reference_words = normalize_words(str(fixture["reference"]))
            if any(
                not _contains_words(reference_words, normalize_words(term))
                for term in protected_terms
            ):
                raise ValueError(
                    f"fixture {fixture_id} has a protected term outside its reference"
                )
            fixtures[fixture_id] = fixture
    return fixtures


def score_record(record: dict[str, Any], fixture: dict[str, Any]) -> dict[str, Any]:
    _validate_input_record(record)
    if record["fixture_id"] != fixture["id"]:
        raise ValueError("run fixture_id does not match the selected corpus fixture")
    if record["fixture_sha256"] != fixture["sha256"]:
        raise ValueError(f"fixture hash mismatch for {record['fixture_id']}")

    output = {field: record.get(field) for field in RECORD_FIELDS}
    output["failure_stage"] = record.get("failure_stage") or ""
    output["timing_scope"] = record.get("timing_scope") or ""
    _derive_latencies(output)
    _validate_trace_causality(output)
    _validate_timing_scope(output)

    reference = str(fixture["reference"])
    hypothesis = str(record["hypothesis"])
    reference_tokens = normalize_words(reference)
    hypothesis_tokens = normalize_words(hypothesis)
    errors = alignment(reference_tokens, hypothesis_tokens)
    reference_characters = list(" ".join(reference_tokens))
    hypothesis_characters = list(" ".join(hypothesis_tokens))
    character_errors = alignment(reference_characters, hypothesis_characters)["errors"]
    protected_terms = list(fixture["protected_terms"])
    protected_missing = [
        term
        for term in protected_terms
        if not _contains_words(hypothesis_tokens, normalize_words(term))
    ]
    first_expected = reference_tokens[0] if reference_tokens else ""
    first_actual = hypothesis_tokens[0] if hypothesis_tokens else ""
    final_expected = reference_tokens[-1] if reference_tokens else ""
    final_actual = hypothesis_tokens[-1] if hypothesis_tokens else ""
    first_match = None if not reference_tokens else first_expected == first_actual
    final_match = None if not reference_tokens else final_expected == final_actual
    expected_empty = bool(fixture["expected_empty"])
    empty_actual = not hypothesis_tokens
    empty_match = expected_empty == empty_actual
    protected_total = len(protected_terms)
    protected_missed = len(protected_missing)

    output.update(
        {
            "category": fixture["category"],
            "speaker_id": fixture["speaker_id"],
            "publishable": fixture["publishable"],
            "reference": reference,
            "normalized_hypothesis": " ".join(hypothesis_tokens),
            "expected_empty": expected_empty,
            "protected_terms": protected_terms,
            "reference_words": len(reference_tokens),
            "hypothesis_words": len(hypothesis_tokens),
            **errors,
            "wer": None
            if not reference_tokens
            else errors["errors"] / len(reference_tokens),
            "reference_characters": len(reference_characters),
            "character_errors": character_errors,
            "cer": None
            if not reference_characters
            else character_errors / len(reference_characters),
            "protected_total": protected_total,
            "protected_missed": protected_missed,
            "protected_missing": protected_missing,
            "protected_error": None
            if protected_total == 0
            else protected_missed / protected_total,
            "first_word_expected": first_expected,
            "first_word_actual": first_actual,
            "first_word_match": first_match,
            "final_word_expected": final_expected,
            "final_word_actual": final_actual,
            "final_word_match": final_match,
            "empty_actual": empty_actual,
            "empty_match": empty_match,
            "safety_passed": bool(record["success"])
            and empty_match
            and protected_missed == 0
            and first_match is not False
            and final_match is not False,
        }
    )
    return output


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    from benchmark_summary import summarize as summarize_records

    return summarize_records(records)


def compare_configurations(
    control: list[dict[str, Any]],
    candidate: list[dict[str, Any]],
    *,
    bootstrap_samples: int = 10_000,
    bootstrap_seed: int = 0,
) -> dict[str, Any]:
    from benchmark_summary import compare_configurations as compare

    return compare(
        control,
        candidate,
        bootstrap_samples=bootstrap_samples,
        bootstrap_seed=bootstrap_seed,
    )


def read_records(path: Path) -> list[dict[str, Any]]:
    if path.suffix == ".jsonl":
        records = []
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not line.strip():
                continue
            value = json.loads(line)
            if not isinstance(value, dict):
                raise ValueError(f"JSONL line {line_number} is not an object")
            records.append(value)
        return records
    if path.suffix == ".json":
        value = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
            raise ValueError("JSON benchmark input must be an array of objects")
        return value
    if path.suffix == ".tsv":
        with path.open(encoding="utf-8", newline="") as handle:
            return [
                {key: _parse_tsv_value(key, value) for key, value in row.items()}
                for row in csv.DictReader(handle, delimiter="\t")
            ]
    raise ValueError("benchmark records must use .jsonl, .json, or .tsv")


def write_records(path: Path, records: Iterable[dict[str, Any]]) -> None:
    records = list(records)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.suffix == ".jsonl":
        text = "".join(
            json.dumps({field: record.get(field) for field in RECORD_FIELDS}, ensure_ascii=False)
            + "\n"
            for record in records
        )
        path.write_text(text, encoding="utf-8")
        return
    if path.suffix == ".json":
        values = [{field: record.get(field) for field in RECORD_FIELDS} for record in records]
        path.write_text(json.dumps(values, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        return
    if path.suffix == ".tsv":
        with path.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=RECORD_FIELDS, delimiter="\t")
            writer.writeheader()
            for record in records:
                writer.writerow(
                    {field: _format_tsv_value(record.get(field)) for field in RECORD_FIELDS}
                )
        return
    raise ValueError("benchmark records must use .jsonl, .json, or .tsv")


def _validate_input_record(record: dict[str, Any]) -> None:
    unknown = sorted(record.keys() - set(RECORD_FIELDS))
    if unknown:
        raise ValueError(f"run record has unknown fields: {', '.join(unknown)}")
    missing = sorted(REQUIRED_INPUT_FIELDS - record.keys())
    if missing:
        raise ValueError(f"run record is missing fields: {', '.join(missing)}")
    if (
        not isinstance(record["schema_version"], int)
        or isinstance(record["schema_version"], bool)
        or record["schema_version"] != SCHEMA_VERSION
    ):
        raise ValueError("run record uses an unsupported schema_version")
    if record["stage"] not in STAGES:
        raise ValueError("stage must be raw_asr or formatting")
    if not isinstance(record["source_run_id"], str):
        raise ValueError("source_run_id must be a string")
    if record["stage"] == "formatting" and not record["source_run_id"]:
        raise ValueError("a formatting run requires source_run_id")
    if record["stage"] == "raw_asr" and record["source_run_id"]:
        raise ValueError("a raw_asr run cannot have source_run_id")
    formatting_identity = (
        record["formatter"],
        record["formatter_revision"],
        record["formatting_profile"],
    )
    if record["stage"] == "raw_asr" and formatting_identity != (
        "none",
        "none",
        "none",
    ):
        raise ValueError("a raw_asr run must use the none formatting identity")
    if record["stage"] == "formatting" and "none" in formatting_identity:
        raise ValueError("a formatting run requires its full formatting identity")
    if record["normalizer_revision"] != "koett-words-v1":
        raise ValueError("normalizer_revision must be koett-words-v1")
    if not isinstance(record["success"], bool):
        raise ValueError("success must be a boolean")
    if record.get("failure_stage") is not None and not isinstance(
        record["failure_stage"], str
    ):
        raise ValueError("failure_stage must be a string")
    for field in ("failure_code", "failure_message", "hypothesis"):
        if not isinstance(record[field], str):
            raise ValueError(f"{field} must be a string")
    if not record["success"] and not record.get("failure_stage"):
        raise ValueError("a failed run requires failure_stage")
    if not record["success"] and not record["failure_code"]:
        raise ValueError("a failed run requires failure_code")
    if record["success"] and any(
        (record.get("failure_stage"), record["failure_code"], record["failure_message"])
    ):
        raise ValueError("a successful run cannot retain failure details")
    if record.get("timing_scope") not in TIMING_SCOPES:
        raise ValueError(
            "timing_scope must be cold_start, engine, full, resource, or target_visible"
        )
    for field in (
        "run_id",
        "created_at",
        "configuration",
        "app_commit",
        "adapter",
        "adapter_revision",
        "runtime",
        "model",
        "model_revision",
        "decoder",
        "decoder_revision",
        "formatter",
        "formatter_revision",
        "formatting_profile",
        "normalizer_revision",
        "platform",
        "os_version",
        "hardware",
        "power_mode",
        "target",
        "fixture_id",
        "fixture_sha256",
    ):
        if not isinstance(record[field], str) or not record[field].strip():
            raise ValueError(f"{field} must be a nonempty string")
    if not re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", record["app_commit"]):
        raise ValueError("app_commit must be a full 40- or 64-character commit hash")
    _validate_sha256(record["fixture_sha256"], "fixture_sha256")
    if (
        not isinstance(record["repeat"], int)
        or isinstance(record["repeat"], bool)
        or record["repeat"] < 1
    ):
        raise ValueError("repeat must be a positive integer")
    for field in TRACE_FIELDS:
        value = record.get(field)
        if value is not None and (
            not isinstance(value, int) or isinstance(value, bool) or value < 0
        ):
            raise ValueError(f"{field} must be a nonnegative integer or null")
    for field in set(MEASUREMENT_FIELDS) - {"timing_scope"}:
        value = record.get(field)
        if value is not None and (
            not isinstance(value, (int, float))
            or isinstance(value, bool)
            or not math.isfinite(value)
            or value < 0
        ):
            raise ValueError(f"{field} must be a nonnegative finite number or null")
    for field in ("peak_rss_bytes", "installed_footprint_bytes"):
        if record.get(field) is not None and (
            not isinstance(record[field], int) or isinstance(record[field], bool)
        ):
            raise ValueError(f"{field} must be an integer or null")


def _derive_latencies(record: dict[str, Any]) -> None:
    pairs = {
        "formatting_ms": ("formatting_start_ns", "formatting_end_ns"),
        "stop_to_final_ms": ("stop_request_ns", "model_final_result_ns"),
        "stop_to_visible_ms": ("stop_request_ns", "stable_text_ns"),
        "final_sample_to_visible_ms": ("last_audio_callback_ns", "stable_text_ns"),
    }
    for field, (start_field, end_field) in pairs.items():
        derived = _elapsed_ms(
            record[start_field],
            record[end_field],
            start_field=start_field,
            end_field=end_field,
        )
        explicit = record[field]
        if derived is not None and explicit is not None and not math.isclose(
            float(explicit), derived, rel_tol=0.0, abs_tol=0.000001
        ):
            raise ValueError(f"{field} conflicts with monotonic trace")
        if derived is not None:
            record[field] = derived


def _validate_timing_scope(record: dict[str, Any]) -> None:
    if not record["success"]:
        return
    required_by_scope = {
        "cold_start": ("cold_load_ms",),
        "engine": ("engine_ms",),
        "full": ("engine_ms", "stop_to_visible_ms", "final_sample_to_visible_ms"),
        "resource": ("peak_rss_bytes", "installed_footprint_bytes"),
        "target_visible": ("stop_to_visible_ms", "final_sample_to_visible_ms"),
    }
    missing = [
        field for field in required_by_scope[record["timing_scope"]] if record[field] is None
    ]
    if missing:
        raise ValueError(
            f"timing_scope {record['timing_scope']} requires {', '.join(missing)}"
        )


def _validate_trace_causality(record: dict[str, Any]) -> None:
    causal_edges = (
        ("shortcut_event_ns", "shortcut_callback_ns"),
        ("capture_start_request_ns", "first_audio_callback_ns"),
        ("capture_start_request_ns", "last_audio_callback_ns"),
        ("first_audio_callback_ns", "last_audio_callback_ns"),
        ("last_audio_callback_ns", "capture_drained_ns"),
        ("stop_request_ns", "capture_drained_ns"),
        ("model_ingest_start_ns", "model_final_result_ns"),
        ("capture_drained_ns", "model_final_result_ns"),
        ("model_final_result_ns", "formatting_start_ns"),
        ("formatting_start_ns", "formatting_end_ns"),
        ("model_final_result_ns", "result_notification_ns"),
        ("formatting_end_ns", "result_notification_ns"),
        ("result_notification_ns", "clipboard_write_ns"),
        ("clipboard_write_ns", "paste_post_ns"),
        ("paste_post_ns", "target_receipt_ns"),
        ("target_receipt_ns", "target_mutation_ns"),
        ("target_mutation_ns", "target_paint_ns"),
        ("target_paint_ns", "stable_text_ns"),
        ("model_final_result_ns", "stable_text_ns"),
        ("last_audio_callback_ns", "stable_text_ns"),
    )
    for start_field, end_field in causal_edges:
        start = record[start_field]
        end = record[end_field]
        if start is not None and end is not None and end < start:
            raise ValueError(f"{end_field} is before {start_field}")


def _elapsed_ms(
    start_ns: Any,
    end_ns: Any,
    *,
    start_field: str,
    end_field: str,
) -> float | None:
    if start_ns is None or end_ns is None:
        return None
    if end_ns < start_ns:
        raise ValueError(f"{end_field} is before {start_field}")
    return (end_ns - start_ns) / 1_000_000


def _contains_words(haystack: list[str], needle: list[str]) -> bool:
    if not needle:
        return False
    width = len(needle)
    return any(
        haystack[index : index + width] == needle
        for index in range(len(haystack) - width + 1)
    )


def _validate_sha256(value: str, field: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ValueError(
            f"{field} must be a lowercase 64-character hexadecimal SHA-256"
        )


def _parse_bool(value: Any, field: str) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, str) and value.lower() in {"true", "false"}:
        return value.lower() == "true"
    raise ValueError(f"{field} must be true or false")


def _parse_json_list(value: Any, field: str) -> list[str]:
    if isinstance(value, list):
        parsed = value
    else:
        try:
            parsed = json.loads(value or "[]")
        except json.JSONDecodeError as error:
            raise ValueError(f"{field} must be a JSON string array") from error
    if not isinstance(parsed, list) or not all(isinstance(item, str) and item for item in parsed):
        raise ValueError(f"{field} must be a JSON string array")
    return parsed


def _parse_tsv_value(field: str, value: str | None) -> Any:
    value = "" if value is None else value
    if field in LIST_FIELDS:
        return _parse_json_list(value, field)
    if field in BOOLEAN_FIELDS:
        return None if value == "" else _parse_bool(value, field)
    if field in INTEGER_FIELDS:
        return None if value == "" else int(value)
    if field in FLOAT_FIELDS:
        return None if value == "" else float(value)
    return value


def _format_tsv_value(value: Any) -> Any:
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, list):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Score Koett JSONL, JSON, or TSV benchmark run records."
    )
    parser.add_argument("runs", type=Path)
    parser.add_argument("--corpus", type=Path, default=Path(__file__).with_name("corpus.tsv"))
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--control")
    parser.add_argument("--candidate")
    parser.add_argument("--bootstrap-samples", type=int, default=10_000)
    parser.add_argument("--bootstrap-seed", type=int, default=0)
    return parser.parse_args()


def main() -> int:
    arguments = parse_args()
    corpus = load_corpus(arguments.corpus)
    records = read_records(arguments.runs)
    scored = []
    for record in records:
        fixture_id = str(record.get("fixture_id", ""))
        if fixture_id not in corpus:
            raise ValueError(f"run uses unknown fixture_id: {fixture_id}")
        scored.append(score_record(record, corpus[fixture_id]))
    write_records(arguments.output, scored)
    arguments.summary.parent.mkdir(parents=True, exist_ok=True)
    summary = summarize(scored)
    if bool(arguments.control) != bool(arguments.candidate):
        raise ValueError("--control and --candidate must be used together")
    if arguments.control:
        control = [
            record
            for record in scored
            if record["stage"] == "raw_asr"
            and record["configuration"] == arguments.control
        ]
        candidate = [
            record
            for record in scored
            if record["stage"] == "raw_asr"
            and record["configuration"] == arguments.candidate
        ]
        summary["comparison"] = compare_configurations(
            control,
            candidate,
            bootstrap_samples=arguments.bootstrap_samples,
            bootstrap_seed=arguments.bootstrap_seed,
        )
    arguments.summary.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        raise SystemExit(1)

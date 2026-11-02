"""Summaries and paired comparisons for scored Koett benchmark records."""

from __future__ import annotations

import math
import random
from collections import Counter
from typing import Any

from benchmark_schema import (
    COMPARISON_CONTEXT_FIELDS,
    CONFIGURATION_IDENTITY_FIELDS,
    LOCKED_SAFETY_CATEGORIES,
    SCHEMA_VERSION,
)


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    run_ids = [str(record["run_id"]) for record in records]
    if len(set(run_ids)) != len(run_ids):
        raise ValueError("benchmark records contain repeated run_id values")
    by_id = {str(record["run_id"]): record for record in records}
    for record in records:
        if record["stage"] != "formatting":
            continue
        source = by_id.get(str(record["source_run_id"]))
        if (
            source is None
            or source["stage"] != "raw_asr"
            or source["fixture_id"] != record["fixture_id"]
            or source["repeat"] != record["repeat"]
            or _asr_context(source) != _asr_context(record)
        ):
            raise ValueError(
                f"formatting run {record['run_id']} must reference its matching raw_asr run"
            )
    stages: dict[str, Any] = {}
    for stage in sorted({str(record["stage"]) for record in records}):
        stage_records = [record for record in records if record["stage"] == stage]
        configurations: dict[str, Any] = {}
        for configuration in sorted(
            {str(record["configuration"]) for record in stage_records}
        ):
            configuration_records = [
                record
                for record in stage_records
                if record["configuration"] == configuration
            ]
            configurations[configuration] = {
                "identity": _single_configuration_identity(configuration_records),
                "metrics": _summarize_configuration(configuration_records),
            }
        stages[stage] = {"configurations": configurations}
    return {"schema_version": SCHEMA_VERSION, "records": len(records), "stages": stages}


def compare_configurations(
    control: list[dict[str, Any]],
    candidate: list[dict[str, Any]],
    *,
    bootstrap_samples: int = 10_000,
    bootstrap_seed: int = 0,
) -> dict[str, Any]:
    """Compare two scored configurations with paired fixture-block bootstrap."""
    if bootstrap_samples < 1:
        raise ValueError("bootstrap_samples must be positive")
    control_identity = _single_configuration_identity(control)
    candidate_identity = _single_configuration_identity(candidate)
    for field in COMPARISON_CONTEXT_FIELDS:
        if control_identity[field] != candidate_identity[field]:
            raise ValueError(f"comparison context differs for {field}")
    if control_identity["stage"] != candidate_identity["stage"]:
        raise ValueError("comparison stages differ")

    control_by_pair = _records_by_pair(control)
    candidate_by_pair = _records_by_pair(candidate)
    if control_by_pair.keys() != candidate_by_pair.keys():
        raise ValueError("configurations must have identical paired fixture/repeat runs")
    for pair in control_by_pair:
        if (
            control_by_pair[pair]["fixture_sha256"]
            != candidate_by_pair[pair]["fixture_sha256"]
        ):
            raise ValueError(f"paired fixture hash differs for {pair[0]} repeat {pair[1]}")
        if control_by_pair[pair]["speaker_id"] != candidate_by_pair[pair]["speaker_id"]:
            raise ValueError(f"paired speaker differs for {pair[0]} repeat {pair[1]}")
        if control_by_pair[pair]["category"] != candidate_by_pair[pair]["category"]:
            raise ValueError(f"paired category differs for {pair[0]} repeat {pair[1]}")

    control_metrics = _summarize_configuration(control)
    candidate_metrics = _summarize_configuration(candidate)
    bootstrap = _paired_wer_bootstrap(
        control_by_pair,
        candidate_by_pair,
        samples=bootstrap_samples,
        seed=bootstrap_seed,
    )
    new_safety_failures = _new_locked_safety_failures(
        control_by_pair, candidate_by_pair
    )
    coverage_complete = bool(control_metrics["locked_safety_complete"]) and bool(
        candidate_metrics["locked_safety_complete"]
    )
    public_sample_complete = bool(
        control_metrics["minimum_public_sample_complete"]
    ) and bool(candidate_metrics["minimum_public_sample_complete"])
    protected_not_worse = _not_increased(
        control_metrics["protected_error"], candidate_metrics["protected_error"]
    )
    latency_improvement = _p95_improvement(
        control_metrics["stop_to_visible_ms"],
        candidate_metrics["stop_to_visible_ms"],
    )
    rss_reduction = _median_reduction(
        control_metrics["peak_rss_bytes"], candidate_metrics["peak_rss_bytes"]
    )
    footprint_reduction = _median_reduction(
        control_metrics["installed_footprint_bytes"],
        candidate_metrics["installed_footprint_bytes"],
    )
    protected_reduction = _relative_reduction(
        control_metrics["protected_error"], candidate_metrics["protected_error"]
    )
    benefit_passed = (
        (latency_improvement is not None and latency_improvement >= 50.0)
        or (rss_reduction is not None and rss_reduction >= 0.20)
        or (footprint_reduction is not None and footprint_reduction >= 0.20)
        or (protected_reduction is not None and protected_reduction >= 0.25)
    )
    safety_passed = coverage_complete and not new_safety_failures
    wer_passed = bootstrap["ci95_high_points"] <= 0.25
    return {
        "control_configuration": control_identity["configuration"],
        "candidate_configuration": candidate_identity["configuration"],
        "paired_runs": len(control_by_pair),
        "paired_fixtures": len({pair[0] for pair in control_by_pair}),
        "bootstrap_unit": "speaker_id",
        "bootstrap_samples": bootstrap_samples,
        "bootstrap_seed": bootstrap_seed,
        **bootstrap,
        "protected_error_not_increased": protected_not_worse,
        "new_locked_safety_failures": new_safety_failures,
        "locked_safety_complete": coverage_complete,
        "minimum_public_sample_complete": public_sample_complete,
        "stop_to_visible_p95_improvement_ms": latency_improvement,
        "peak_rss_median_reduction": rss_reduction,
        "installed_footprint_median_reduction": footprint_reduction,
        "protected_error_reduction": protected_reduction,
        "wer_noninferiority_passed": wer_passed,
        "locked_safety_passed": safety_passed,
        "defined_benefit_passed": benefit_passed,
    }


def _summarize_configuration(records: list[dict[str, Any]]) -> dict[str, Any]:
    reference_words = sum(int(record["reference_words"]) for record in records)
    errors = sum(
        int(record["errors"])
        for record in records
        if int(record["reference_words"]) > 0
    )
    reference_characters = sum(int(record["reference_characters"]) for record in records)
    character_errors = sum(
        int(record["character_errors"])
        for record in records
        if int(record["reference_characters"]) > 0
    )
    protected_total = sum(int(record["protected_total"]) for record in records)
    protected_missed = sum(int(record["protected_missed"]) for record in records)
    categories = sorted({str(record["category"]) for record in records})
    missing_safety = sorted(LOCKED_SAFETY_CATEGORIES - set(categories))
    publishable_recordings = {
        str(record["fixture_sha256"]) for record in records if record["publishable"]
    }
    publishable_speakers = {
        str(record["speaker_id"]) for record in records if record["publishable"]
    }
    first_values = [
        record["first_word_match"]
        for record in records
        if record["first_word_match"] is not None
    ]
    final_values = [
        record["final_word_match"]
        for record in records
        if record["final_word_match"] is not None
    ]
    failures = Counter(
        str(record["failure_code"]) for record in records if not record["success"]
    )
    summary: dict[str, Any] = {
        "runs": len(records),
        "successful_runs": sum(bool(record["success"]) for record in records),
        "safety_passes": sum(bool(record["safety_passed"]) for record in records),
        "failures": dict(sorted(failures.items())),
        "reference_words": reference_words,
        "errors": errors,
        "wer": None if reference_words == 0 else errors / reference_words,
        "reference_characters": reference_characters,
        "character_errors": character_errors,
        "cer": None
        if reference_characters == 0
        else character_errors / reference_characters,
        "protected_total": protected_total,
        "protected_missed": protected_missed,
        "protected_error": None
        if protected_total == 0
        else protected_missed / protected_total,
        "first_word_retention": _retention(first_values),
        "final_word_retention": _retention(final_values),
        "empty_matches": sum(bool(record["empty_match"]) for record in records),
        "non_speech_false_pastes": sum(
            bool(record["expected_empty"]) and not bool(record["empty_actual"])
            for record in records
        ),
        "categories": {
            category: _summarize_category(
                [record for record in records if record["category"] == category]
            )
            for category in categories
        },
        "missing_locked_safety_categories": missing_safety,
        "locked_safety_complete": not missing_safety,
        "unique_recordings": len({str(record["fixture_sha256"]) for record in records}),
        "publishable_recordings": len(publishable_recordings),
        "publishable_speakers": len(publishable_speakers),
        "minimum_public_sample_complete": len(publishable_recordings) >= 100
        and len(publishable_speakers) >= 10,
    }
    for field in (
        "engine_ms",
        "stop_to_final_ms",
        "stop_to_visible_ms",
        "final_sample_to_visible_ms",
    ):
        summary[field] = _latency_summary(records, field)
    for field in ("peak_rss_bytes", "installed_footprint_bytes", "cpu_seconds"):
        summary[field] = _resource_summary(records, field)
    return summary


def _single_configuration_identity(records: list[dict[str, Any]]) -> dict[str, Any]:
    if not records:
        raise ValueError("a configuration must contain at least one record")
    identity = {field: records[0][field] for field in CONFIGURATION_IDENTITY_FIELDS}
    if any(
        any(record[field] != identity[field] for field in CONFIGURATION_IDENTITY_FIELDS)
        for record in records[1:]
    ):
        raise ValueError(
            f"configuration {records[0]['configuration']} maps to more than one identity"
        )
    return identity


def _asr_context(record: dict[str, Any]) -> tuple[Any, ...]:
    fields = (
        "app_commit",
        "adapter",
        "adapter_revision",
        "runtime",
        "model",
        "model_revision",
        "decoder",
        "decoder_revision",
        "normalizer_revision",
        "platform",
        "os_version",
        "hardware",
        "power_mode",
        "target",
        "timing_scope",
    )
    return tuple(record[field] for field in fields)


def _records_by_pair(
    records: list[dict[str, Any]],
) -> dict[tuple[str, int], dict[str, Any]]:
    pairs: dict[tuple[str, int], dict[str, Any]] = {}
    for record in records:
        pair = (str(record["fixture_id"]), int(record["repeat"]))
        if pair in pairs:
            raise ValueError(f"repeated paired fixture/repeat run: {pair}")
        pairs[pair] = record
    return pairs


def _new_locked_safety_failures(
    control: dict[tuple[str, int], dict[str, Any]],
    candidate: dict[tuple[str, int], dict[str, Any]],
) -> list[dict[str, Any]]:
    failures = []
    for pair in sorted(control):
        baseline = control[pair]
        result = candidate[pair]
        reasons = []
        category = str(result["category"])
        if (
            category in LOCKED_SAFETY_CATEGORIES
            and baseline["success"]
            and not result["success"]
        ):
            reasons.append("run_failure")
        if (
            category in LOCKED_SAFETY_CATEGORIES
            and int(result["errors"]) > 0
            and (
                int(baseline["errors"]) == 0
                or result["normalized_hypothesis"]
                != baseline["normalized_hypothesis"]
            )
        ):
            reasons.append(category)
        if baseline["first_word_match"] is True and result["first_word_match"] is False:
            reasons.append("first_word")
        if baseline["final_word_match"] is True and result["final_word_match"] is False:
            reasons.append("final_word")
        if reasons:
            failures.append(
                {"fixture_id": pair[0], "repeat": pair[1], "reasons": reasons}
            )
    return failures


def _paired_wer_bootstrap(
    control: dict[tuple[str, int], dict[str, Any]],
    candidate: dict[tuple[str, int], dict[str, Any]],
    *,
    samples: int,
    seed: int,
) -> dict[str, Any]:
    speaker_ids = sorted(
        {
            str(record["speaker_id"])
            for record in control.values()
            if int(record["reference_words"]) > 0
        }
    )
    if not speaker_ids:
        raise ValueError("paired WER needs at least one speaker with spoken fixtures")
    pairs_by_speaker = {
        speaker_id: sorted(
            pair
            for pair, record in control.items()
            if record["speaker_id"] == speaker_id and int(record["reference_words"]) > 0
        )
        for speaker_id in speaker_ids
    }

    def difference(selected_speakers: list[str]) -> float:
        control_errors = control_words = candidate_errors = candidate_words = 0
        for speaker_id in selected_speakers:
            for pair in pairs_by_speaker[speaker_id]:
                control_errors += int(control[pair]["errors"])
                control_words += int(control[pair]["reference_words"])
                candidate_errors += int(candidate[pair]["errors"])
                candidate_words += int(candidate[pair]["reference_words"])
        return 100.0 * (
            candidate_errors / candidate_words - control_errors / control_words
        )

    observed = difference(speaker_ids)
    generator = random.Random(seed)
    differences = [
        difference(
            [speaker_ids[generator.randrange(len(speaker_ids))] for _ in speaker_ids]
        )
        for _ in range(samples)
    ]
    return {
        "wer_difference_points": observed,
        "ci95_low_points": _percentile(differences, 0.025),
        "ci95_high_points": _percentile(differences, 0.975),
    }


def _summarize_category(records: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "runs": len(records),
        "failures": sum(not bool(record["success"]) for record in records),
        "safety_failures": sum(not bool(record["safety_passed"]) for record in records),
    }


def _retention(values: list[bool]) -> dict[str, Any]:
    matched = sum(values)
    return {
        "eligible": len(values),
        "matched": matched,
        "rate": None if not values else matched / len(values),
    }


def _latency_summary(records: list[dict[str, Any]], field: str) -> dict[str, Any]:
    values = [float(record[field]) for record in records if record[field] is not None]
    return {
        "count": len(values),
        "missing": len(records) - len(values),
        "p50": _percentile(values, 0.50),
        "p95": _percentile(values, 0.95),
        "p99": _percentile(values, 0.99),
    }


def _resource_summary(records: list[dict[str, Any]], field: str) -> dict[str, Any]:
    values = [float(record[field]) for record in records if record[field] is not None]
    return {
        "count": len(values),
        "missing": len(records) - len(values),
        "minimum": None if not values else min(values),
        "median": _percentile(values, 0.50),
        "maximum": None if not values else max(values),
    }


def _not_increased(control: Any, candidate: Any) -> bool:
    return control is not None and candidate is not None and candidate <= control


def _p95_improvement(control: dict[str, Any], candidate: dict[str, Any]) -> float | None:
    if control["missing"] or candidate["missing"]:
        return None
    if control["p95"] is None or candidate["p95"] is None:
        return None
    return float(control["p95"]) - float(candidate["p95"])


def _median_reduction(control: dict[str, Any], candidate: dict[str, Any]) -> float | None:
    if control["missing"] or candidate["missing"]:
        return None
    baseline = control["median"]
    result = candidate["median"]
    if baseline is None or result is None or baseline <= 0:
        return None
    return (float(baseline) - float(result)) / float(baseline)


def _relative_reduction(control: Any, candidate: Any) -> float | None:
    if control is None or candidate is None or control <= 0:
        return None
    return (float(control) - float(candidate)) / float(control)


def _percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)

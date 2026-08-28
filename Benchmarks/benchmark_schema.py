"""Versioned fields for Koett benchmark records."""

SCHEMA_VERSION = 1
STAGES = {"raw_asr", "formatting"}
TIMING_SCOPES = {"cold_start", "engine", "full", "resource", "target_visible"}
LOCKED_SAFETY_CATEGORIES = {"code", "negation", "number", "path"}

IDENTITY_FIELDS = [
    "schema_version",
    "run_id",
    "created_at",
    "stage",
    "source_run_id",
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
    "repeat",
]

RESULT_FIELDS = [
    "success",
    "failure_stage",
    "failure_code",
    "failure_message",
    "hypothesis",
]

TRACE_FIELDS = [
    "shortcut_event_ns",
    "shortcut_callback_ns",
    "capture_start_request_ns",
    "first_audio_callback_ns",
    "stop_request_ns",
    "last_audio_callback_ns",
    "capture_drained_ns",
    "model_ingest_start_ns",
    "model_final_result_ns",
    "formatting_start_ns",
    "formatting_end_ns",
    "result_notification_ns",
    "clipboard_write_ns",
    "paste_post_ns",
    "target_receipt_ns",
    "target_mutation_ns",
    "target_paint_ns",
    "stable_text_ns",
]

MEASUREMENT_FIELDS = [
    "audio_seconds",
    "download_ms",
    "cold_load_ms",
    "prewarm_ms",
    "first_prediction_ms",
    "engine_ms",
    "formatting_ms",
    "stop_to_final_ms",
    "stop_to_visible_ms",
    "final_sample_to_visible_ms",
    "peak_rss_bytes",
    "energy_joules",
    "installed_footprint_bytes",
    "cpu_seconds",
    "timing_scope",
]

SCORE_FIELDS = [
    "category",
    "speaker_id",
    "publishable",
    "reference",
    "normalized_hypothesis",
    "expected_empty",
    "protected_terms",
    "reference_words",
    "hypothesis_words",
    "substitutions",
    "deletions",
    "insertions",
    "errors",
    "wer",
    "reference_characters",
    "character_errors",
    "cer",
    "protected_total",
    "protected_missed",
    "protected_missing",
    "protected_error",
    "first_word_expected",
    "first_word_actual",
    "first_word_match",
    "final_word_expected",
    "final_word_actual",
    "final_word_match",
    "empty_actual",
    "empty_match",
    "safety_passed",
]

RECORD_FIELDS = (
    IDENTITY_FIELDS
    + RESULT_FIELDS
    + TRACE_FIELDS
    + MEASUREMENT_FIELDS
    + SCORE_FIELDS
)

REQUIRED_INPUT_FIELDS = set(IDENTITY_FIELDS + RESULT_FIELDS) - {"failure_stage"}

CONFIGURATION_IDENTITY_FIELDS = [
    "stage",
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
    "timing_scope",
]

COMPARISON_CONTEXT_FIELDS = [
    "app_commit",
    "normalizer_revision",
    "platform",
    "os_version",
    "hardware",
    "power_mode",
    "target",
    "timing_scope",
]

INTEGER_FIELDS = {
    "schema_version",
    "repeat",
    "peak_rss_bytes",
    "installed_footprint_bytes",
    "reference_words",
    "hypothesis_words",
    "substitutions",
    "deletions",
    "insertions",
    "errors",
    "reference_characters",
    "character_errors",
    "protected_total",
    "protected_missed",
    *TRACE_FIELDS,
}

FLOAT_FIELDS = {
    "audio_seconds",
    "download_ms",
    "cold_load_ms",
    "prewarm_ms",
    "first_prediction_ms",
    "engine_ms",
    "formatting_ms",
    "stop_to_final_ms",
    "stop_to_visible_ms",
    "final_sample_to_visible_ms",
    "energy_joules",
    "cpu_seconds",
    "wer",
    "cer",
    "protected_error",
}

BOOLEAN_FIELDS = {
    "success",
    "publishable",
    "expected_empty",
    "first_word_match",
    "final_word_match",
    "empty_actual",
    "empty_match",
    "safety_passed",
}

LIST_FIELDS = {"protected_terms", "protected_missing"}

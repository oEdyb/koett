#include "whisper.h"
#include <limits.h>
#include <stdlib.h>

typedef void (*emit_token)(void *, const char *, int, int);
typedef void (*emit_span)(void *, int, int);

static void discard_log(enum ggml_log_level level, const char *text, void *context) {
    (void)level;
    (void)text;
    (void)context;
}

void *koett_whisper_open(const char *path) {
    whisper_log_set(discard_log, NULL);
    struct whisper_context_params params = whisper_context_default_params();
    params.use_gpu = true;
    return whisper_init_from_file_with_params(path, params);
}

void koett_whisper_close(void *model) { whisper_free(model); }

int koett_whisper_language(void *model, const float *pcm, int count, float *scores) {
    if (whisper_pcm_to_mel(model, pcm, count, 2) != 0) return -1;
    float probs[256] = {0};
    if (whisper_lang_max_id() >= 256 || whisper_lang_auto_detect(model, 0, 2, probs) < 0) return -1;
    scores[0] = probs[whisper_lang_id("en")];
    scores[1] = probs[whisper_lang_id("sv")];
    return 0;
}

int koett_whisper_decode(void *model, const float *pcm, int count, const char *lang,
                        void *context, emit_token emit) {
    struct whisper_full_params params = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
    params.n_threads = 2;
    params.language = lang;
    params.no_context = true;
    params.print_realtime = false;
    params.print_progress = false;
    params.print_special = false;
    params.print_timestamps = false;
    params.token_timestamps = true;
    params.temperature_inc = 0;
    int status = whisper_full(model, params, pcm, count);
    if (status != 0) return status;
    for (int s = 0; s < whisper_full_n_segments(model); ++s) {
        for (int t = 0; t < whisper_full_n_tokens(model, s); ++t) {
            if (whisper_full_get_token_id(model, s, t) >= whisper_token_eot(model)) continue;
            int64_t start = whisper_full_get_token_t0(model, s, t);
            int64_t end = whisper_full_get_token_t1(model, s, t);
            if (start < 0 || end < start) {
                start = whisper_full_get_segment_t0(model, s);
                end = whisper_full_get_segment_t1(model, s);
            }
            emit(context, whisper_full_get_token_text(model, s, t), (int)(start * 160), (int)(end * 160));
        }
    }
    return 0;
}

void *koett_vad_open(const char *path) {
    whisper_log_set(discard_log, NULL);
    struct whisper_vad_context_params params = whisper_vad_default_context_params();
    params.use_gpu = false;
    params.n_threads = 2;
    return whisper_vad_init_from_file_with_params(path, params);
}

void koett_vad_close(void *model) { whisper_vad_free(model); }

int koett_vad_find(void *model, const float *pcm, int count, void *context, emit_span emit) {
    struct whisper_vad_params params = whisper_vad_default_params();
    params.min_silence_duration_ms = 500;
    params.speech_pad_ms = 0;
    params.max_speech_duration_s = 20;
    params.samples_overlap = 0;
    struct whisper_vad_segments * spans = whisper_vad_segments_from_samples(model, params, pcm, count);
    if (!spans) return -1;
    for (int i = 0; i < whisper_vad_segments_n_segments(spans); ++i) {
        emit(context, (int)(whisper_vad_segments_get_segment_t0(spans, i) * 160),
                      (int)(whisper_vad_segments_get_segment_t1(spans, i) * 160));
    }
    whisper_vad_free_segments(spans);
    return 0;
}

import json
import os
from pathlib import Path
import site
import subprocess
import sys
import traceback

TURBO = "whisper-large-v3-turbo"
SWEDISH = "kb-whisper-large"
SOURCES = {TURBO: "large-v3-turbo", SWEDISH: "KBLab/kb-whisper-large"}
SAMPLE_RATE = 16_000
SHORT_SPEECH = 3 * SAMPLE_RATE
OWN_LABEL_CONFIDENCE = 0.99
LANGUAGE_CONTEXT = 10 * SAMPLE_RATE


def configure_cuda():
    libraries = [
        str(path)
        for packages in site.getsitepackages()
        for library in ("cublas", "cudnn", "cuda_nvrtc")
        if (path := Path(packages) / "nvidia" / library / "lib").is_dir()
    ]
    current = os.environ.get("LD_LIBRARY_PATH", "").split(":")
    if libraries and current[:len(libraries)] != libraries:
        os.environ["LD_LIBRARY_PATH"] = ":".join(dict.fromkeys(libraries + [p for p in current if p]))
        os.execv(sys.executable, [sys.executable, *sys.argv])


def vram_mb():
    try:
        rows = subprocess.check_output(
            ["nvidia-smi", "--query-compute-apps=pid,used_gpu_memory",
             "--format=csv,noheader,nounits"],
            text=True, timeout=5,
        ).splitlines()
        allocations = [int(mib) for pid, mib in (row.split(",") for row in rows)
                       if int(pid) == os.getpid()]
        return sum(allocations) if allocations else None
    except (OSError, ValueError, subprocess.SubprocessError):
        return None


def send(reply):
    print(json.dumps(reply, allow_nan=False), flush=True)


def read_audio(window):
    import numpy as np

    try:
        return np.fromfile(window["pcm"], dtype="<i2").astype(np.float32) / 32768.0
    except OSError as error:
        return error


def language_audio(index, tracks, waveforms, speech_samples=None):
    import numpy as np

    audio = waveforms[index]
    if (len(audio) if speech_samples is None else speech_samples) >= SHORT_SPEECH:
        return audio
    nearby = sorted(
        (i for i, track in enumerate(tracks) if i != index and track == tracks[index]),
        key=lambda i: abs(i - index),
    )
    parts = {index: audio}
    remaining = LANGUAGE_CONTEXT - len(audio)
    for i in nearby:
        if remaining == 0:
            break
        neighbour = waveforms[i]
        if isinstance(neighbour, OSError):
            continue
        part = neighbour[-remaining:] if i < index else neighbour[:remaining]
        parts[i] = part
        remaining -= len(part)
    return np.concatenate([parts[i] for i in sorted(parts)]) if len(parts) > 1 else audio


def detect_language(engine, audio):
    _, _, probabilities = engine.detect_language(audio)
    return max(
        (candidate for candidate in probabilities if candidate[0] in ("sv", "en")),
        key=lambda candidate: candidate[1],
    )


def decode(window, audio, context, engines):
    lang, probability = detect_language(engines[TURBO], audio)
    if context is not audio and probability < OWN_LABEL_CONFIDENCE:
        lang, probability = detect_language(engines[TURBO], context)
    backend = SWEDISH if lang == "sv" else TURBO
    segments, _ = engines[backend].transcribe(
        audio, language=lang, word_timestamps=True, beam_size=5,
        condition_on_previous_text=False, vad_filter=False,
    )
    return {
        "id": window["id"], "lang": lang, "lang_p": probability, "backend": backend,
        "words": [[word.word.strip(), word.start, word.end]
                  for segment in segments for word in segment.words],
    }


def main():
    configure_cuda()
    fetch = sys.argv[1:] == ["--fetch"]
    check = fetch or sys.argv[1:] == ["--check"]
    if fetch:
        os.environ.pop("HF_HUB_OFFLINE", None)
    else:
        os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"

    from faster_whisper import WhisperModel
    from faster_whisper.utils import download_model

    snapshots = {}
    engines = {}
    for backend, source in SOURCES.items():
        snapshot = download_model(source, local_files_only=not fetch)
        if not (Path(snapshot) / "tokenizer.json").is_file():
            raise FileNotFoundError(f"Missing cached tokenizer for {source}")
        snapshots[backend] = snapshot
        engines[backend] = WhisperModel(
            snapshot, device="cuda", compute_type="float16", local_files_only=True,
        )
    ready = {"ready": True, "device": "cuda", "models": snapshots}
    if check:
        ready["vram_mb"] = vram_mb()
    send(ready)
    if check:
        return
    for line in sys.stdin:
        batch = json.loads(line)
        if batch["op"] != "decode":
            raise ValueError("Expected a decode batch")
        windows = batch["windows"]
        tracks = [window["id"].rsplit("-", 1)[0] for window in windows]
        waveforms = [read_audio(window) for window in windows]
        for index, (window, audio) in enumerate(zip(windows, waveforms)):
            try:
                if isinstance(audio, OSError):
                    raise audio
                context = language_audio(index, tracks, waveforms, window.get("speech_samples"))
                send(decode(window, audio, context, engines))
            except Exception as error:
                traceback.print_exc()
                send({"id": window["id"], "error": str(error)})
                return


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        traceback.print_exc()
        send({"error": str(error)})
        sys.exit(1)

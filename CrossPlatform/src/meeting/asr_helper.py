import json
import os
from pathlib import Path
import site
import subprocess
import sys

TURBO = "whisper-large-v3-turbo"
SWEDISH = "kb-whisper-large"
SOURCES = {TURBO: "large-v3-turbo", SWEDISH: "KBLab/kb-whisper-large"}


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


def decode(window, engines):
    import numpy as np

    audio = np.fromfile(window["pcm"], dtype="<i2").astype(np.float32) / 32768.0
    lang, probability, _ = engines[TURBO].detect_language(audio)
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
    send({"ready": True, "device": "cuda", "models": snapshots, "vram_mb": vram_mb()})
    for line in sys.stdin:
        batch = json.loads(line)
        if batch["op"] != "decode":
            raise ValueError("Expected a decode batch")
        for window in batch["windows"]:
            try:
                send(decode(window, engines))
            except Exception as error:
                send({"id": window["id"], "error": str(error)})
                return


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"{type(error).__name__}: {error}", file=sys.stderr, flush=True)
        send({"error": str(error)})
        sys.exit(1)

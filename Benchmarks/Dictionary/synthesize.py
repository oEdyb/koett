"""Synthesize the dictionary benchmark corpus with edge-tts into 16 kHz mono WAVs.

Usage: uvx --with edge-tts python synthesize.py OUT_DIR [VOICE]
Only the public test sentences in corpus.json are sent to the TTS service.
"""
import asyncio, json, os, subprocess, sys
from pathlib import Path
import edge_tts

async def main(out: Path, only: str | None) -> None:
    corpus = json.loads((Path(__file__).parent / "corpus.json").read_text())
    out.mkdir(parents=True, exist_ok=True)
    clips = corpus["positive"] + corpus["negative"]
    for voice in [v for v in corpus["voices"] if only in (None, v)]:
        for clip in clips:
            wav = out / f"{clip['id']}-{voice}.wav"
            if wav.exists():
                continue
            mp3 = wav.with_name(f"{wav.stem}.{os.getpid()}.mp3")
            part = wav.with_name(f"{wav.stem}.{os.getpid()}.part.wav")
            for attempt in range(5):
                try:
                    await edge_tts.Communicate(clip["text"], voice).save(str(mp3))
                    break
                except edge_tts.exceptions.NoAudioReceived:
                    await asyncio.sleep(5 * (attempt + 1))
            else:
                raise SystemExit(f"no audio for {wav.name}")
            subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(mp3),
                            "-ac", "1", "-ar", "16000", "-sample_fmt", "s16", str(part)], check=True)
            part.rename(wav)
            mp3.unlink()
            await asyncio.sleep(0.7)  # the free service drops bursts

asyncio.run(main(Path(sys.argv[1]), sys.argv[2] if len(sys.argv) > 2 else None))

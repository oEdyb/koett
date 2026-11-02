"""Synthesize the dictionary benchmark corpus with edge-tts into 16 kHz mono WAVs.

Usage: uvx --with edge-tts python synthesize.py OUT_DIR
Only the public test sentences in corpus.json are sent to the TTS service.
"""
import asyncio, json, subprocess, sys
from pathlib import Path
import edge_tts

async def main(out: Path) -> None:
    corpus = json.loads((Path(__file__).parent / "corpus.json").read_text())
    out.mkdir(parents=True, exist_ok=True)
    clips = corpus["positive"] + corpus["negative"]
    for voice in corpus["voices"]:
        for clip in clips:
            wav = out / f"{clip['id']}-{voice}.wav"
            if wav.exists():
                continue
            mp3 = wav.with_suffix(".mp3")
            for attempt in range(5):
                try:
                    await edge_tts.Communicate(clip["text"], voice).save(str(mp3))
                    break
                except edge_tts.exceptions.NoAudioReceived:
                    await asyncio.sleep(1 + attempt)
            else:
                raise SystemExit(f"no audio for {wav.name}")
            subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(mp3),
                            "-ac", "1", "-ar", "16000", "-sample_fmt", "s16", str(wav)], check=True)
            mp3.unlink()

asyncio.run(main(Path(sys.argv[1])))

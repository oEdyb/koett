# Dictionary benchmark

Shared corpus for comparing ways to make Koett write Olle's own words correctly.

- `corpus.json`: 30 dictionary terms, 40 positive sentences that contain them, and 30
  negative sentences with near-sounding ordinary words and no dictionary term.
- `synthesize.py`: renders every sentence with three edge-tts voices to 16 kHz mono WAV.
  Run `uvx --with edge-tts python synthesize.py ~/.cache/koett/dictionary-bench/clips`.
  Only these public test sentences go to the TTS service. Audio stays out of git.
- `transcribe.sh`: runs the release `koett-engine` CLI over every clip and writes
  `{clip: text}` JSON.
- `score.py`: term recall, false dictionary terms on negatives, WER, and broken negatives
  against a baseline. Add `-v` to list the misses.

Synthetic voices are cleaner than a real microphone. Treat the numbers as relative.

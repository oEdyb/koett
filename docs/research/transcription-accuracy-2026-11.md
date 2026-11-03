# Transcription accuracy research, November 2026

Question: what is the highest-accuracy way to improve Koett on Mac and Linux,
locally, with low delay? The mishearings behind this: "Koett" as "Poets", "coet"
and "Taldi coet", and "Scrawl" as "scroll". The speaker is Swedish and dictates
long, fast English with fillers, names and technical terms.

## Answer

Don't change the model first. Olle's Linux dictionary is **empty**, so the
hotwords and spelling pass Koett already ships never run for him. Fill it with
safe forms, then measure candidate models on his own voice. Nobody has published
results for Swedish-accented English, and the gaps between leading models are
small enough for a speaker's accent to outweigh them.

No model reliably fixes "Scrawl" → "scroll". It is a real word, so it needs a
context check, and Koett doesn't have one yet.

Evidence tags:

- **[LOCAL]**: read from this machine on 2026-11-03.
- **[REPO]**: earlier benchmark in this repo.
- **[INDEP]**: third-party measurement.
- **[VENDOR]**: claim by the maker or an integrator.
- **[EST]**: my estimate, not measured.

## Ranked recommendation

| # | Do this | Expected gain | Added delay | Effort | Risk |
|---|---|---|---|---|---|
| 1 | Seed the dictionary with non-word forms ([starter list](#starter-dictionary)) | Fixes the 6 "coet" family misses, by the spelling rule **[EST]**. Term recall 31% → 70% on synthetic voices **[REPO]** | Linux about 92 ms on a 3–4 s clip; Mac about 236 ms **[REPO]** | 5 minutes, no code | About zero for non-word forms |
| 2 | Run the [experiment](#experiment-on-olles-voice) | Decides everything below on his voice | None | About 30 minutes | Read-aloud speech is not natural dictation |
| 3 | Delete um/uh with a deterministic pass, as its own setting | Removes about 2.5% of Parakeet v2's words **[LOCAL]** | Under 1 ms | Small: Rust, Swift, parity tests | Near zero. Keep the raw text. Changes the raw default, so it's Olle's call |
| 4 | Add a context check for real-word forms such as `scroll` | Hand replay kept 8/8 real uses and fixed 6/6 Scrawl uses, but n=14 and overfit **[EST]** | Under 1 ms | About 1 day | Medium |
| 5 | Try Parakeet Unified on both platforms | Small: offline WER 5.91 vs v2's 6.04 **[VENDOR]** | Similar speed **[VENDOR]** | Linux hours, Mac under a day **[EST]** | Hotwords and boost tuning untested on it |
| 6 | Linux only: try Qwen3-ASR 0.6B if #5 doesn't help | Conversational WER 12.74 vs v2's 13.58 **[INDEP]** | Maybe 3–6× slower than v2 **[EST]** | 1–2 days, sherpa ≥1.13.7 | 1.9 GB directory; worse on scripted speech (3.92 vs 3.62) |
| 7 | Mac: upgrade FluidAudio from 0.15.6 to 0.18.2 and re-tune the floor | Wrong replacements 35 → 12 at 50 terms, recall about flat **[VENDOR]** | Possibly lower **[VENDOR]** | Small, plus the benchmark the pin rule requires | Homophones remain |
| 8 | Optional LLM cleanup behind a deletion-only gate, long entries only | Restarts and self-corrections | +150–700 ms **[REPO]** | Medium | Can still delete a real hedge such as "I think" |

Not recommended:

- Free-form S1-mini in the normal path.
- Automatic phonetic matching.
- Prompt-biased Whisper as the default.

Reasons are in the sections below.

## What the local data shows

All **[LOCAL]**, read only, from `History.sqlite3` and `settings.json`. This
public file holds aggregates only.

- **Dictionary:** 0 entries, 0 replacements, and no `Corrections.json`. The Mac's
  dictionary was not inspected.
- **History:** 714 entries and about 47,500 words, median 34 words per entry.
  329 entries came from 110M, the rest almost all from v2.
- **Saved audio:** none. No `Failed Recordings` folder exists, and successful
  audio is deleted by design. That is why the experiment records new clips.
- **Misses, by hand reading:** "coet"/"coets" 6, "Poets" 1, and "scroll" 14, of
  which 6 mean Scrawl and 8 are real scrolling. "Koett" is written correctly 0 times.
- **Fillers (um/uh/er/erm):** v2 590 of 24,008 words (2.46%); 110M 139 of 23,098
  (0.60%). Daily v2 rates mostly run 2.1–3.0%. The models ran on different days
  with no controlled comparison, so this suggests v2 writes more fillers but
  doesn't prove it. Switching to 110M is not the fix: on clean speech it has
  3.69% WER against v2's 1.47% **[REPO]**.

## What the current pipeline does with each mishearing

From `CrossPlatform/src/dictionary/merge.rs` and `correct.rs`.

| Heard | Term alone | With a sounds-like form |
|---|---|---|
| coet | Adopted only if the biased decode emits "Koett" | Always fixed. "coets" needs its own form |
| Poets | Kept, because it's a common word | Every "poets" is rewritten |
| Taldi coet | Stays plain unless the biased decode gives "Taldi Koett"; a biased "the Koett" is a two-word span and fails the exact-term rule | Form `coet` gives "Taldi Koett" |
| scroll | Kept, because it's a common word | Every "scroll" is rewritten, wrong 8 times out of 14 |

Phrase forms can't add context, because the whole span becomes the term. Linux
literal replacements can, because they are case-sensitive and run afterwards.

On the Mac, `coet` and `poets` score 0.60 against Koett and `scroll` scores 0.67
against Scrawl. All are below the 0.70 floor unless given as aliases, and the
adoption gate still blocks the two real words.

### Starter dictionary

Exchange format for Settings → Dictionary → Import (then Save):

```text
Koett = coet, coets
Scrawl
```

Add other project names as terms only, and keep the list small. Do not add
`Scrawl = scroll` or `Koett = coat`. Linux can also take the literal replacements
`Poets` → `Koett` and `Taldi` → `the`; a real sentence-initial "Poets" would be
hit. Pick any Scrawl phrase replacements from your own History.

## Models

Scores are from the [Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard),
version 02-11-2026 **[INDEP]**. I re-read the rows from the Space's config. Speed
figures are from the sources named below the table.

| Model | Average | Conversational | Scripted | VoxPopuli-AA | Licence | Runs in Koett today |
|---|---|---|---|---|---|---|
| Parakeet v2 | 6.91 | 13.58 | 3.62 | 3.78 | CC BY | Mac, Linux |
| Parakeet v3 | 7.03 | 13.44 | 4.27 | 3.05 | CC BY | Linux; FluidAudio supports it |
| [Parakeet Unified](https://huggingface.co/nvidia/parakeet-unified-en-0.6b) | not listed | — | — | — | NVIDIA Open Model | Not wired. FluidAudio 0.15.6 [has a manager](https://github.com/FluidInference/FluidAudio/tree/v0.15.6/Sources/FluidAudio/ASR/Parakeet/Unified); sherpa [packages it](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html) |
| [Parakeet Ultra](https://huggingface.co/moondream/parakeet-ultra) | not listed | — | — | — | CC BY | Needs FluidAudio ≥0.17.3; no sherpa package found |
| [Qwen3-ASR 0.6B](https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/pretrained.html) | 6.99 | 12.74 | 3.92 | 2.90 | Apache | Not wired; sherpa supports it with hotwords |
| Qwen3-ASR 1.7B | 6.14 | 12.14 | 2.87 | 2.85 | Apache | Not wired; about 1.9 GB RAM on MLX **[VENDOR]** |
| Nemotron streaming EN | 7.37 | 13.28 | 4.79 | 2.40 | not checked | Experimental, spelling pass only |
| Whisper turbo (OpenAI row) | 8.68 | 15.40 | 4.05 | 7.02 | MIT | Not in Koett |
| Cohere Transcribe | 6.72 | 13.46 | 3.20 | 5.38 | Apache | Not wired; [no biasing documented](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) |

Speed:

- **Parakeet v2:** 129× on the Mac at 130 MB RSS, and 1.15 s per clip on Linux
  capped at two cores **[REPO]**.
- **Parakeet Unified:** 123× on an M5 Pro, 632 MB int8 **[VENDOR]**.
- **Qwen3 on MLX:** 0.6B 70×, 1.7B 36× **[VENDOR]**.
- **Whisper turbo:** 18.6 s per clip on this CPU under the same caps, and its
  Vulkan run ran out of GPU memory with 1 GB free **[REPO]**. Too slow for
  dictation without a free GPU.

Read these scores with care:

- **The gaps are small.** The same v2 weights score 13.33 or 13.58 on conversation
  depending on the backend, and a third-party Whisper turbo row averages 6.71
  against OpenAI's 8.68. Implementation choices move scores as much as the gaps
  above.
- **Benchmarks are partly fitted.** [One study](https://arxiv.org/abs/2608.19936)
  found top models reproducing benchmark reference errors **[INDEP]**. v2 scores
  high on that measure; Whisper and Qwen3 0.6B low. The v2-vs-Qwen3 gap may be
  larger than it looks.
- **Accent evidence is thin and indirect.**
  - [First-language distance](https://arxiv.org/abs/2608.30853) tracks WER, with
    v3 more sensitive (.585) than Whisper large (.370). It's not Swedish-specific
    **[INDEP]**.
  - [Entity recall](https://arxiv.org/abs/2609.20828) is 77–79% for Whisper and
    54–55% for Parakeet 110M, on speakers from India, Indonesia and Latin America
    with Gemini-generated references. It tests 110M, not v2 **[INDEP]**.
- **Long audio.** [Chunking hurts Qwen3 1.7B](https://arxiv.org/html/2604.14493v2)
  (5.90 → 10.45), so it would need whole-utterance decoding **[INDEP]**.

## Vocabulary methods

- **Koett, Linux:** term recall 31% → 70%, or 79% with accepted sounds-like forms,
  with 0 false terms **[REPO, synthetic]**.
- **Koett, Mac:** 30% → 72% at floor 0.70, 0 false terms **[REPO, synthetic]**.
- **Published biasing:** [TurboBias](https://arxiv.org/html/2508.07014) raises RNN-T
  beam recall from 29 to 76 while precision falls from 97 to 90 **[VENDOR]**.
  [CTC-WS](https://arxiv.org/pdf/2406.07096) takes a transducer's F from 0.44 to
  0.87 **[VENDOR]**.
- **Closest real-world warning:** in [FluidAudio #967](https://github.com/FluidInference/FluidAudio/issues/967),
  defaults changed 278 of 500 real dictations; on Common Voice, catching 41% of
  terms cost 9.3% wrong swaps **[one app vendor, self-reported]**. This supports
  Koett's conservative two-stream design. The maintainer says a later fix makes
  these worth re-measuring. [#1100](https://github.com/FluidInference/FluidAudio/pull/1100)
  is the improvement behind item 7.
- **Confidence as a gate for real words:** unmeasured here. sherpa v1.13.5
  [has `ys_log_probs`](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.5/sherpa-onnx/csrc/offline-stream.cc)
  but the pinned Rust struct drops them, so it needs a small binding. FluidAudio
  exposes `TokenTiming.confidence`.
- **Phonetic matching:** Koett, coet, coat, cot, cut, kit and Kate all give Metaphone
  `KT`; "scroll" and "Scrawl" share `SKRL`; "poets" is `PTS`. It over-fires and
  misses the hard cases. Koett's fuzzy pass reached 47% recall with 3 false terms
  **[REPO]**. [SpellMapper](https://arxiv.org/abs/2306.02317) reports 77.6–88.7%
  precision and adds a BERT model **[VENDOR]**.
- **Prompt biasing:** Koett's Parakeet path has no prompt input. [KWS-Whisper](https://arxiv.org/abs/2309.09552)
  raised entity recall to about 88% on one set but raised errors on technical talks
  (5.9 → 7.7), and warns of hallucination **[INDEP]**.
- **App scoping:** not recommended. He says real "scroll" while working on UIs, so
  the focused app wouldn't separate the two senses.

## Cleanup after transcription

**S1-mini in Koett.** Optional, temperature 0, falls back to raw text on failure.
Measured on an M5 with n-gram decoding: 150 ms at 50 words, 218 ms at 100, 691 ms
at 200 **[REPO]**. It made meaning changes: `option once` → `Option 1`, deleted
`I think it was called` at every quantization and style, and `FluidAudio` →
`Fluid Audio` **[REPO]**. Keep it out of the normal path.

**Two Mac gaps found while reading the code.**

- `DictationSession.swift:559` runs the spelling pass only before cleanup, while
  `docs/dictionary.md` says before and after. S1-mini can re-break a term.
- `S1MiniCleaner.swift:244` uses a 180 s request timeout, so a hung server could
  hold a paste far longer than the product rules intend.

**Deterministic first.** "um/uh" removal is nearly free. Don't remove "like" or
"you know" by rule: many are real, and [VoiceInk warns of the same](https://tryvoiceink.com/docs/filler-words).

**Small LLMs are weak at this.** On Switchboard, [base Qwen3-0.6B scored F 18–52 and
fine-tuned gpt-4o-mini 94.8](https://arxiv.org/pdf/2509.20321v1) **[INDEP]**. A
small [disfluency tagger](https://arxiv.org/pdf/2104.10769) reached F1 about 89 with
about 92% precision at 11–32 ms on a phone, but on clean transcripts, and no
off-the-shelf model was found **[INDEP]**.

**Good ASR leaves little to fix.** LLM correction [worsened Whisper large-v3 WER
from 3.11 to 3.34](https://arxiv.org/html/2407.21414v1) **[INDEP]**. On names,
[one study](https://arxiv.org/pdf/2509.19567v2) favoured biasing over LLM
post-correction on WER, with about 83.5% lower latency, though with retrieved
context rather than a personal list **[INDEP]**. An LLM has no audio to separate
"scroll" from "Scrawl".

**Shipping apps.** All latency and method statements here are vendor claims or
vendor docs.

| App | Cleanup | Fillers | Names |
|---|---|---|---|
| [Wispr Flow](https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary) | Cloud LLM | LLM | Learns from edits, skips everyday words |
| [Superwhisper](https://superwhisper.com/changelog) | S1-mini locally; cloud options | Automatic um/uh removal | [Vocabulary hints and replacements](https://superwhisper.com/docs/get-started/interface-vocabulary) |
| [Aqua Voice](https://aquavoice.com/vs/wispr-flow) | Cloud | Not stated | Custom terms |
| [VoiceInk](https://tryvoiceink.com/docs/filler-words) | Local; optional Ollama | Word list, no LLM needed | Replacements |
| [Handy](https://github.com/cjpais/Handy/pull/589) | Local | Regex list | Custom words |
| [Voxtype](https://github.com/peteonrails/voxtype) | Local; optional command | LLM hook | Replacement tables |

The local apps with built-in filler removal use word lists. Larger tuned LLMs mostly
live in the cloud apps. That pattern supports item 3 before item 8.

## Experiment on Olle's voice

No saved audio exists, so History supplies the sentences and he re-records them.
Everything stays on this machine, under `~/.cache/koett/accuracy-2026-11/`, outside
git. **Nothing is uploaded.**

**Pipeline check:** I ran steps 2–4 end to end on three public synthetic clips, and
the tools work. That run says nothing about accuracy. Those clips came out as "Cat"
and "code" with and without the dictionary, which shows the real-word limit but not
how Olle's voice behaves.

**1. Pick sentences from History (read only).**

```sh
X=~/.cache/koett/accuracy-2026-11; mkdir -p "$X/clips"
sqlite3 -readonly ~/.local/share/koett/History.sqlite3 \
  "select text from transcripts where text like '%coet%' or text like '%scroll%'
     or text like '%poets%' or length(text) between 80 and 200
   order by created_at_ms desc limit 200;" > "$X/candidates.txt"
```

Fix the words by hand and write `$X/corpus.json` in the shape of
`Benchmarks/Dictionary/corpus.json`, with `"voices": ["olle"]`:

- 30 `positive` sentences holding Koett, Scrawl and other names.
- 15 `negative` sentences with real "scroll", "poets", "coat" and "code".

**2. Record each one, spoken fast as you would dictate (about 15 minutes).**

```sh
pw-record --rate 16000 --channels 1 --format s16 "$X/clips/p01-olle.wav"   # Ctrl-C to stop
```

**3. Transcribe with each setup.** Build `koett-engine` first with
`cargo build --release --locked` in `CrossPlatform/`.

```sh
M=~/.local/share/koett/models; S=Benchmarks/Dictionary/transcribe.sh
printf 'Koett = coet, coets\nScrawl\n' > "$X/dictionary.txt"
V2=$M/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8; V3=$M/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8
bash $S "$V2" "$X/clips" "$X/v2.json"
bash $S "$V3" "$X/clips" "$X/v3.json"
KOETT_DICTIONARY="$X/dictionary.txt" bash $S "$V2" "$X/clips" "$X/v2-dict.json"
KOETT_DICTIONARY="$X/dictionary.txt" bash $S "$V3" "$X/clips" "$X/v3-dict.json"
```

**4. Score against plain v2.**

```sh
for r in v3 v2-dict v3-dict; do
  KOETT_BENCH_CORPUS="$X/corpus.json" python3 Benchmarks/Dictionary/score.py "$X/$r.json" "$X/v2.json"
done
```

Each `.json.log` also has per-clip `transcribe_ms`. Candidate models join the same
loop once they're downloaded.

**Decision rule, set in advance:**

- Keep the dictionary if `false_terms` and `broken_negatives` are 0 and term recall
  rises.
- Change the default model only if it beats v2 on both term recall and positive
  WER, with no extra lost words, and the median delay stays under 200 ms more per
  clip. 45 clips can show a large difference, not a 1-point WER gap, so a close
  result means keep v2.
- Changing the default still needs Olle's approval.

**Limits:** read-aloud is slower and tidier than real dictation, and the WER counts
any fillers he adds, so term recall is the main number. One speaker and one
microphone. It runs on Linux only; repeat on the Mac with
`DictionaryBenchmarkTests`, as `docs/dictionary.md` describes.

## Could not verify

- Any result for Swedish-accented English, on any model. VoxPopuli-AA is European
  Parliament speech, and I couldn't confirm it includes Swedish speakers.
- Whether the biased decode emits "Koett" or "Scrawl" on his voice. No real-voice
  test exists; the 70% figures are synthetic.
- How he says "Koett". "Poets" and "coet" suggest two syllables, but that's an
  inference.
- The hand labels for the 14 "scroll" hits come from short snippets.
- Parakeet Unified and Ultra have no independent scores, and Unified's licence tag
  differs between NVIDIA's card and the Core ML repo. Neither they nor Qwen3 were
  run on the Ryzen 9600X or M5 Air.
- Why v2 writes more fillers than 110M. Its card doesn't say.
- Whether FluidAudio 0.15.6 shares the log-prob bug fixed in 0.17.4.
- Apple Foundation Models and small Gemma, Phi or LFM models for cleanup: no
  latency per 50/100/200 words on Apple Silicon, and nothing on the RTX 4070.
- Meaning-change rates for LLM cleanup: no peer-reviewed measure.
- Wispr Flow's internals come from search snippets and reviews. Aqua Voice and
  Willow auto-learning have no official docs I could find.
- Web sources were read by research agents; I spot-checked that the main links
  load, the titles of three recent papers, and the leaderboard rows. Other numbers
  are as the agents reported them.

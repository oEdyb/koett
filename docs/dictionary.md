# Dictionary

The dictionary makes Koett write Olle's own words his way: names, products,
jargon and Swedish terms. Each entry is a written term plus optional "sounds
like" forms (what Koett writes instead). Linux and the Mac both ship it with the same rules.

## How it works on Linux

**Release default:** verified native builds share one encoder result between a
plain greedy decoder and a dictionary-biased beam decoder. The merge keeps plain
words and adopts only supported dictionary terms. This avoids encoding the same
audio twice. The four-length production benchmark measured 5.9–9.3% added median
ASR time, preserving 115/162 terms versus 115/162 for the old acoustic path.
See the [latency and recall audit](dictionary-latency-2026-11.md) for conditions,
tail latency and limits.

Older native libraries without the verified capability and CTC 110M models use
one greedy stream plus cheap spelling, explicit forms and exact-term correction.
They do not start the slow acoustic sidecar. The selected mode is logged at load.
The old two-encoder mode is available only to controlled benchmarks through
`ParakeetTranscriber::load_with_acoustic_dictionary`.

1. **Decoder biasing** (`CrossPlatform/src/transcription/hotwords.rs`). With a
   populated dictionary and a transducer model, the explicit native
   `koett_context` mode encodes identical stream features once. A greedy decoder
   supplies the base text; a four-path modified beam decoder applies per-stream
   hotwords with score 2.0. The native capability is bound to the pinned source,
   patched header, target and linked archive hashes at build time.
2. **Term adoption** (`CrossPlatform/src/dictionary/merge.rs`). The two word
   sequences are aligned. A differing biased span is used only when it is
   exactly one dictionary term; the term then replaces the plain words closest
   in sound, keeping the other plain words and their punctuation. The hotword
   boost also rewrites unrelated words ("Code Turns My Voice", "invoices" →
   "receipts"), so everything else keeps the plain decode. When the plain words
   are all common words, the biased term is treated as a boosted homophone
   ("the cloud" → "Claude", "Jag fikar" → "Figma") and the plain words stay.
   The lists are English SCOWL levels 10–20 (`dictionary/common-words.txt`) and
   25,000 Swedish forms from Språkbanken's Flashback statistics, CC BY 4.0
   (`dictionary/common-words-sv.txt`). An inflected plain word ("parakeets" for
   "Parakeet": +s, es, ed, ing) also stays. Multi-word terms in the biased text
   align as one unit ("Old Dyberg" → "Olle Dyberg"), and only sentence
   punctuation around the plain words is kept, so "G++." becomes "C++.".
3. **Spelling pass** (`CrossPlatform/src/dictionary/correct.rs`). Completed
   dictation rewrites spans of up to four words that match a term or a sounds-like
   form, ignoring case and hyphens but not punctuation. Terms with their own
   punctuation (`C++`, `C#`, `.NET`) match only with it and are replaced whole.
   Keys are NFC, so a decomposed "Göteborg" matches. Terms also match across
   spaces ("sherpa onnx"); a form only matches spans with its own word count, so
   the form "Superbase" leaves "the super base price" alone. One pass, no
   cascades within a pass. Runtime applies spelling before optional S1-mini and
   restores it on successful cleanup output. Without cleanup there is one pass.
   Literal replacements run afterwards.

An empty dictionary keeps the original single greedy decode. In explicit
acoustic benchmarks only, Parakeet 110M can use a verified, already-downloaded
v2 transducer sidecar with a 250 ms deadline
and raw CTC fallback. This is acoustic sidecar decoding, not CTC hotwords.
See [benchmark, setup and limits](ctc-dictionary-benchmark.md). Nemotron streaming
still gets spelling only. The engine
re-reads the saved dictionary once a second while idle and swaps hotwords
without reloading the acoustic model, including when the list becomes empty
or populated. If the vocabulary cache cannot be prepared at startup, raw
dictation still loads and uses spelling only; the selected fallback is logged.

Storage: `Settings.dictionary` in `settings.json`. Exchange format, one entry
per line, `#` comments: `Term` or `Term = heard as, other form`. Settings has a
Dictionary page to add, edit, remove, import and export (`linux/dictionary_view.rs`).
Parakeet ships no `bpe.vocab`; Koett derives one from `tokens.txt` into
`~/.cache/koett/Hotwords/`, never into the verified model folder.

## Starter list

[`docs/dictionary-seed.txt`](dictionary-seed.txt) is the shared starter list, in
the exchange format, for both platforms. Import it from Settings > Dictionary; on
Linux, `koett menu-dictionary-import /absolute/path` does the same in the running
app and keeps existing entries.

Koett = coet, coets · Claude = clawed · Opus · ChatGPT = chat GPT · Scrawl ·
Qwen = Quen, Qen · GitHub = Gitub · Kimi = Kimmy · Codex · Sonnet · Anthropic ·
OpenAI · T3 Code · MCP · QMD · Gemma · Obsidian · TikTok · Parakeet · Klippt

Forms come from counting recurring non-dictionary words in Olle's local Linux
History (717 transcripts): Gitub 12 next to GitHub 11, Quen 25, Qen 23, Kimmy 7.
No transcript text is in this repo. A form rewrites every match, so it must never
be a word with another meaning. `Claude = clawed` is Olle's own choice, and
`Scrawl = scroll`, `Koett = coat` and `Claude = cloud` stay out: "scroll" meant real
scrolling in 8 of 14 cases. People's names stay out of this public file.

## Evidence

Shared benchmark in `Benchmarks/Dictionary/`: 30 terms, 40 sentences using them,
30 near-miss sentences without them ("super base price", "parakeets in the
park", "Jacob and Matthias"), three synthetic voices, 210 clips. Synthetic
speech is cleaner than a microphone; treat numbers as relative.

| Parakeet v2, all voices | Term recall | False terms | WER pos. | WER neg. | Broken neg. | Lost words |
|---|---|---|---|---|---|---|
| Baseline (greedy) | 31% (50/162) | 0 | 14.3% | 0.28% | 0 | 0 |
| Fuzzy text correction only | 47% | 3 | 10.3% | 0.62% | 3 | — |
| Hotwords, single biased stream, score 2 | 73% | 0 | 6.4% | 0.79% | 4 + 66 case changes | 11 |
| Hotwords, score 3 / score 4 | 73% / 76% | 23 / 72 | — | — | 22 / 57 | — |
| PR #8: plain + biased, adopt any span with a term | 73% (118/162) | 0 | 6.6% | 0.16% | 0 | 6 |
| **Now: exact-term spans, common words kept** | **70% (114/162)** | **0** | **6.9%** | **0.16%** | **0** | **1** |
| Now + accepted learned sounds-like forms | 79% (128/162) | 0 | 5.1% | 0.16% | 0 | 1 |

Swedish benchmark (`corpus-sv.json`, run with `KOETT_BENCH_CORPUS=corpus-sv.json`):
16 sentences with terms, 16 everyday sentences with near-sounding words
("fikar", "kod", "alla", "kött", "koll", "Figuren"), two Swedish voices, Parakeet v3:
baseline 27% (12/44), dictionary 55% (24/44), 0 false terms, 0 broken
everyday sentences, 1 lost word.

"Lost words" counts words outside the terms that the baseline wrote correctly
and the output lost, in the 40 term sentences (`score.py` `collateral_words`).
PR #8 lost 6: "the cloud"/"the call"/"calling" became "Claude", and spans like
"Superbase invoices" were replaced whole. The remaining 1 comes from beam search
itself ("met in" → "met and"), which unbiased beam also produces.

Learning loop (approach c, prototype on branch `learning-correction-loop`):
corrections from one voice, aligned word by word, suggested 26 forms with no
junk from the near-miss sentences. Accepting them all automatically breaks
ordinary speech (`all → Olle`, `coat → Koett`). Accepting all but ordinary
English words (all, coat, Jakob, Matthias, "open code") gives the last row; on
the two voices it never learned from, recall goes 74% → 81% with 0 false terms.

Parakeet v3 baseline: 36% recall. Latency, release build, 2 threads, clips of
3–4 s: greedy median 96 ms, dictionary 188 ms (two streams). Most audio is
decoded while recording, so only the last segment adds to the wait. Integration
checks: `scripts/test_linux_dictionary.py` (Settings page, and with `--model`
a real dictation of benchmark clip p05 that turns "Nematron" into "Nemotron" after Save, no restart; the plain decode of "Superbase" proved borderline through the lab microphone);
an ignored model test in `transcription/hotwords.rs` (Phebe → Phoebe and back).

**Fix a word** (Linux, `linux/fix_word_view.rs`, `corrections/`): History →
select an entry → Fix a word…. Olle edits what Koett wrote and saves. The
correction goes to `~/.local/share/koett/Corrections.json`, keyed by entry ID,
with the original snapshot; history text and the shared history database stay
untouched, and saving the original again removes the correction. The learner
(from the `learning-correction-loop` prototype) aligns every saved correction and
proposes "Write Groq when Koett hears Grok". Each suggestion needs an explicit
Add or Dismiss; dismissals persist. A form made only of common English words
(the SCOWL list) shows a warning and the button reads "Add anyway". Imported
archive entries cannot be fixed, since one archive entry may not be one recording.

Remaining misses are words Parakeet hears as ordinary English ("all" for Olle,
"code"/"coat" for Koett, "Jakob" for Yakob, "Quinn" for Qwen). Hotwords cannot
safely override real words; a sounds-like form can, at the user's choice.

## Seed list

Exchange format, for Dictionary → Import or `koett --import-dictionary FILE`. Project
and tool names Olle says often; the forms are mishearings found in his Mac History
(398 entries, mined locally). Never add `scroll` as a form of Scrawl: it was the real
word in 8 of 14 cases. Keep `Koett = coat` out for the same reason.

```text
Koett = coet, coets
Claude = clawed
Opus
ChatGPT = chat GPT
Scrawl
Codex
Kimi = Kimmy
Qwen = Quen
Karpathy = Karpothy
Wispr Flow = Whisperflow, whisper flow
Excalidraw = Excaladra, Excala
Nemotron
Parakeet
OpenClaw
QMD
Klippt
Olle Dyberg
```

## How it works on the Mac

Same rules, ported to Swift in `Sources/Koett/Dictionary/`:

1. **Storage**: `[DictionaryEntry]` as JSON in the preferences (`dictionary`
   key, the same `term`/`sounds_like` fields as Linux settings). Menu →
   Dictionary… edits it, with Import… and Export… in the same text format.
2. **Vocabulary boosting** (`VocabularyBooster.swift`). FluidAudio has no TDT
   hotword beam search, so Parakeet's text is rescored with the separate
   Parakeet CTC 110M model (`VocabularyBoostingSession`, about 100 MB). The
   model loads in the background only while the dictionary has words and is
   dropped when it is emptied. Dictation never waits for it: until it is ready,
   after a failure, or when rescoring takes over 2 s, the plain text is used.
   The rescored text then goes through the same term adoption as Linux.
   Rescorer floor: string similarity 0.70 for Parakeet v2 (the Mac app's model)
   and 0.85 for Parakeet v3; acoustic rescue pass off (`BoostTuning.forModel`).
3. **Spelling pass** in `DictationSession.prepareDictation`, before the optional
   S1-mini cleanup, so the cleaner sees the right word, and on its output, because
   it can break a term again. Nemotron gets the spelling pass only. Literal
   replacements (`Replacements.swift`, the Replacements tab) run last, on the
   final text.
4. **Import for agents**: `koett --import-dictionary FILE` merges the exchange
   format through the same store as the window. It refuses while Koett is running,
   since the running app would overwrite the change.
5. **Fix a word**: History → Fix a Word…, `~/Library/Application Support/Koett/Corrections.json`
   (same JSON shape, 0600), explicit Add or Dismiss, everyday-word warning.

`Tests/KoettTests/Dictionary/Fixtures/parity.json` holds the Rust output for
benchmark transcripts; `DictionaryParityTests` requires identical Swift output.
Regenerate it whenever `CrossPlatform/src/dictionary` or `corrections/learn.rs`
changes.

### Mac evidence

Same 210 clips (`synthesize.py`), Parakeet v2 with silence trimming, the 30
corpus terms, no sounds-like forms. Run with
`KOETT_DICTIONARY_BENCH=<clips> KOETT_DICTIONARY_BENCH_OUT=<dir> swift test --filter DictionaryBenchmarkTests`,
then `score.py <dir>/mac-boosted-koett.json <dir>/mac-baseline.json`.

| Mac, all voices | Term recall | False terms | WER pos. | WER neg. | Broken neg. | Lost words |
|---|---|---|---|---|---|---|
| Baseline | 30% (49/162) | 0 | 14.4% | 0.28% | 0 | 0 |
| Spelling pass only | 38% (61/162) | 0 | 12.4% | 0.28% | 0 | 0 |
| CTC boosting, FluidAudio defaults | 81% (131/162) | 18 | 4.8% | 2.9% | 15 | 3 |
| Similarity floor 0.60 | 81% (131/162) | 12 | 4.8% | 2.1% | 9 | 3 |
| **Similarity floor 0.70 (shipped)** | **72% (117/162)** | **0** | **5.8%** | **0.28%** | **0** | **0** |

At FluidAudio defaults the rescorer turned "salad" into "Wayland", "Highland"
into "Hyprland", "Jacob and Matthias" into "Yakob and Matyas", "Gothenburg" into
"Göteborg" and "parakeets" into "Parakeet". Floors 0.65 and 0.70 insert none
(0.65: 120/162). A plain word that is the term plus "s", "es", "ed" or "ing"
stays. Floors 0.75/0.80/0.85 give 111/86/74 terms, all with 0 false terms.
Latency per 3–4 s clip, release-to-text: median 74 ms plain, about 310 ms
with boosting.

Swedish (`KOETT_BENCH_CORPUS=corpus-sv.json KOETT_DICTIONARY_BENCH_MODEL=v3`,
64 clips, Parakeet v3, 31 terms):

| Mac, Swedish | Term recall | False terms | WER pos. | WER neg. | Broken neg. | Lost words |
|---|---|---|---|---|---|---|
| Baseline | 39% (17/44) | 0 | 12.5% | 6.2% | 0 | 0 |
| Spelling pass only | 55% (24/44) | 0 | 12.5% | 6.2% | 0 | 0 |
| Floor 0.70 | 91% (40/44) | 4 | 4.3% | 8.3% | 2 | 0 |
| Floor 0.80 | 82% (36/44) | 2 | 6.4% | 7.2% | 2 | 0 |
| **Floor 0.85 (v3 setting)** | **66% (29/44)** | **0** | **9.9%** | **6.2%** | **0** | **0** |

Below 0.85 the rescorer turns "Jakob och Mattias" into "Yakob och Matyas", which
sound the same. The Mac app ships Parakeet v2 only, so v3's floor is recorded
for when the multilingual model reaches the Mac.

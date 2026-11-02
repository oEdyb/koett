# Dictionary

The dictionary makes Koett write Olle's own words his way: names, products,
jargon and Swedish terms. Each entry is a written term plus optional "sounds
like" forms (what Koett writes instead). Linux ships it; the Mac to-do is below.

## How it works on Linux

1. **Decoder biasing** (`CrossPlatform/src/transcription/hotwords.rs`). With a
   non-empty dictionary and a transducer model (Parakeet v2/v3), each utterance
   decodes twice in one `decode_multiple_streams` batch: plain, and with the
   terms as sherpa-onnx per-stream hotwords (modified beam search, 4 paths,
   score 2.0, as the pinned upstream Parakeet TDT example uses).
2. **Term adoption** (`CrossPlatform/src/dictionary/merge.rs`). The two word
   sequences are aligned. A differing biased span is used only when it is
   exactly one dictionary term; the term then replaces the plain words closest
   in sound, keeping the other plain words and their punctuation. The hotword
   boost also rewrites unrelated words ("Code Turns My Voice", "invoices" →
   "receipts"), so everything else keeps the plain decode. When the plain words
   are all common English (SCOWL levels 10–20, `dictionary/common-words.txt`),
   the biased term is treated as a boosted homophone ("the cloud" → "Claude")
   and the plain words stay.
3. **Spelling pass** (`CrossPlatform/src/dictionary/correct.rs`). Completed
   dictation rewrites spans of up to four words that match a term or a sounds-like
   form, ignoring case and hyphens but not punctuation. Terms with their own
   punctuation (`C++`, `C#`, `.NET`) match only with it and are replaced whole.
   Keys are NFC, so a decomposed "Göteborg" matches. Terms also match across
   spaces ("sherpa onnx"); a form only matches spans with its own word count, so
   the form "Superbase" leaves "the super base price" alone. One pass, no
   cascades. Literal replacements run afterwards.

An empty dictionary keeps the original single greedy decode. CTC models
(Parakeet 110M) and Nemotron streaming get the spelling pass only. The engine
re-reads the saved dictionary once a second while idle and swaps hotwords
without reloading; only turning biasing on or off reloads the decoder.

Storage: `Settings.dictionary` in `settings.json`. Exchange format, one entry
per line, `#` comments: `Term` or `Term = heard as, other form`. Settings has a
Dictionary page to add, edit, remove, import and export (`linux/dictionary_view.rs`).
Parakeet ships no `bpe.vocab`; Koett derives one from `tokens.txt` into
`~/.cache/koett/Hotwords/`, never into the verified model folder.

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
a real dictation that turns "Superbase" into "Supabase" after Save, no restart);
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

## Mac to-do

The Mac app has no dictionary yet. Port in this order:

1. Storage: a `[DictionaryEntry]` (term, soundsLike) in the Mac preferences,
   plus the same plain-text import/export format so one file moves between
   machines.
2. Editor: a small SwiftUI window opened from the menu (like History) with
   Word / Sounds like rows, Add, Remove, Import… and Export… via
   `NSOpenPanel`/`NSSavePanel`. No large settings screen.
3. Spelling pass: port `correct.rs` (about 120 lines: Unicode word spans, key =
   lowercased alphanumerics, longest span first, single pass) and run it in
   `DictationSession` before delivery. Port its unit tests.
4. Decoder biasing: FluidAudio 0.15.6 has no TDT hotword beam search. It offers
   CTC vocabulary boosting (`Documentation/ASR/CustomVocabulary.md`,
   `VocabularyBoostingSession`). Parakeet 0.6B v2/v3 needs a separate CTC
   encoder (Parakeet 110M CTC, about 130 MB, about 26× real time): build a
   `CustomVocabularyContext` from the terms, create
   `VocabularyBoostingSession(vocabulary:ctcModels:)`, then
   `rescore(text:tokenTimings:audioSamples:)` after the TDT transcript. Keep the
   session loaded; fall back to the raw transcript on `nil` or error. Load the
   CTC models only when the dictionary is non-empty.
5. Fix a word: the same per-entry correction file (same JSON shape), a Swift
   port of `corrections/learn.rs`, and a History action with explicit
   Add/Dismiss. Never apply learned forms automatically.
6. Measure before shipping: run `Benchmarks/Dictionary` clips through the Mac
   path and require 0 false terms on the negatives, as on Linux. Then run the
   Swift test suite and the release build checks in `AGENTS.md`.

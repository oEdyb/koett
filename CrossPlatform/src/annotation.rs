//! Platform-neutral timing for "draw while dictating".
//!
//! ASR produces timed words; the desktop overlay produces keyframes (one screen
//! image per group of strokes). This module links them: each keyframe owns the
//! words from the start of the clause in progress when its drawing began until
//! the next keyframe, and the text handed to the target application lists them
//! as `- #N (M:SS) “words”` bullets matching the numbered contact sheet.

use std::ops::Range;

/// Pauses at least this long end a clause even without punctuation.
const CLAUSE_PAUSE_SECONDS: f32 = 1.5;
/// Length assumed for a final token whose end the recognizer does not report.
const LAST_TOKEN_SECONDS: f32 = 0.3;

#[derive(Clone, Debug, PartialEq)]
pub struct TimedWord {
    pub text: String,
    /// Seconds from the start of the recording.
    pub start: f32,
    pub end: f32,
}

/// Groups subword tokens into whitespace-separated words. Returns `None` when
/// the tokens do not reproduce `text` word for word, so callers can fall back
/// to [`estimate_words`] instead of attaching times to the wrong words.
pub fn words_from_tokens(
    text: &str,
    tokens: &[String],
    timestamps: &[f32],
    offset: f32,
    duration: f32,
) -> Option<Vec<TimedWord>> {
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }
    let mut words: Vec<TimedWord> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let start = offset + timestamps[index];
        let end = timestamps.get(index + 1).map_or(
            (timestamps[index] + LAST_TOKEN_SECONDS).min(duration),
            |next| *next,
        ) + offset;
        let begins_word = token.starts_with(char::is_whitespace) || words.is_empty();
        let piece = token.trim();
        if piece.is_empty() {
            continue;
        }
        match words.last_mut() {
            Some(word) if !begins_word => {
                word.text.push_str(piece);
                word.end = end;
            }
            _ => words.push(TimedWord {
                text: piece.to_owned(),
                start,
                end,
            }),
        }
    }
    words
        .iter()
        .map(|word| word.text.as_str())
        .eq(text.split_whitespace())
        .then_some(words)
}

/// Spreads `text` over `start..end` by character count. Used when the
/// recognizer reports no usable token times (merged chunks or streaming).
pub fn estimate_words(text: &str, start: f32, end: f32) -> Vec<TimedWord> {
    let pieces: Vec<&str> = text.split_whitespace().collect();
    let total: usize = pieces.iter().map(|word| word.chars().count() + 1).sum();
    let span = (end - start).max(0.0);
    let mut consumed = 0;
    pieces
        .into_iter()
        .map(|word| {
            let length = word.chars().count() + 1;
            let from = start + span * consumed as f32 / total.max(1) as f32;
            consumed += length;
            TimedWord {
                text: word.to_owned(),
                start: from,
                end: start + span * consumed as f32 / total.max(1) as f32,
            }
        })
        .collect()
}

/// Text plus word times, joined the way transcription joins segment texts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimedText {
    pub text: String,
    pub words: Vec<TimedWord>,
}

impl TimedText {
    pub fn push(&mut self, text: &str, words: Vec<TimedWord>) {
        if text.is_empty() {
            return;
        }
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        self.text.push_str(text);
        self.words.extend(words);
    }
}

fn ends_clause(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', ')', '”', '’'])
        .ends_with(['.', ',', ';', ':', '?', '!', '…'])
}

/// Index of the first word of the clause in progress at `anchor` seconds.
fn clause_start(words: &[TimedWord], anchor: f32) -> usize {
    let begun = words.iter().take_while(|word| word.start < anchor).count();
    (1..=begun)
        .rev()
        .find(|&index| {
            let previous = &words[index - 1];
            let gap = words
                .get(index)
                .map_or(anchor - previous.end, |next| next.start - previous.end);
            ends_clause(&previous.text) || gap >= CLAUSE_PAUSE_SECONDS
        })
        .unwrap_or(0)
}

/// Word ranges owned by each keyframe, given each keyframe's anchor time in
/// ascending order. Words before the first keyframe belong to it; every word
/// belongs to exactly one keyframe.
pub fn keyframe_word_ranges(words: &[TimedWord], anchors: &[f32]) -> Vec<Range<usize>> {
    let mut starts: Vec<usize> = Vec::with_capacity(anchors.len());
    for (index, anchor) in anchors.iter().enumerate() {
        let start = if index == 0 {
            0
        } else {
            let previous = starts[index - 1];
            let clause = clause_start(words, *anchor);
            if clause > previous {
                clause
            } else {
                // Several drawings in one clause: split at the word in progress.
                let begun = words.iter().take_while(|word| word.start < *anchor).count();
                begun.saturating_sub(1).max(previous)
            }
        };
        starts.push(start);
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| *start..starts.get(index + 1).copied().unwrap_or(words.len()))
        .collect()
}

/// `M:SS` clock for keyframe bullets.
pub fn clock(seconds: f32) -> String {
    let whole = seconds.max(0.0) as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// Builds the pasted text. `final_text` is the delivered transcript after
/// replacements or cleanup; ranges computed on the timed recognizer words are
/// mapped by matching surviving words, with proportional fallback for unmatched spans.
pub fn keyframe_summary(final_text: &str, words: &[TimedWord], anchors: &[f32]) -> String {
    if anchors.is_empty() {
        return final_text.to_owned();
    }
    let output: Vec<&str> = final_text.split_whitespace().collect();
    let boundaries = aligned_boundaries(words, &output);
    let scale = |index: usize| {
        if let Some(ref boundaries) = boundaries {
            return boundaries[index];
        }
        if words.is_empty() {
            if index == 0 { 0 } else { output.len() }
        } else {
            (index * output.len() + words.len() / 2) / words.len()
        }
    };
    let ranges = if words.is_empty() {
        // No timing at all: the first keyframe carries the whole transcript.
        let mut ranges = vec![0..0; anchors.len()];
        ranges[0] = 0..1;
        ranges
    } else {
        keyframe_word_ranges(words, anchors)
    };
    let mut summary = String::new();
    for (index, (range, anchor)) in ranges.iter().zip(anchors).enumerate() {
        let (from, to) = (scale(range.start), scale(range.end));
        let said = output[from.min(output.len())..to.min(output.len())].join(" ");
        summary.push_str(&format!("- #{} ({})", index + 1, clock(*anchor)));
        if said.is_empty() {
            summary.push_str(" marked\n");
        } else {
            summary.push_str(&format!(" “{said}”\n"));
        }
    }
    summary.truncate(summary.trim_end().len());
    summary
}

#[cfg(test)]
mod tests;

/// Align unchanged words across cleanup. Unmatched spans retain proportional
/// boundaries; matched words pin boundaries so deleted fillers cannot shift them.
fn aligned_boundaries(words: &[TimedWord], output: &[&str]) -> Option<Vec<usize>> {
    let n = words.len();
    let m = output.len();
    // Bound quadratic work for unusually long recordings.
    if n == 0 || n.checked_mul(m)? > 1_000_000 {
        return None;
    }
    let normalize = |s: &str| {
        s.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let source: Vec<_> = words.iter().map(|w| normalize(&w.text)).collect();
    let target: Vec<_> = output.iter().map(|w| normalize(w)).collect();
    let mut scores = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            scores[i][j] = if !source[i].is_empty() && source[i] == target[j] {
                1 + scores[i + 1][j + 1]
            } else {
                scores[i + 1][j].max(scores[i][j + 1])
            };
        }
    }
    if scores[0][0] == 0 {
        return None;
    }
    let mut pins = vec![(0, 0)];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if !source[i].is_empty() && source[i] == target[j] {
            pins.push((i, j));
            pins.push((i + 1, j + 1));
            i += 1;
            j += 1;
        } else if scores[i + 1][j] >= scores[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pins.push((n, m));
    let mut boundaries = vec![0; n + 1];
    for pair in pins.windows(2) {
        let ((a, b), (c, d)) = (pair[0], pair[1]);
        for (k, boundary) in boundaries.iter_mut().enumerate().take(c + 1).skip(a) {
            *boundary = b + (k - a) * (d - b) / (c - a).max(1);
        }
    }
    Some(boundaries)
}

#[cfg(test)]
mod evaluation;

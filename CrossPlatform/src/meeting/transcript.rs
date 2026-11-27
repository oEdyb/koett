pub(crate) mod helper;
mod markdown;
mod timeline;
mod windows;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use time::{OffsetDateTime, UtcOffset};

use crate::annotation::TimedWord;
use crate::dictionary::DictionaryEntry;
use crate::model::{LocalModel, VAD_FILE, validate_local_model_until_cancelled};
use crate::settings::Settings;
use crate::transcription::{ParakeetTranscriber, StreamingVad};

use super::bundle::Bundle;
use super::bundle::marks::list_marks;
use super::bundle::records::Seal;
use super::capture::journal::{TrackChunk, TrackReader};
use super::session::journal_path;
use super::{Backend, Channel, Offset, SAMPLE_RATE, Span};
use helper::Helper;
use timeline::{Point, SpeechWord};
use windows::WindowBuilder;

pub(super) use markdown::{Rendered, history_text, vault_note};

const BATCH_SIZE: usize = 8;
const PARAKEET: &str = "parakeet-v3";
const SAMPLE_RATE_F64: f64 = SAMPLE_RATE as f64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct WindowId {
    pub(super) channel: Channel,
    pub(super) sequence: u64,
}

pub(super) struct Window {
    pub(super) id: WindowId,
    pub(super) first: Offset,
    pub(super) core: Span,
    ownership: WordOwnership,
    pub(super) samples: Vec<f32>,
}

#[derive(Default)]
struct WordOwnership {
    after: Option<f64>,
    through: Option<f64>,
}

impl WordOwnership {
    fn contains(&self, midpoint: f64) -> bool {
        self.after.is_none_or(|after| midpoint > after)
            && self.through.is_none_or(|through| midpoint <= through)
    }
}

impl Window {
    fn divide_padding(earlier: &mut Self, later: &mut Self) {
        let earlier_end = i128::from(earlier.core.end.0);
        let later_start = i128::from(later.core.first.0);
        let doubled_boundary = earlier_end + earlier_end.max(later_start);
        let from_window = |first: Offset| (doubled_boundary - 2 * i128::from(first.0)) as f64 / 2.0;
        earlier.ownership.through = Some(from_window(earlier.first));
        later.ownership.after = Some(from_window(later.first));
    }
}

pub(super) struct DecodedWindow {
    pub(super) id: WindowId,
    pub(super) words: Vec<TimedWord>,
    pub(super) lang: String,
    pub(super) backend: String,
}

pub(super) trait Decoder {
    fn decode(&mut self, windows: &[Window]) -> Result<Vec<DecodedWindow>, String>;
}

struct Parakeet<'a> {
    dictionary: &'a [DictionaryEntry],
    cancelled: &'a AtomicBool,
    engine: Option<ParakeetTranscriber>,
}

impl Decoder for Parakeet<'_> {
    fn decode(&mut self, windows: &[Window]) -> Result<Vec<DecodedWindow>, String> {
        if self.engine.is_none() {
            check_cancelled(self.cancelled)?;
            let directory = LocalModel::ParakeetV3.directory()?;
            validate_local_model_until_cancelled(
                LocalModel::ParakeetV3,
                &directory,
                self.cancelled,
            )?;
            self.engine = Some(ParakeetTranscriber::load_with_dictionary(
                &directory,
                4,
                self.dictionary,
            )?);
        }
        let engine = self.engine.as_ref().ok_or("Parakeet was not loaded")?;
        windows
            .iter()
            .map(|window| {
                check_cancelled(self.cancelled)?;
                Ok(DecodedWindow {
                    id: window.id,
                    words: engine.decode_window(&window.samples)?,
                    lang: "und".into(),
                    backend: PARAKEET.into(),
                })
            })
            .collect()
    }
}

struct MeetingDecoder<'a, D> {
    helper: Option<Helper>,
    fallback: D,
    cancelled: &'a AtomicBool,
    batch_base: Duration,
    per_window: Duration,
}

impl<D: Decoder> Decoder for MeetingDecoder<'_, D> {
    fn decode(&mut self, windows: &[Window]) -> Result<Vec<DecodedWindow>, String> {
        check_cancelled(self.cancelled)?;
        let Some(mut helper) = self.helper.take() else {
            return self.fallback.decode(windows);
        };
        let budget = self.batch_base + self.per_window * windows.len() as u32;
        let (mut decoded, failure) = helper.decode_prefix(windows, self.cancelled, budget);
        if let Some(error) = failure {
            let error = helper.with_stderr(error);
            check_cancelled(self.cancelled)?;
            eprintln!("Meeting ASR helper failed, using Parakeet. {error}");
            if decoded.len() < windows.len() {
                decoded.extend(self.fallback.decode(&windows[decoded.len()..])?);
            }
        } else {
            self.helper = Some(helper);
        }
        Ok(decoded)
    }
}

pub(super) fn transcribe(
    bundle: &Bundle,
    seal: &Seal,
    preferred: Backend,
    settings: &Settings,
    cancelled: &AtomicBool,
) -> Result<Rendered, String> {
    let mut points = mark_points(bundle, seal)?;
    let parakeet = LocalModel::ParakeetV3.directory()?;
    let vad_directory = if parakeet.join(VAD_FILE).is_file() {
        parakeet
    } else {
        settings.model_directory()?
    };
    let mut vad = StreamingVad::load(&vad_directory)?;
    let helper = match (preferred, settings.meeting.gpu_python.as_deref()) {
        (Backend::Gpu, Some(python)) => match Helper::start(bundle, python, cancelled) {
            Ok(helper) => Some(helper),
            Err(error) => {
                check_cancelled(cancelled)?;
                eprintln!("Meeting ASR helper unavailable, using Parakeet. {error}");
                None
            }
        },
        _ => None,
    };
    let mut decoder = MeetingDecoder {
        helper,
        fallback: Parakeet {
            dictionary: &settings.dictionary,
            cancelled,
            engine: None,
        },
        cancelled,
        batch_base: Duration::from_secs(30),
        per_window: Duration::from_secs(10),
    };
    let mut batch = Batch::new(&mut decoder, preferred);
    for channel in [Channel::You, Channel::Other] {
        read_track(
            bundle,
            seal,
            channel,
            &mut vad,
            &mut batch,
            &mut points,
            cancelled,
        )?;
    }
    batch.decode(&mut points)?;
    check_cancelled(cancelled)?;
    render(bundle, seal, points, &settings.dictionary, batch.fallback)
}

#[cfg(test)]
pub(super) fn render_marks(bundle: &Bundle, seal: &Seal) -> Result<Rendered, String> {
    render(bundle, seal, mark_points(bundle, seal)?, &[], false)
}

fn mark_points(bundle: &Bundle, seal: &Seal) -> Result<Vec<Point>, String> {
    let marks = list_marks(bundle.directory(), Some(seal)).map_err(|err| err.to_string())?;
    Ok(marks.into_iter().map(Point::Mark).collect())
}

fn render(
    bundle: &Bundle,
    seal: &Seal,
    points: Vec<Point>,
    dictionary: &[DictionaryEntry],
    fallback: bool,
) -> Result<Rendered, String> {
    let utc = OffsetDateTime::from_unix_timestamp_nanos(
        i128::from(bundle.manifest().info.started_at_ms) * 1_000_000,
    )
    .map_err(|err| err.to_string())?;
    let offset = UtcOffset::local_offset_at(utc).unwrap_or(UtcOffset::UTC);
    markdown::render(
        bundle.manifest(),
        utc.to_offset(offset),
        seal.cutoff,
        timeline::merge(points),
        dictionary,
        fallback,
    )
}

struct Batch<'a, D> {
    decoder: &'a mut D,
    windows: Vec<Window>,
    preferred: Backend,
    fallback: bool,
}

impl<'a, D: Decoder> Batch<'a, D> {
    fn new(decoder: &'a mut D, preferred: Backend) -> Self {
        Self {
            decoder,
            windows: Vec::with_capacity(BATCH_SIZE),
            preferred,
            fallback: false,
        }
    }

    fn push(&mut self, window: Window, points: &mut Vec<Point>) -> Result<(), String> {
        self.windows.push(window);
        if self.windows.len() == BATCH_SIZE {
            self.decode(points)?;
        }
        Ok(())
    }

    fn decode(&mut self, points: &mut Vec<Point>) -> Result<(), String> {
        if self.windows.is_empty() {
            return Ok(());
        }
        let decoded = self.decoder.decode(&self.windows)?;
        let normalized = validate(&self.windows, decoded)?;
        self.fallback |= normalized.iter().any(|result| {
            (result.backend.as_str() == PARAKEET) != (self.preferred == Backend::Parakeet)
        });
        points.extend(
            normalized
                .into_iter()
                .flat_map(|result| result.words)
                .map(Point::Word),
        );
        self.windows.clear();
        Ok(())
    }
}

fn read_track(
    bundle: &Bundle,
    seal: &Seal,
    channel: Channel,
    vad: &mut StreamingVad,
    batch: &mut Batch<'_, impl Decoder>,
    points: &mut Vec<Point>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let cutoff = seal.cutoff;
    let track = match channel {
        Channel::You => seal.tracks.you,
        Channel::Other => seal.tracks.other,
    };
    let mut builder = WindowBuilder::new(vad, channel);
    let mut end = Offset(0);
    let mut missing = None;
    if track.valid_bytes != 0 {
        let path = journal_path(bundle.directory(), channel);
        let reader =
            TrackReader::open_sealed(&path, cutoff, track).map_err(|err| err.to_string())?;
        let silence = [0.0; 8000];
        let mut samples_f32 = Vec::with_capacity(silence.len());
        for chunk in reader {
            check_cancelled(cancelled)?;
            let chunk = chunk.map_err(|err| err.to_string())?;
            let first = match &chunk {
                TrackChunk::Pcm { first, .. } => *first,
                TrackChunk::Silence(span) | TrackChunk::Gap { span, .. } => span.first,
            };
            if first > end {
                append_gap(
                    &mut missing,
                    Span {
                        first: end,
                        end: first,
                    },
                );
            }
            if let TrackChunk::Gap { span, .. } = chunk {
                end = end.max(span.end);
                append_gap(
                    &mut missing,
                    Span {
                        first: span.first,
                        end,
                    },
                );
                continue;
            }
            if let Some(span) = missing.take() {
                builder.end_at(first, |window| batch.push(window, points))?;
                points.push(Point::MissingAudio { channel, span });
            }
            match chunk {
                TrackChunk::Pcm { first, samples } => {
                    end = Offset(first.0 + samples.len() as u64);
                    samples_f32.clear();
                    samples_f32.extend(
                        samples
                            .into_iter()
                            .map(|sample| f32::from(sample) / 32_768.0),
                    );
                    builder.push(&samples_f32, |window| batch.push(window, points))?;
                }
                TrackChunk::Silence(span) => {
                    end = span.end;
                    let len = (span.end.0 - span.first.0) as usize;
                    builder.push(&silence[..len], |window| batch.push(window, points))?;
                }
                TrackChunk::Gap { .. } => unreachable!(),
            }
        }
        builder.end_at(cutoff, |window| batch.push(window, points))?;
    }
    if end < cutoff {
        append_gap(
            &mut missing,
            Span {
                first: end,
                end: cutoff,
            },
        );
    }
    if let Some(span) = missing {
        points.push(Point::MissingAudio { channel, span });
    }
    Ok(())
}

fn append_gap(pending: &mut Option<Span>, span: Span) {
    match pending {
        Some(previous) => {
            previous.first = previous.first.min(span.first);
            previous.end = previous.end.max(span.end);
        }
        None => *pending = Some(span),
    }
}

struct NormalizedWindow {
    backend: String,
    words: Vec<SpeechWord>,
}

fn validate(
    windows: &[Window],
    decoded: Vec<DecodedWindow>,
) -> Result<Vec<NormalizedWindow>, String> {
    let mut by_id = HashMap::with_capacity(decoded.len());
    for result in decoded {
        let id = result.id;
        if by_id.insert(id, result).is_some() {
            return Err(format!("Duplicate decode result for {id:?}"));
        }
    }
    let validated = windows
        .iter()
        .map(|window| {
            let result = by_id
                .remove(&window.id)
                .ok_or_else(|| format!("Missing decode result for {:?}", window.id))?;
            normalize(window, result)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !by_id.is_empty() {
        return Err("Unexpected decode result ID".into());
    }
    Ok(validated)
}

fn validate_words(window: &Window, result: &DecodedWindow) -> Result<(), String> {
    let duration = window.samples.len() as f64 / SAMPLE_RATE_F64;
    let mut previous = 0.0;
    if !label(&result.lang) || !label(&result.backend) {
        return Err(format!("Invalid language or backend in {:?}", window.id));
    }
    for word in &result.words {
        let start = f64::from(word.start);
        let end = f64::from(word.end);
        if !start.is_finite()
            || !end.is_finite()
            || start < previous
            || end < start
            || end > duration + 0.05
        {
            return Err(format!("Invalid word timestamps in {:?}", window.id));
        }
        previous = start;
    }
    Ok(())
}

fn normalize(window: &Window, result: DecodedWindow) -> Result<NormalizedWindow, String> {
    validate_words(window, &result)?;
    let to_offset = |seconds: f64| {
        Offset(
            window
                .first
                .0
                .saturating_add((seconds * SAMPLE_RATE_F64).round() as u64),
        )
    };
    let mut words = Vec::new();
    for word in result.words {
        let start = f64::from(word.start);
        let end = f64::from(word.end);
        let midpoint = f64::midpoint(start, end) * SAMPLE_RATE_F64;
        if window.ownership.contains(midpoint) && !word.text.trim().is_empty() {
            words.push(SpeechWord {
                channel: window.id.channel,
                span: Span {
                    first: to_offset(start),
                    end: to_offset(end),
                },
                text: word.text,
                backend: result.backend.clone(),
                lang: result.lang.clone(),
            });
        }
    }
    Ok(NormalizedWindow {
        backend: result.backend,
        words,
    })
}

fn label(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("Meeting transcription was cancelled".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

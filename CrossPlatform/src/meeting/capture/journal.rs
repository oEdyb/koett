use std::fs::File;
use std::io::{self, BufReader, Read, Take, Write};
use std::path::Path;

use super::super::{Offset, Span, TrackSeal};
use super::{GAP_THRESHOLD, PCM_FRAMES, SourceEpoch, Timing};

const HEADER: &[u8; 12] = b"KOETTJNL\x01\x01\x80\x3e";
const RECORD_HEADER: usize = 9;
const CHECKSUM: usize = 4;
const MAX_BODY: usize = RECORD_HEADER + PCM_FRAMES * 2 + CHECKSUM;
const EPOCH: u8 = 1;
const PCM: u8 = 2;
const GAP: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GapCause {
    ClockJump = 1,
    Overrun = 2,
    SourceRestart = 3,
    Unstamped = 4,
}

impl GapCause {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::ClockJump),
            2 => Some(Self::Overrun),
            3 => Some(Self::SourceRestart),
            4 => Some(Self::Unstamped),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TrackChunk {
    Pcm { first: Offset, samples: Vec<i16> },
    Silence(Span),
    Gap { span: Span, cause: GapCause },
}

pub(super) struct JournalWriter<W> {
    sink: W,
    buffer: Vec<u8>,
}

impl<W: Write> JournalWriter<W> {
    pub(super) fn new(mut sink: W) -> io::Result<Self> {
        sink.write_all(HEADER)?;
        Ok(Self {
            sink,
            buffer: Vec::with_capacity(4 + MAX_BODY),
        })
    }

    fn record(&mut self, kind: u8, first: Offset, payload_bytes: usize) {
        self.buffer.clear();
        let len = (RECORD_HEADER + payload_bytes + CHECKSUM) as u32;
        self.buffer.extend_from_slice(&len.to_le_bytes());
        self.buffer.push(kind);
        self.buffer.extend_from_slice(&first.0.to_le_bytes());
    }

    fn write_record(&mut self) -> io::Result<()> {
        let checksum = crc32fast::hash(&self.buffer[4..]);
        self.buffer.extend_from_slice(&checksum.to_le_bytes());
        self.sink.write_all(&self.buffer)
    }

    pub(super) fn epoch(&mut self, first: Offset, source: SourceEpoch) -> io::Result<()> {
        self.record(EPOCH, first, 9);
        self.buffer
            .extend_from_slice(&source.generation.to_le_bytes());
        self.buffer.push(match source.timing {
            Timing::Callback => 1,
            Timing::Arrival => 2,
        });
        self.write_record()
    }

    pub(super) fn pcm(&mut self, first: Offset, samples: &[f32]) -> io::Result<()> {
        self.record(PCM, first, samples.len() * 2);
        for &sample in samples {
            let sample = (sample * 32_768.0).round() as i16;
            self.buffer.extend_from_slice(&sample.to_le_bytes());
        }
        self.write_record()
    }

    pub(super) fn gap(&mut self, span: Span, cause: GapCause) -> io::Result<()> {
        self.record(GAP, span.first, 9);
        self.buffer
            .extend_from_slice(&(span.end.0 - span.first.0).to_le_bytes());
        self.buffer.push(cause as u8);
        self.write_record()
    }

    pub(super) fn sink(&self) -> &W {
        &self.sink
    }
}

enum Record {
    Epoch,
    Pcm { frames: u64 },
    Gap { span: u64, cause: GapCause },
}

pub(crate) struct TrackReader<R = BufReader<File>> {
    source: Option<Take<R>>,
    buffer: Vec<u8>,
    cutoff: Offset,
    previous_first: Offset,
    covered: Option<Offset>,
    seal: TrackSeal,
    pending: Option<TrackChunk>,
    seal_bytes: Option<u64>,
}

pub(in crate::meeting) fn has_record_bytes(path: &Path) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len() > HEADER.len() as u64),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

impl TrackReader {
    pub(crate) fn open(path: &Path, cutoff: Offset) -> io::Result<Self> {
        Self::from_reader(BufReader::new(File::open(path)?), cutoff, u64::MAX)
    }

    pub(crate) fn open_sealed(path: &Path, cutoff: Offset, seal: TrackSeal) -> io::Result<Self> {
        let mut reader =
            Self::from_reader(BufReader::new(File::open(path)?), cutoff, seal.valid_bytes)?;
        if reader.source.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid sealed track header",
            ));
        }
        reader.seal_bytes = Some(seal.valid_bytes);
        Ok(reader)
    }
}

impl<R: Read> TrackReader<R> {
    pub(super) fn from_reader(source: R, cutoff: Offset, limit: u64) -> io::Result<Self> {
        let mut source = source.take(limit);
        let mut header = [0; HEADER.len()];
        let valid = read_full(&mut source, &mut header)? && header == *HEADER;
        Ok(Self {
            source: valid.then_some(source),
            buffer: Vec::with_capacity(MAX_BODY),
            cutoff,
            previous_first: Offset(0),
            covered: None,
            seal: TrackSeal {
                valid_bytes: if valid { HEADER.len() as u64 } else { 0 },
                captured_until: Offset(0),
            },
            pending: None,
            seal_bytes: None,
        })
    }

    pub(crate) fn seal(mut self) -> io::Result<TrackSeal> {
        while let Some((first, record)) = self.read_record()? {
            if let Record::Pcm { frames } = record
                && first < self.cutoff
            {
                self.seal.captured_until = self
                    .seal
                    .captured_until
                    .max(Offset(first.0 + frames).min(self.cutoff));
            }
        }
        Ok(self.seal)
    }

    fn read_record(&mut self) -> io::Result<Option<(Offset, Record)>> {
        let Some(source) = &mut self.source else {
            return Ok(None);
        };
        let result = read_record(source, &mut self.buffer, self.previous_first);
        match result {
            Ok(Some((first, record))) => {
                self.previous_first = first;
                self.seal.valid_bytes += 4 + self.buffer.len() as u64;
                Ok(Some((first, record)))
            }
            ended => {
                self.source = None;
                if matches!(ended, Ok(None))
                    && self
                        .seal_bytes
                        .is_some_and(|limit| self.seal.valid_bytes != limit)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Truncated or corrupt sealed track",
                    ));
                }
                ended
            }
        }
    }

    fn next_chunk(&mut self) -> io::Result<Option<TrackChunk>> {
        if let Some(pending) = self.pending.take() {
            return Ok(Some(pending));
        }
        while self.previous_first < self.cutoff {
            let Some((first, record)) = self.read_record()? else {
                break;
            };
            if first >= self.cutoff {
                return Ok(None);
            }
            let covered = self.covered.unwrap_or(first);
            let chunk = match record {
                Record::Epoch => {
                    self.covered.get_or_insert(first);
                    continue;
                }
                Record::Pcm { frames } => {
                    let end = Offset(first.0 + frames).min(self.cutoff);
                    self.seal.captured_until = self.seal.captured_until.max(end);
                    let start = covered.max(first);
                    if end <= start {
                        continue;
                    }
                    self.covered = Some(end);
                    let start_byte = RECORD_HEADER + (start.0 - first.0) as usize * 2;
                    let end_byte = RECORD_HEADER + (end.0 - first.0) as usize * 2;
                    let samples = self.buffer[start_byte..end_byte]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|bytes| i16::from_le_bytes(*bytes))
                        .collect();
                    TrackChunk::Pcm {
                        first: start,
                        samples,
                    }
                }
                Record::Gap { span, cause } => {
                    let end = Offset(first.0 + span).min(self.cutoff);
                    self.covered = Some(covered.max(end));
                    TrackChunk::Gap {
                        span: Span { first, end },
                        cause,
                    }
                }
            };
            if covered < first {
                self.pending = Some(chunk);
                let span = Span {
                    first: covered,
                    end: first,
                };
                return Ok(Some(if first.0 - covered.0 < GAP_THRESHOLD {
                    TrackChunk::Silence(span)
                } else {
                    TrackChunk::Gap {
                        span,
                        cause: GapCause::Unstamped,
                    }
                }));
            }
            return Ok(Some(chunk));
        }
        Ok(None)
    }
}

impl<R: Read> Iterator for TrackReader<R> {
    type Item = io::Result<TrackChunk>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_chunk().transpose()
    }
}

fn read_record(
    source: &mut impl Read,
    buffer: &mut Vec<u8>,
    previous_first: Offset,
) -> io::Result<Option<(Offset, Record)>> {
    let mut len = [0; 4];
    if !read_full(source, &mut len)? {
        return Ok(None);
    }
    let len = u32::from_le_bytes(len) as usize;
    if !(RECORD_HEADER + CHECKSUM..=MAX_BODY).contains(&len) {
        return Ok(None);
    }

    buffer.resize(len, 0);
    if !read_full(source, buffer)? {
        return Ok(None);
    }
    let (contents, checksum) = buffer.split_at(len - CHECKSUM);
    if crc32fast::hash(contents).to_le_bytes() != checksum {
        return Ok(None);
    }
    Ok(parse_record(contents, previous_first))
}

fn parse_record(contents: &[u8], previous_first: Offset) -> Option<(Offset, Record)> {
    let (&kind, body) = contents.split_first()?;
    let (&first_bytes, payload) = body.split_first_chunk::<8>()?;
    let first = Offset(u64::from_le_bytes(first_bytes));
    if first < previous_first {
        return None;
    }

    let record = match (kind, payload) {
        (EPOCH, [_, _, _, _, _, _, _, _, 1 | 2]) => Record::Epoch,
        (PCM, payload) if !payload.is_empty() && payload.len().is_multiple_of(2) => {
            let frames = (payload.len() / 2) as u64;
            first.0.checked_add(frames)?;
            Record::Pcm { frames }
        }
        (GAP, payload) => {
            let (&length, [cause]) = payload.split_first_chunk::<8>()? else {
                return None;
            };
            let span = u64::from_le_bytes(length);
            if span == 0 {
                return None;
            }
            first.0.checked_add(span)?;
            Record::Gap {
                span,
                cause: GapCause::from_byte(*cause)?,
            }
        }
        _ => return None,
    };
    Some((first, record))
}

fn read_full(source: &mut impl Read, bytes: &mut [u8]) -> io::Result<bool> {
    match source.read_exact(bytes) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

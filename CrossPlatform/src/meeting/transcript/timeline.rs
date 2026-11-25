use super::super::bundle::marks::MarkRef;
use super::super::{Channel, Offset, SAMPLE_RATE, Span};

const MAX_PAUSE: u64 = SAMPLE_RATE as u64 * 3 / 2;
const MAX_TURN: u64 = SAMPLE_RATE as u64 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OtherVoice {
    Unassigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Speaker {
    You,
    Other(OtherVoice),
}

impl From<Channel> for Speaker {
    fn from(channel: Channel) -> Self {
        match channel {
            Channel::You => Self::You,
            Channel::Other => Self::Other(OtherVoice::Unassigned),
        }
    }
}

pub(super) struct SpeechWord {
    pub(super) channel: Channel,
    pub(super) span: Span,
    pub(super) text: String,
    pub(super) backend: String,
    pub(super) lang: String,
}

pub(super) struct Turn {
    pub(super) speaker: Speaker,
    pub(super) span: Span,
    pub(super) words: Vec<SpeechWord>,
    pub(super) backend: String,
    pub(super) lang: String,
}

impl Turn {
    fn new(word: SpeechWord) -> Self {
        Self {
            speaker: word.channel.into(),
            span: word.span,
            backend: word.backend.clone(),
            lang: word.lang.clone(),
            words: vec![word],
        }
    }

    fn accepts(&self, word: &SpeechWord) -> bool {
        self.speaker == word.channel.into()
            && self.backend == word.backend
            && self.lang == word.lang
            && word.span.first.0.saturating_sub(self.span.end.0) <= MAX_PAUSE
            && word.span.end.0.max(self.span.end.0) - self.span.first.0 <= MAX_TURN
    }
}

pub(super) enum Point {
    MissingAudio { channel: Channel, span: Span },
    Mark(MarkRef),
    Word(SpeechWord),
}

impl Point {
    fn sort_key(&self) -> (Offset, u8, Option<Channel>) {
        match self {
            Self::MissingAudio { channel, span } => (span.first, 0, Some(*channel)),
            Self::Mark(mark) => (mark.offset, 1, None),
            Self::Word(word) => (word.span.first, 2, Some(word.channel)),
        }
    }
}

pub(super) enum Entry {
    Turn(Turn),
    Mark(MarkRef),
    MissingAudio { channel: Channel, span: Span },
}

pub(super) fn merge(mut points: Vec<Point>) -> Vec<Entry> {
    points.sort_by_key(Point::sort_key);
    let mut entries = Vec::new();
    for point in points {
        let entry = match point {
            Point::Mark(mark) => Entry::Mark(mark),
            Point::MissingAudio { channel, span } => Entry::MissingAudio { channel, span },
            Point::Word(word) => {
                if let Some(Entry::Turn(turn)) = entries.last_mut()
                    && turn.accepts(&word)
                {
                    turn.span.end = turn.span.end.max(word.span.end);
                    turn.words.push(word);
                    continue;
                }
                Entry::Turn(Turn::new(word))
            }
        };
        entries.push(entry);
    }
    entries
}

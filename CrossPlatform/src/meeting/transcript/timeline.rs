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
        self.backend == word.backend
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

impl Entry {
    fn sort_key(&self) -> (Offset, u8, Option<Channel>) {
        match self {
            Self::MissingAudio { channel, span } => (span.first, 0, Some(*channel)),
            Self::Mark(mark) => (mark.offset, 1, None),
            Self::Turn(turn) => (turn.span.first, 2, Some(turn.words[0].channel)),
        }
    }
}

#[derive(Default)]
struct Side {
    open: Option<Turn>,
    reply_end: Option<Offset>,
}

impl Side {
    fn close(&mut self, entries: &mut Vec<Entry>) {
        self.reply_end = None;
        if let Some(turn) = self.open.take() {
            entries.push(Entry::Turn(turn));
        }
    }

    fn hear(&mut self, word: &SpeechWord) {
        if self
            .open
            .as_ref()
            .is_some_and(|turn| word.span.first >= turn.span.end)
        {
            let end = word.span.end;
            self.reply_end = Some(self.reply_end.map_or(end, |earlier| earlier.min(end)));
        }
    }

    fn speak(&mut self, word: SpeechWord, entries: &mut Vec<Entry>) {
        let reply_finished = self
            .reply_end
            .take()
            .is_some_and(|end| end <= word.span.first);
        match self.open.as_mut() {
            Some(turn) if !reply_finished && turn.accepts(&word) => {
                turn.span.end = turn.span.end.max(word.span.end);
                turn.words.push(word);
            }
            _ => {
                self.close(entries);
                self.open = Some(Turn::new(word));
            }
        }
    }
}

pub(super) fn merge(mut points: Vec<Point>) -> Vec<Entry> {
    points.sort_by_key(Point::sort_key);
    let mut entries = Vec::new();
    let [mut you, mut other] = [Side::default(), Side::default()];
    for point in points {
        match point {
            Point::Mark(mark) => {
                you.close(&mut entries);
                other.close(&mut entries);
                entries.push(Entry::Mark(mark));
            }
            Point::MissingAudio { channel, span } => {
                match channel {
                    Channel::You => you.close(&mut entries),
                    Channel::Other => other.close(&mut entries),
                }
                entries.push(Entry::MissingAudio { channel, span });
            }
            Point::Word(word) => {
                let (mine, theirs) = match word.channel {
                    Channel::You => (&mut you, &mut other),
                    Channel::Other => (&mut other, &mut you),
                };
                theirs.hear(&word);
                mine.speak(word, &mut entries);
            }
        }
    }
    you.close(&mut entries);
    other.close(&mut entries);
    entries.sort_by_key(Entry::sort_key);
    entries
}

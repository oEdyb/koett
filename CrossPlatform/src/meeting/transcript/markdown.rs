use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

use crate::dictionary::{self, DictionaryEntry};

use super::super::bundle::records::{HistoryReceipt, Manifest, Publication};
use super::super::bundle::sha256;
use super::super::{Offset, SAMPLE_RATE};
use super::timeline::{Entry, Speaker};

pub(in crate::meeting) struct Rendered {
    pub(in crate::meeting) markdown: String,
    pub(in crate::meeting) publication: Publication,
}

pub(super) fn render(
    manifest: &Manifest,
    date: OffsetDateTime,
    cutoff: Offset,
    entries: Vec<Entry>,
    dictionary: &[DictionaryEntry],
    fallback: bool,
) -> Result<Rendered, String> {
    let iso = date.format(&Rfc3339).map_err(|err| err.to_string())?;
    let title = date
        .format(format_description!("[year]-[month]-[day] [hour]:[minute]"))
        .map_err(|err| err.to_string())?;
    let mut backends = BTreeMap::new();
    let mut assets = Vec::new();
    let mut body = format!("# Meeting · {title}\n");
    for entry in entries {
        match entry {
            Entry::Turn(turn) => {
                let start = timestamp(turn.span.first);
                let speaker = speaker_label(turn.speaker);
                let words = turn
                    .words
                    .into_iter()
                    .map(|word| word.text)
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = dictionary::apply(&words, dictionary)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace('&', "&amp;")
                    .replace('<', "&lt;");
                let _ = writeln!(
                    body,
                    "\n[{start}] **{speaker}:** {text} <!-- {} {} -->",
                    turn.lang, turn.backend
                );
                *backends.entry(turn.backend).or_insert(0) += 1;
            }
            Entry::Mark(mark) => {
                let _ = writeln!(
                    body,
                    "\n[{}] ![Mark {}]({})",
                    timestamp(mark.offset),
                    assets.len() + 1,
                    mark.relative_path
                );
                assets.push(mark.relative_path);
            }
            Entry::MissingAudio { channel, span }
                if span.end.0 - span.first.0 >= 5 * u64::from(SAMPLE_RATE) =>
            {
                let _ = writeln!(
                    body,
                    "\n*[{}–{}] No {} audio*",
                    timestamp(span.first),
                    timestamp(span.end),
                    speaker_label(channel.into())
                );
            }
            Entry::MissingAudio { .. } => {}
        }
    }
    let duration = timestamp(cutoff);
    let counts = backends
        .iter()
        .map(|(name, count)| format!("{name}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let id = manifest.info.id;
    let markdown = format!(
        "---\ntype: meeting-transcript\nkoett_id: {id}\ndate: {iso}\nduration: \"{duration}\"\nasr: {{{counts}}}\n---\n\n{body}"
    );
    Ok(Rendered {
        publication: Publication {
            history: HistoryReceipt {
                id: format!("meeting:{id}"),
                created_at_ms: manifest.info.started_at_ms,
                model: backends.keys().cloned().collect::<Vec<_>>().join("+"),
                text_sha256: sha256(strip_comments(&body).as_bytes()),
            },
            assets,
            backends,
            fallback,
        },
        markdown,
    })
}

fn timestamp(time: Offset) -> String {
    let seconds = time.0 / u64::from(SAMPLE_RATE);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn speaker_label(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::You => "You",
        Speaker::Other(_) => "Other",
    }
}

fn frontmatter_and_body(markdown: &str) -> Result<(&str, &str), String> {
    markdown
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n\n"))
        .ok_or_else(|| "Meeting transcript frontmatter is missing".into())
}

fn frontmatter_field<'a>(frontmatter: &'a str, key: &str) -> Option<&'a str> {
    let mut values = frontmatter.lines().filter_map(|line| {
        let (field, value) = line.split_once(':')?;
        (field == key).then(|| value.trim())
    });
    values.next().filter(|_| values.next().is_none())
}

fn strip_comments(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    for line in body.lines() {
        let line = line
            .rsplit_once(" <!-- ")
            .filter(|(_, comment)| comment.ends_with(" -->"))
            .map_or(line, |(text, _)| text);
        let _ = writeln!(result, "{line}");
    }
    result
}

pub(in crate::meeting) fn history_text(markdown: &str) -> Result<String, String> {
    let (_, body) = frontmatter_and_body(markdown)?;
    Ok(strip_comments(body))
}

pub(in crate::meeting) fn vault_note(
    manifest: &Manifest,
    cutoff: Offset,
    publication: &Publication,
    markdown: &str,
    bundle_path: &Path,
) -> Result<(String, String), String> {
    let (frontmatter, body) = frontmatter_and_body(markdown)?;
    let date =
        frontmatter_field(frontmatter, "date").ok_or("Meeting transcript date is missing")?;
    let date = OffsetDateTime::parse(date, &Rfc3339).map_err(|err| err.to_string())?;
    let day = date.date().to_string();
    let minute = date
        .format(format_description!("[hour][minute]"))
        .map_err(|err| err.to_string())?;
    let id = manifest.info.id.to_string();
    let title = format!("{day} {minute} Koett meeting {}", &id[..8]);
    let model = if publication.history.model.is_empty() {
        "none"
    } else {
        &publication.history.model
    };
    let summary = format!(
        "{day} meeting, duration {}, tracks You and Other, {} marks, ASR backends: {model}.",
        timestamp(cutoff),
        publication.assets.len()
    );
    let mut note = format!(
        "---\ntype: meeting\nkoett_id: {id}\ntitle: {title:?}\nsummary: {summary:?}\ndate: {day}\nstatus: complete\ntags: [meeting, koett]\n---\n\n"
    );
    for line in body.lines() {
        if let Some((prefix, relative)) = line
            .split_once("] ![Mark ")
            .and_then(|(time, mark)| {
                mark.split_once("](")
                    .map(|(number, path)| ((time, number), path))
            })
            .and_then(|(prefix, relative)| relative.strip_suffix(')').map(|path| (prefix, path)))
            && publication.assets.iter().any(|asset| asset == relative)
        {
            let (time, number) = prefix;
            let path = url::Url::from_file_path(bundle_path.join(relative))
                .map_err(|()| "Mark path is not an absolute file path")?;
            let _ = writeln!(note, "{time}] ![Mark {number}](<{path}>)");
        } else {
            let _ = writeln!(note, "{line}");
        }
    }
    Ok((format!("{title}.md"), note))
}

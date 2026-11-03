//! What a pin read at the click: the window, the page, the thing under the
//! pointer and the selection. Formats the lines pasted under the pin's bullet,
//! in the same format as the Mac's `PinContext.swift`. Every field is optional
//! because any app may expose less.

pub const TITLE_LIMIT: usize = 120;
pub const SELECTION_LIMIT: usize = 500;
pub const URL_LIMIT: usize = 2_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PinContext {
    pub app: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub link: Option<String>,
    pub image: Option<String>,
    pub selection: Option<String>,
}

impl PinContext {
    /// `App · “Title” · URL · link … · image …`, then `selected: “…”`.
    pub fn lines(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(app) = clean(self.app.as_deref(), TITLE_LIMIT) {
            parts.push(app);
        }
        if let Some(title) = clean(self.title.as_deref(), TITLE_LIMIT) {
            parts.push(format!("“{title}”"));
        }
        let page = clean(self.url.as_deref(), URL_LIMIT);
        if let Some(page) = &page {
            parts.push(page.clone());
        }
        if let Some(link) = clean(self.link.as_deref(), URL_LIMIT)
            && Some(&link) != page.as_ref()
            && Some(link.as_str()) != self.url.as_deref()
        {
            parts.push(format!("link {link}"));
        }
        if let Some(image) = clean(self.image.as_deref(), URL_LIMIT) {
            parts.push(if image.contains("://") {
                format!("image {image}")
            } else {
                format!("image “{image}”")
            });
        }
        let mut lines = Vec::new();
        if !parts.is_empty() {
            lines.push(parts.join(" · "));
        }
        if let Some(selection) = clean(self.selection.as_deref(), SELECTION_LIMIT) {
            lines.push(format!("selected: “{selection}”"));
        }
        lines
    }
}

fn is_newline(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// One line, trimmed, capped with an ellipsis; `None` when empty.
pub fn clean(text: Option<&str>, limit: usize) -> Option<String> {
    let line = text?
        .split(is_newline)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if line.chars().count() > limit {
        let mut capped: String = line.chars().take(limit).collect();
        capped.push('…');
        return Some(capped);
    }
    Some(line.to_owned())
}

/// Puts each pin's context lines, indented, under its bullet. Bullets are one
/// line each (`annotation::keyframe_summary`), in the frames' order; a summary
/// that does not line up with the contexts is returned unchanged.
pub fn annotate(summary: &str, contexts: &[Option<PinContext>]) -> String {
    if contexts.iter().all(Option::is_none) {
        return summary.to_owned();
    }
    let bullets: Vec<&str> = summary.split('\n').collect();
    if bullets.len() != contexts.len() {
        return summary.to_owned();
    }
    bullets
        .iter()
        .zip(contexts)
        .map(|(bullet, context)| {
            let mut lines = vec![(*bullet).to_owned()];
            if let Some(context) = context {
                lines.extend(context.lines().iter().map(|line| format!("  {line}")));
            }
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> PinContext {
        PinContext {
            app: Some(name.to_owned()),
            ..PinContext::default()
        }
    }

    #[test]
    fn context_line_joins_window_page_and_selection() {
        let context = PinContext {
            app: Some("Google Chrome".into()),
            title: Some("Pull Request #22".into()),
            url: Some("https://github.com/oEdyb/koett/pull/22".into()),
            link: Some("https://github.com/oEdyb/koett/pull/22".into()),
            image: Some("https://example.com/pill.png".into()),
            selection: Some("one fixed\n48 px row".into()),
        };
        assert_eq!(
            context.lines(),
            [
                "Google Chrome · “Pull Request #22” · https://github.com/oEdyb/koett/pull/22 · image https://example.com/pill.png",
                "selected: “one fixed 48 px row”",
            ]
        );
    }

    #[test]
    fn a_different_link_and_a_described_image_are_labelled() {
        let context = PinContext {
            url: Some("https://a.example/".into()),
            link: Some("https://b.example/".into()),
            image: Some("A red pill".into()),
            ..PinContext::default()
        };
        assert_eq!(
            context.lines(),
            ["https://a.example/ · link https://b.example/ · image “A red pill”"]
        );
    }

    #[test]
    fn selection_is_capped() {
        let context = PinContext {
            selection: Some("a".repeat(600)),
            ..PinContext::default()
        };
        let line = context.lines().remove(0);
        assert_eq!(
            line.chars().count(),
            "selected: “”".chars().count() + SELECTION_LIMIT + 1
        );
    }

    #[test]
    fn clean_rejects_blank_text() {
        assert_eq!(clean(Some(" \n\n "), 10), None);
        assert_eq!(clean(None, 10), None);
    }

    #[test]
    fn annotate_puts_context_under_its_bullet() {
        let summary = "- #1 (0:02) “this video”\n- #2 (0:09) “circle this”";
        assert_eq!(
            annotate(summary, &[Some(app("Safari")), None]),
            "- #1 (0:02) “this video”\n  Safari\n- #2 (0:09) “circle this”"
        );
    }

    #[test]
    fn annotate_leaves_the_summary_alone_without_context() {
        let summary = "- #1 (0:02) “this video”\n- #2 (0:09) “circle this”";
        assert_eq!(annotate(summary, &[None, None]), summary);
    }

    #[test]
    fn annotate_leaves_a_mismatched_summary_alone() {
        let summary = "- #1 (0:02) “this video”\n- #2 (0:09) “circle this”";
        assert_eq!(annotate(summary, &[Some(app("Safari"))]), summary);
    }
}

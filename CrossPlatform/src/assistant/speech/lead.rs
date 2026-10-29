#[derive(Default)]
pub struct Lead {
    pending: String,
    count: usize,
    stopped: bool,
}
impl Lead {
    pub fn append(&mut self, delta: &str, finish: bool) -> Vec<String> {
        if self.stopped {
            return vec![];
        }
        if self.pending.len() + delta.len() > 8192 {
            self.stopped = true;
            return vec![];
        }
        self.pending.push_str(delta);
        let boundary = ["\n\n", "\r\n\r\n", "```"]
            .iter()
            .filter_map(|s| self.pending.find(s))
            .min();
        if let Some(n) = boundary {
            self.pending.truncate(n);
            self.stopped = true;
        }
        let mut out = vec![];
        loop {
            let end = self
                .pending
                .char_indices()
                .find_map(|(i, c)| {
                    let next = i + c.len_utf8();
                    (".!?".contains(c) && self.pending[next..].starts_with(char::is_whitespace))
                        .then_some(next)
                })
                .or_else(|| (finish || self.stopped).then_some(self.pending.len()));
            let Some(end) = end else {
                break;
            };
            if end == 0 {
                break;
            }
            let source: String = self.pending.drain(..end).collect();
            let text = clean(&source);
            if !text.is_empty() {
                out.push(text);
                self.count += 1;
            }
            if self.count == 3 {
                self.stopped = true;
                self.pending.clear();
                break;
            }
        }
        out
    }
}
fn clean(source: &str) -> String {
    let mut text = String::new();
    let mut link = false;
    let mut url = false;
    for word in source.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            continue;
        }
        for c in word.chars() {
            if c == ']' {
                link = true;
                continue;
            }
            if link {
                if c == '(' {
                    url = true;
                    continue;
                }
                if url {
                    if c == ')' {
                        link = false;
                        url = false;
                    }
                    continue;
                }
                link = false;
            }
            if !"[*_~`$#>".contains(c) {
                text.push(c);
            }
        }
        text.push(' ');
    }
    text.trim().to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_boundary_and_limit() {
        let mut lead = Lead::default();
        assert!(lead.append("**Hello", false).is_empty());
        assert_eq!(
            lead.append(" world. Next! Third? Fourth.\n\nDetails", false),
            vec!["Hello world.", "Next!", "Third?"]
        );
        assert!(lead.append("more", true).is_empty());
    }
    #[test]
    fn strips_links_and_code() {
        let mut lead = Lead::default();
        assert_eq!(
            lead.append(
                "See [docs](https://example.com) https://example.com now\n\n```bad",
                false
            ),
            vec!["See docs now"]
        );
    }
}

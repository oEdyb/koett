use crate::pin_context::URL_LIMIT;
pub(super) fn private_indicator(name: &str) -> bool {
    ["InPrivate", "Incognito", "Private browsing"]
        .iter()
        .any(|label| {
            name.strip_prefix(label)
                .is_some_and(|suffix| suffix.chars().next().is_none_or(|c| !c.is_alphanumeric()))
        })
}
pub(super) fn web_url(raw: &str) -> Option<String> {
    if raw.chars().count() > URL_LIMIT {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    (url.as_str().chars().count() <= URL_LIMIT).then(|| url.into())
}
pub(super) fn youtube_time(raw: &str, seconds: f64) -> Option<String> {
    if !seconds.is_finite() || !(0.0..=604800.0).contains(&seconds) {
        return None;
    }
    let mut url = url::Url::parse(raw).ok()?;
    if !matches!(
        url.host_str(),
        Some("youtube.com" | "www.youtube.com" | "m.youtube.com" | "youtu.be")
    ) || !(url.path() == "/watch" && url.query_pairs().any(|(k, v)| k == "v" && !v.is_empty())
        || url
            .path()
            .strip_prefix("/shorts/")
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
        || url.host_str() == Some("youtu.be") && url.path().len() > 1)
    {
        return None;
    }
    let pairs = url
        .query_pairs()
        .filter(|(k, _)| k != "t" && k != "start")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(pairs)
        .append_pair("t", &format!("{}s", seconds.floor() as u64));
    url.set_fragment(None);
    Some(url.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_private_labels_allow_descriptions_and_counts_at_word_boundaries() {
        for name in [
            "InPrivate",
            "InPrivate browsing",
            "InPrivate (2)",
            "Incognito, 2 tabs",
            "Private browsing",
        ] {
            assert!(private_indicator(name), "{name}");
        }
        for name in [
            "Not InPrivate",
            "InPrivateX",
            "Private browsingPublic",
            "Profile 1",
        ] {
            assert!(!private_indicator(name), "{name}");
        }
    }
    #[test]
    fn urls_are_explicit_web_values_and_video_times_cannot_be_guessed() {
        assert!(web_url("Settings - https://example.com").is_none());
        assert!(web_url("file:///private/file").is_none());
        assert!(web_url("https://name:secret@example.com").is_none());
        assert_eq!(
            web_url("https://example.com/page").as_deref(),
            Some("https://example.com/page")
        );
        assert_eq!(
            youtube_time("https://www.youtube.com/watch?v=abc&t=3", 83.9).as_deref(),
            Some("https://www.youtube.com/watch?v=abc&t=83s")
        );
        assert!(youtube_time("https://youtube.com.evil/watch?v=abc", 83.0).is_none());
        assert!(youtube_time("https://youtube.com/watch?v=abc", f64::NAN).is_none());
    }
}

pub(super) fn video_seconds(properties: &str) -> Option<f64> {
    let value = properties
        .split(';')
        .find_map(|property| property.trim().strip_prefix("valuetext="))?;
    let (current, duration) = value
        .split_once(" of ")
        .or_else(|| value.split_once(" / "))?;
    fn clock(raw: &str) -> Option<u64> {
        let parts = raw.trim().split(':').collect::<Vec<_>>();
        if !(2..=3).contains(&parts.len()) {
            return None;
        }
        let mut seconds = 0_u64;
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let number = part.parse::<u64>().ok()?;
            if index > 0 && number >= 60 {
                return None;
            }
            seconds = seconds.checked_mul(60)?.checked_add(number)?;
        }
        (seconds <= 604800).then_some(seconds)
    }
    let (current, duration) = (clock(current)?, clock(duration)?);
    (duration > 0 && current <= duration).then_some(current as f64)
}
#[cfg(test)]
mod video_tests {
    use super::*;
    #[test]
    fn seek_percentage_is_never_fabricated_into_seconds() {
        assert_eq!(
            video_seconds("valuenow=39;valuemax=100;valuetext=1:23 of 4:56"),
            Some(83.0)
        );
        assert_eq!(video_seconds("valuetext=0:00 / 4:56"), Some(0.0));
        for text in [
            "valuenow=39;valuemax=100",
            "valuetext=39",
            "valuetext=1:99 of 4:56",
            "valuetext=9:00 of 4:56",
            "valuetext=0:00 of 0:00",
        ] {
            assert_eq!(video_seconds(text), None);
        }
    }
}

use std::collections::HashMap;

pub fn is_youtube_video(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    match url.host_str() {
        Some("youtube.com" | "www.youtube.com" | "m.youtube.com") => {
            (url.path() == "/watch" && url.query_pairs().any(|(k, v)| k == "v" && !v.is_empty()))
                || url
                    .path()
                    .strip_prefix("/shorts/")
                    .is_some_and(|id| !id.is_empty())
        }
        Some("youtu.be") => url.path().len() > 1,
        _ => false,
    }
}

pub fn is_player(attributes: &HashMap<String, String>) -> bool {
    attributes.iter().any(|(key, value)| {
        matches!(key.as_str(), "class" | "class-name")
            && value
                .split_whitespace()
                .any(|class| class == "ytp-progress-bar")
    })
}

pub fn timestamp(raw: &str, seconds: f64) -> Option<String> {
    if !is_youtube_video(raw) || !seconds.is_finite() || !(0.0..=604800.0).contains(&seconds) {
        return None;
    }
    let mut url = url::Url::parse(raw).ok()?;
    let retained: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key != "t" && key != "start")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(retained)
        .append_pair("t", &format!("{}s", seconds.floor() as u64));
    url.set_fragment(None);
    Some(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_only_matched_video_urls_and_preserves_identity() {
        assert_eq!(
            timestamp(
                "https://www.youtube.com/watch?v=abc&t=9&start=2&list=PL",
                83.9
            )
            .as_deref(),
            Some("https://www.youtube.com/watch?v=abc&list=PL&t=83s")
        );
        assert_eq!(
            timestamp("https://youtu.be/abc", 0.0).as_deref(),
            Some("https://youtu.be/abc?t=0s")
        );
        for raw in [
            "https://youtube.com.evil/watch?v=a",
            "https://youtube.com/",
            "https://example.com/watch?v=a",
        ] {
            assert!(timestamp(raw, 20.0).is_none());
        }
        for seconds in [f64::NAN, f64::INFINITY, -1.0, 604801.0] {
            assert!(timestamp("https://youtube.com/watch?v=a", seconds).is_none());
        }
    }
    #[test]
    fn volume_and_other_sliders_are_never_time() {
        assert!(is_player(&HashMap::from([(
            "class".into(),
            "ytp-progress-bar ytp-a11y-slider".into()
        )])));
        assert!(!is_player(&HashMap::from([(
            "class".into(),
            "ytp-volume-slider".into()
        )])));
        assert!(!is_player(&HashMap::from([(
            "class".into(),
            "not-ytp-progress-bar".into()
        )])));
    }
}

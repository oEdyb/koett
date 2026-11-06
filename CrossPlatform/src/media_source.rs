pub fn validate_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "Enter an HTTP(S) URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.chars().any(char::is_control)
        || value.len() > 7000
        || !value.contains("://")
        || value.split_once("://").is_some_and(|(_, rest)| {
            rest.split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
    {
        return Err("Enter an HTTP(S) URL without credentials".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_http_urls_without_credentials_are_accepted() {
        for value in [
            "file:///C:/private",
            "https://u:p@example.com",
            "https://@example.com",
            "https://example.com/\n",
            "--exec=bad",
            "https:example.com",
            "ftp://example.com",
        ] {
            assert!(validate_url(value).is_err(), "{value:?}");
        }
        for value in [
            "https://example.com/å?q=ö",
            "http://127.0.0.1:18767/fixture.wav",
        ] {
            assert!(validate_url(value).is_ok());
        }
        assert!(validate_url(&format!("https://example.com/{}", "x".repeat(7000))).is_err());
    }
}

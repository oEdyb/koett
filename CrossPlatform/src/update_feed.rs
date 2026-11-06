//! The Linux update feed: one JSON file per architecture, beside each release. The
//! signature covers everything that decides what gets installed, so a feed can't offer
//! other bytes, another architecture or a different build number than was signed.
use base64::{Engine, engine::general_purpose::STANDARD};

const SCHEMA: u32 = 1;
const DOMAIN: &str = "koett-linux-update-v1";
pub const MAX_FEED_BYTES: u64 = 64 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    pub schema: u32,
    pub version: String,
    pub build: u64,
    pub arch: String,
    pub url: String,
    pub length: u64,
    pub sha256: String,
    pub signature: String,
}

impl Feed {
    /// The exact bytes the Ed25519 signature covers.
    pub fn signed_message(&self) -> String {
        format!(
            "{DOMAIN}\n{}\n{}\n{}\n{}\n{}\n",
            self.version, self.build, self.arch, self.length, self.sha256
        )
    }

    /// Parses and checks a feed, signature first. Nothing is downloaded before this.
    pub fn verified(
        raw: &[u8],
        public_key: &[u8; 32],
        allow_loopback: bool,
    ) -> Result<Self, String> {
        let feed: Self =
            serde_json::from_slice(raw).map_err(|_| "The update feed is not valid".to_string())?;
        let signature = STANDARD
            .decode(&feed.signature)
            .map_err(|_| "The update signature is not valid".to_string())?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
            .verify(feed.signed_message().as_bytes(), &signature)
            .map_err(|_| "The update signature is not valid".to_string())?;
        if feed.schema != SCHEMA {
            return Err("The update feed has an unknown format".into());
        }
        if feed.arch != std::env::consts::ARCH {
            return Err("The update is for another architecture".into());
        }
        if feed.length == 0 || feed.length > MAX_ARCHIVE_BYTES {
            return Err("The update has an invalid size".into());
        }
        if feed.sha256.len() != 64 || !feed.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("The update has an invalid checksum".into());
        }
        if !accepts_url(&feed.url, allow_loopback) {
            return Err("The update address is not allowed".into());
        }
        Ok(feed)
    }
}

/// HTTPS only. A lab build may also use plain HTTP to its own machine.
pub fn accepts_url(raw: &str, allow_loopback: bool) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => url.host_str().is_some_and(|host| !host.is_empty()),
        "http" => {
            allow_loopback
                && matches!(
                    url.host(),
                    Some(url::Host::Ipv4(ip)) if ip.is_loopback()
                )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn key() -> (Ed25519KeyPair, [u8; 32]) {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let public = pair.public_key().as_ref().try_into().unwrap();
        (pair, public)
    }

    fn feed(pair: &Ed25519KeyPair) -> Feed {
        let mut feed = Feed {
            schema: SCHEMA,
            version: "0.3.2".into(),
            build: 7,
            arch: std::env::consts::ARCH.into(),
            url: "https://github.com/oEdyb/koett/releases/download/v0.3.2/koett.tar.gz".into(),
            length: 10,
            sha256: "a".repeat(64),
            signature: String::new(),
        };
        feed.signature = STANDARD.encode(pair.sign(feed.signed_message().as_bytes()));
        feed
    }

    fn raw(feed: &Feed) -> Vec<u8> {
        serde_json::to_vec(feed).unwrap()
    }

    #[test]
    fn a_signed_feed_is_accepted() {
        let (pair, public) = key();
        let signed = feed(&pair);
        assert_eq!(
            Feed::verified(&raw(&signed), &public, false).unwrap(),
            signed
        );
    }

    #[test]
    fn another_key_or_a_changed_field_is_refused() {
        let (pair, public) = key();
        let (other, _) = key();
        let error = "The update signature is not valid";
        assert_eq!(
            Feed::verified(&raw(&feed(&other)), &public, false).unwrap_err(),
            error
        );
        for change in [
            (|feed: &mut Feed| feed.build = 8) as fn(&mut Feed),
            |feed| feed.version = "9.9.9".into(),
            |feed| feed.length = 11,
            |feed| feed.sha256 = "b".repeat(64),
            |feed| feed.arch = "other".into(),
            |feed| feed.signature = "not base64".into(),
        ] {
            let mut changed = feed(&pair);
            change(&mut changed);
            assert_eq!(
                Feed::verified(&raw(&changed), &public, false).unwrap_err(),
                error
            );
        }
    }

    #[test]
    fn a_signed_feed_still_needs_this_architecture_and_a_safe_address() {
        let (pair, public) = key();
        let sign = |mut feed: Feed| {
            feed.signature = STANDARD.encode(pair.sign(feed.signed_message().as_bytes()));
            feed
        };
        let mut other = feed(&pair);
        other.arch = "other".into();
        assert!(Feed::verified(&raw(&sign(other)), &public, false).is_err());
        let mut http = feed(&pair);
        http.url = "http://127.0.0.1:1/koett.tar.gz".into();
        let http = sign(http);
        assert!(Feed::verified(&raw(&http), &public, false).is_err());
        assert!(Feed::verified(&raw(&http), &public, true).is_ok());
    }

    #[test]
    fn addresses_are_https_except_a_lab_builds_own_machine() {
        for (url, loopback, accepted) in [
            ("https://github.com/a", false, true),
            ("http://github.com/a", false, false),
            ("http://github.com/a", true, false),
            ("http://127.0.0.1:9/a", false, false),
            ("http://127.0.0.1:9/a", true, true),
            ("https://user:pass@github.com/a", false, false),
            ("https://github.com/a#b", false, false),
            ("file:///etc/passwd", true, false),
            ("not a url", true, false),
        ] {
            assert_eq!(accepts_url(url, loopback), accepted, "{url}");
        }
    }
}

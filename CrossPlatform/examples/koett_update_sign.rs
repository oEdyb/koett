//! Creates the Linux update signing key and signs a release archive. The private key
//! never leaves its file: it must be mode 600 and stay out of the repository.
//!   koett_update_sign keygen KEY_FILE
//!   koett_update_sign public KEY_FILE
//!   koett_update_sign sign KEY_FILE ARCHIVE VERSION BUILD URL VALID_DAYS [ARCH] > appcast-linux-ARCH.json
#[cfg(target_os = "linux")]
fn main() -> Result<(), String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use koett_engine::update_feed::Feed;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::Path;

    let expires = |days: &str| -> Result<u64, String> {
        let days: u64 = days
            .parse()
            .ok()
            .filter(|days| (1..=366).contains(days))
            .ok_or("VALID_DAYS must be 1 to 366")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?;
        Ok(now.as_secs() + days * 24 * 60 * 60)
    };

    fn pair(path: &Path) -> Result<Ed25519KeyPair, String> {
        let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
        if metadata.mode() & 0o077 != 0 {
            return Err(format!("{} must be mode 600", path.display()));
        }
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        Ed25519KeyPair::from_pkcs8(&bytes).map_err(|_| "not an Ed25519 key".to_string())
    }

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match arguments.as_slice() {
        ["keygen", path] => {
            let path = Path::new(path);
            if let Some(parent) = path.parent() {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)
                    .map_err(|error| error.to_string())?;
            }
            let document = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
                .map_err(|_| "could not create a key".to_string())?;
            // Never replaces an existing key: rotating it would strand every installed copy.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            file.write_all(document.as_ref()).map_err(|error| error.to_string())?;
            println!("{}", STANDARD.encode(pair(path)?.public_key().as_ref()));
        }
        ["public", path] => println!("{}", STANDARD.encode(pair(Path::new(path))?.public_key().as_ref())),
        ["sign", key, archive, version, build, url, days, rest @ ..] if rest.len() <= 1 => {
            let pair = pair(Path::new(key))?;
            let bytes = std::fs::read(archive).map_err(|error| error.to_string())?;
            let mut feed = Feed {
                schema: 1,
                version: (*version).to_string(),
                build: build.parse().map_err(|_| "BUILD must be a number".to_string())?,
                expires: expires(days)?,
                // A release signed on another machine names the package's architecture.
                arch: rest.first().copied().unwrap_or(std::env::consts::ARCH).to_string(),
                url: (*url).to_string(),
                length: bytes.len() as u64,
                sha256: Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                signature: String::new(),
            };
            feed.signature = STANDARD.encode(pair.sign(feed.signed_message().as_bytes()));
            println!("{}", serde_json::to_string_pretty(&feed).map_err(|error| error.to_string())?);
        }
        _ => return Err("usage: koett_update_sign keygen|public KEY_FILE, or sign KEY_FILE ARCHIVE VERSION BUILD URL VALID_DAYS [ARCH]".into()),
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {}

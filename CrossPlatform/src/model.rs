use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use ureq::tls::{RootCerts, TlsConfig};

use crate::paths::{self, DEFAULT_MODEL_ID};

const MODEL_URL: &str = concat!(
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/",
    "sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8.tar.bz2"
);
const ARCHIVE_BYTES: u64 = 104_337_827;
const ARCHIVE_SHA256: &str = "17f945007b52ccd8b7200ffc7c5652e9e8e961dfdf479cefcabd06cf5703630b";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelProgress {
    Downloading { received: u64, total: u64 },
    Installing,
    Ready,
}

pub fn ensure_default_model(
    cancelled: &AtomicBool,
    mut progress: impl FnMut(ModelProgress),
) -> Result<PathBuf, String> {
    let destination = paths::default_model_directory()?;
    if model_is_complete(&destination) {
        progress(ModelProgress::Ready);
        return Ok(destination);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", destination.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let archive = parent.join(format!("{DEFAULT_MODEL_ID}.tar.bz2.part"));

    let download = download_archive(&archive, cancelled, &mut progress);
    if let Err(error) = download {
        let _ = fs::remove_file(&archive);
        return Err(error);
    }
    if let Err(error) = check_cancelled(cancelled) {
        let _ = fs::remove_file(&archive);
        return Err(error);
    }
    progress(ModelProgress::Installing);
    let result = install_archive(&archive, parent, &destination, cancelled);
    let _ = fs::remove_file(&archive);
    result?;
    progress(ModelProgress::Ready);
    Ok(destination)
}

fn download_archive(
    path: &Path,
    cancelled: &AtomicBool,
    progress: &mut impl FnMut(ModelProgress),
) -> Result<(), String> {
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(Duration::from_secs(30 * 60)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(5)))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    let mut response = agent
        .get(MODEL_URL)
        .call()
        .map_err(|error| format!("could not download the Koett model: {error}"))?;
    let mut input = response.body_mut().as_reader();
    let mut output = File::create(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut received = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancelled)?;
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("could not read the model download: {error}"))?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > ARCHIVE_BYTES {
            return Err("the model download is larger than the pinned release".to_string());
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        hasher.update(&buffer[..count]);
        progress(ModelProgress::Downloading {
            received,
            total: ARCHIVE_BYTES,
        });
    }
    output
        .sync_all()
        .map_err(|error| format!("could not finish {}: {error}", path.display()))?;
    if received != ARCHIVE_BYTES {
        return Err(format!(
            "the model download is incomplete: got {received} of {ARCHIVE_BYTES} bytes"
        ));
    }
    let digest = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        });
    if digest != ARCHIVE_SHA256 {
        return Err("the model download failed its SHA-256 check".to_string());
    }
    Ok(())
}

fn install_archive(
    archive: &Path,
    parent: &Path,
    destination: &Path,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let staging = parent.join(format!(".{DEFAULT_MODEL_ID}-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("could not reset {}: {error}", staging.display()))?;
    }
    fs::create_dir(&staging)
        .map_err(|error| format!("could not create {}: {error}", staging.display()))?;

    let result = (|| {
        let input = File::open(archive)
            .map_err(|error| format!("could not open {}: {error}", archive.display()))?;
        let decoder = BzDecoder::new(input);
        let mut archive = tar::Archive::new(decoder);
        let entries = archive
            .entries()
            .map_err(|error| format!("could not read the model archive: {error}"))?;
        for entry in entries {
            check_cancelled(cancelled)?;
            let mut entry =
                entry.map_err(|error| format!("could not read a model archive entry: {error}"))?;
            let kind = entry.header().entry_type();
            if !kind.is_file() && !kind.is_dir() {
                return Err("the model archive contains an unsupported entry type".to_string());
            }
            if !entry
                .unpack_in(&staging)
                .map_err(|error| format!("could not unpack the model archive: {error}"))?
            {
                return Err("the model archive contains an unsafe path".to_string());
            }
        }

        let extracted = staging.join(DEFAULT_MODEL_ID);
        if !model_is_complete(&extracted) {
            return Err("the model archive is missing model.int8.onnx or tokens.txt".to_string());
        }
        if destination.exists() {
            if model_is_complete(destination) {
                return Ok(());
            }
            return Err(format!(
                "the incomplete model folder needs attention: {}",
                destination.display()
            ));
        }
        fs::rename(&extracted, destination).map_err(|error| {
            format!(
                "could not install {} as {}: {error}",
                extracted.display(),
                destination.display()
            )
        })
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("model setup cancelled".to_string())
    } else {
        Ok(())
    }
}

fn model_is_complete(directory: &Path) -> bool {
    directory.join("model.int8.onnx").is_file() && directory.join("tokens.txt").is_file()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DEFAULT_MODEL_ID, model_is_complete};

    #[test]
    fn complete_model_needs_both_required_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("koett-model-{nonce}"));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("model.int8.onnx"), []).unwrap();
        assert!(!model_is_complete(&directory));
        fs::write(directory.join("tokens.txt"), []).unwrap();
        assert!(model_is_complete(&directory));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pinned_model_name_matches_the_archive_root() {
        assert_eq!(
            DEFAULT_MODEL_ID,
            "sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8"
        );
    }
}

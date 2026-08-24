use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::paths::{self, DEFAULT_MODEL_ID};

const MODEL_URL: &str = concat!(
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/",
    "sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8.tar.bz2"
);
const ARCHIVE_BYTES: u64 = 104_337_827;
const ARCHIVE_SHA256: &str = "17f945007b52ccd8b7200ffc7c5652e9e8e961dfdf479cefcabd06cf5703630b";
const VAD_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";
const VAD_BYTES: u64 = 643_854;
const VAD_SHA256: &str = "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6";
pub const VAD_FILE: &str = "silero_vad.onnx";

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
    if default_model_is_complete(&destination) {
        progress(ModelProgress::Ready);
        return Ok(destination);
    }
    let needs_asr = !asr_model_is_complete(&destination);
    let needs_vad = !destination.join(VAD_FILE).is_file();
    let parent = destination
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", destination.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let archive = parent.join(format!("{DEFAULT_MODEL_ID}.tar.bz2.part"));
    let vad_download = parent.join(format!(".{VAD_FILE}.part"));
    let download_total = u64::from(needs_asr) * ARCHIVE_BYTES + u64::from(needs_vad) * VAD_BYTES;
    let mut download_offset = 0;

    if needs_asr {
        let download = download_file(
            "speech model",
            MODEL_URL,
            &archive,
            ARCHIVE_BYTES,
            ARCHIVE_SHA256,
            download_offset,
            download_total,
            cancelled,
            &mut progress,
        );
        if let Err(error) = download {
            let _ = fs::remove_file(&archive);
            return Err(error);
        }
        download_offset += ARCHIVE_BYTES;
    }
    if needs_vad {
        let download = download_file(
            "voice detector",
            VAD_URL,
            &vad_download,
            VAD_BYTES,
            VAD_SHA256,
            download_offset,
            download_total,
            cancelled,
            &mut progress,
        );
        if let Err(error) = download {
            let _ = fs::remove_file(&archive);
            let _ = fs::remove_file(&vad_download);
            return Err(error);
        }
    }
    if let Err(error) = check_cancelled(cancelled) {
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_file(&vad_download);
        return Err(error);
    }
    progress(ModelProgress::Installing);
    let result = if needs_asr {
        install_archive(&archive, parent, &destination, cancelled)
    } else {
        Ok(())
    };
    let _ = fs::remove_file(&archive);
    if let Err(error) = result {
        let _ = fs::remove_file(&vad_download);
        return Err(error);
    }
    if needs_vad {
        fs::rename(&vad_download, destination.join(VAD_FILE)).map_err(|error| {
            format!(
                "could not install {} as {}: {error}",
                vad_download.display(),
                destination.join(VAD_FILE).display()
            )
        })?;
    }
    progress(ModelProgress::Ready);
    Ok(destination)
}

#[allow(clippy::too_many_arguments)]
fn download_file(
    label: &str,
    url: &str,
    path: &Path,
    expected_bytes: u64,
    expected_sha256: &str,
    progress_offset: u64,
    progress_total: u64,
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
                .provider(tls_provider())
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|error| format!("could not download the Koett {label}: {error}"))?;
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
        if received > expected_bytes {
            return Err(format!(
                "the {label} download is larger than the pinned release"
            ));
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        hasher.update(&buffer[..count]);
        progress(ModelProgress::Downloading {
            received: progress_offset + received,
            total: progress_total,
        });
    }
    output
        .sync_all()
        .map_err(|error| format!("could not finish {}: {error}", path.display()))?;
    if received != expected_bytes {
        return Err(format!(
            "the {label} download is incomplete: got {received} of {expected_bytes} bytes"
        ));
    }
    let digest = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        });
    if digest != expected_sha256 {
        return Err(format!("the {label} download failed its SHA-256 check"));
    }
    Ok(())
}

fn tls_provider() -> TlsProvider {
    #[cfg(target_os = "windows")]
    {
        TlsProvider::NativeTls
    }
    #[cfg(not(target_os = "windows"))]
    {
        TlsProvider::Rustls
    }
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
        if !asr_model_is_complete(&extracted) {
            return Err("the model archive is missing model.int8.onnx or tokens.txt".to_string());
        }
        if destination.exists() {
            if asr_model_is_complete(destination) {
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

fn asr_model_is_complete(directory: &Path) -> bool {
    directory.join("model.int8.onnx").is_file() && directory.join("tokens.txt").is_file()
}

fn default_model_is_complete(directory: &Path) -> bool {
    asr_model_is_complete(directory) && directory.join(VAD_FILE).is_file()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DEFAULT_MODEL_ID, VAD_FILE, asr_model_is_complete, default_model_is_complete};

    #[test]
    fn asr_model_needs_both_required_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("koett-model-{nonce}"));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("model.int8.onnx"), []).unwrap();
        assert!(!asr_model_is_complete(&directory));
        fs::write(directory.join("tokens.txt"), []).unwrap();
        assert!(asr_model_is_complete(&directory));
        assert!(!default_model_is_complete(&directory));
        fs::write(directory.join(VAD_FILE), []).unwrap();
        assert!(default_model_is_complete(&directory));
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

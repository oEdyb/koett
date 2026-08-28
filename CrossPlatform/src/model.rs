use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::paths::{self, DEFAULT_MODEL_ID};

mod manifest;
use manifest::{
    ModelManifest, default_model_manifest, digest_hex, manifest_artifact, manifest_sha256,
    quarantine_model_directory, validate_cached_model, validate_model_files,
    write_verification_stamp,
};

const MODEL_MANIFEST_JSON: &str =
    include_str!("../model-manifests/parakeet-tdt-ctc-110m-int8.json");
const ASR_ARTIFACT: &str = "asr";
const VAD_ARTIFACT: &str = "vad";
pub const VAD_FILE: &str = "silero_vad.onnx";
static TRANSACTION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelProgress {
    Repairing,
    Downloading { received: u64, total: u64 },
    Installing,
    Ready,
}

pub fn ensure_default_model(
    cancelled: &AtomicBool,
    mut progress: impl FnMut(ModelProgress),
) -> Result<PathBuf, String> {
    let manifest = default_model_manifest()?;
    let manifest_sha256 = manifest_sha256();
    let destination = paths::default_model_directory()?;
    check_cancelled(cancelled)?;
    if prepare_destination(
        &destination,
        &manifest,
        &manifest_sha256,
        cancelled,
        &mut progress,
    )? {
        progress(ModelProgress::Ready);
        return Ok(destination);
    }
    let asr = manifest_artifact(&manifest, ASR_ARTIFACT)?;
    let vad = manifest_artifact(&manifest, VAD_ARTIFACT)?;
    let parent = destination
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", destination.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let transaction = create_install_transaction(parent)?;
    let archive = transaction.join("model.tar.bz2.part");
    let vad_download = transaction.join("silero_vad.onnx.part");
    let download_total = asr.bytes + vad.bytes;
    let result = (|| {
        download_file(
            "speech model",
            &asr.source_url,
            &archive,
            asr.bytes,
            &asr.sha256,
            0,
            download_total,
            cancelled,
            &mut progress,
        )?;
        download_file(
            "voice detector",
            &vad.source_url,
            &vad_download,
            vad.bytes,
            &vad.sha256,
            asr.bytes,
            download_total,
            cancelled,
            &mut progress,
        )?;
        check_cancelled(cancelled)?;
        progress(ModelProgress::Installing);
        install_archive(
            &archive,
            &vad_download,
            &destination,
            &manifest,
            &manifest_sha256,
            cancelled,
            &mut progress,
        )?;
        progress(ModelProgress::Ready);
        Ok(destination.clone())
    })();
    let _ = fs::remove_dir_all(&transaction);
    result
}

fn create_install_transaction(parent: &Path) -> Result<PathBuf, String> {
    for _ in 0..1_000_u16 {
        let index = TRANSACTION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let transaction = parent.join(format!(
            ".{DEFAULT_MODEL_ID}.install-{}-{index}",
            std::process::id()
        ));
        match fs::create_dir(&transaction) {
            Ok(()) => return Ok(transaction),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "could not create model install folder {}: {error}",
                    transaction.display()
                ));
            }
        }
    }
    Err(format!(
        "could not create a unique model install folder in {}",
        parent.display()
    ))
}

fn prepare_destination(
    destination: &Path,
    manifest: &ModelManifest,
    manifest_sha256: &str,
    cancelled: &AtomicBool,
    progress: &mut impl FnMut(ModelProgress),
) -> Result<bool, String> {
    for _ in 0..10_u8 {
        match validate_cached_model(destination, &manifest.files, manifest_sha256, cancelled) {
            Ok(()) => return Ok(true),
            Err(error) if cancelled.load(Ordering::Acquire) => return Err(error),
            Err(_) if !destination.exists() => return Ok(false),
            Err(error) => {
                progress(ModelProgress::Repairing);
                let quarantine = match quarantine_model_directory(destination) {
                    Ok(quarantine) => quarantine,
                    Err(_) if !destination.exists() => continue,
                    Err(quarantine_error) => {
                        return Err(format!(
                            "the model cache failed verification ({error}); {quarantine_error}"
                        ));
                    }
                };
                if validate_cached_model(&quarantine, &manifest.files, manifest_sha256, cancelled)
                    .is_ok()
                {
                    if !destination.exists() {
                        match fs::rename(&quarantine, destination) {
                            Ok(()) => continue,
                            Err(_) if destination.exists() => continue,
                            Err(restore_error) => {
                                return Err(format!(
                                    "a valid model cache moved during repair and could not be restored from {}: {restore_error}",
                                    quarantine.display()
                                ));
                            }
                        }
                    }
                    continue;
                }
                return Ok(false);
            }
        }
    }
    Err(format!(
        "the model cache changed too many times while Koett checked {}",
        destination.display()
    ))
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
    write_verified_download(
        label,
        response.body_mut().as_reader(),
        path,
        expected_bytes,
        expected_sha256,
        cancelled,
        |received| {
            progress(ModelProgress::Downloading {
                received: progress_offset + received,
                total: progress_total,
            });
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn write_verified_download(
    label: &str,
    mut input: impl Read,
    path: &Path,
    expected_bytes: u64,
    expected_sha256: &str,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let result = (|| {
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
            progress(received);
        }
        output
            .sync_all()
            .map_err(|error| format!("could not finish {}: {error}", path.display()))?;
        if received != expected_bytes {
            return Err(format!(
                "the {label} download is incomplete: got {received} of {expected_bytes} bytes"
            ));
        }
        if digest_hex(&hasher.finalize()) != expected_sha256 {
            return Err(format!("the {label} download failed its SHA-256 check"));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
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
    vad_download: &Path,
    destination: &Path,
    manifest: &ModelManifest,
    manifest_sha256: &str,
    cancelled: &AtomicBool,
    progress: &mut impl FnMut(ModelProgress),
) -> Result<(), String> {
    let transaction = archive
        .parent()
        .ok_or_else(|| format!("{} has no transaction folder", archive.display()))?;
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
            .unpack_in(transaction)
            .map_err(|error| format!("could not unpack the model archive: {error}"))?
        {
            return Err("the model archive contains an unsafe path".to_string());
        }
    }

    let extracted = transaction.join(DEFAULT_MODEL_ID);
    validate_model_files(
        &extracted,
        manifest
            .files
            .iter()
            .filter(|file| file.artifact == ASR_ARTIFACT),
        cancelled,
    )
    .map_err(|error| format!("the extracted speech model failed verification: {error}"))?;
    fs::rename(vad_download, extracted.join(VAD_FILE)).map_err(|error| {
        format!(
            "could not add {} to the model transaction: {error}",
            vad_download.display()
        )
    })?;
    let verified_files = validate_model_files(&extracted, &manifest.files, cancelled)
        .map_err(|error| format!("the complete model transaction failed verification: {error}"))?;
    write_verification_stamp(&extracted, &verified_files, manifest_sha256)?;

    if prepare_destination(destination, manifest, manifest_sha256, cancelled, progress)? {
        return Ok(());
    }
    match fs::rename(&extracted, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            match validate_cached_model(destination, &manifest.files, manifest_sha256, cancelled) {
                Ok(()) => Ok(()),
                Err(winner_error) => Err(format!(
                    "could not promote the complete model transaction to {}: {rename_error}; the competing cache is not valid: {winner_error}",
                    destination.display()
                )),
            }
        }
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("model setup cancelled".to_string())
    } else {
        Ok(())
    }
}

pub fn quarantine_default_model_after_load_failure(
    directory: &Path,
) -> Result<Option<PathBuf>, String> {
    let manifest = default_model_manifest()?;
    let cancelled = AtomicBool::new(false);
    match validate_model_files(directory, &manifest.files, &cancelled) {
        Ok(_) => Ok(None),
        Err(error) if directory.exists() => quarantine_model_directory(directory)
            .map(Some)
            .map_err(|quarantine_error| {
                format!("the model cache failed verification ({error}); {quarantine_error}")
            }),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ASR_ARTIFACT, MODEL_MANIFEST_JSON, VAD_ARTIFACT, VAD_FILE, check_cancelled};
use crate::paths::DEFAULT_MODEL_ID;

pub(super) const VERIFICATION_STAMP_FILE: &str = ".koett-model-verification.json";

#[derive(Debug, Deserialize)]
pub(super) struct ModelManifest {
    pub(super) schema_version: u32,
    pub(super) app_model_schema_version: u32,
    pub(super) model_id: String,
    pub(super) purpose: String,
    pub(super) artifacts: Vec<ModelArtifact>,
    pub(super) files: Vec<ModelFile>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModelArtifact {
    pub(super) id: String,
    pub(super) source_url: String,
    pub(super) immutable_revision: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) license: String,
    pub(super) upstream_reference_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ModelFile {
    pub(super) artifact: String,
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VerificationStamp {
    schema_version: u32,
    manifest_sha256: String,
    files: Vec<VerifiedFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct VerifiedFile {
    path: String,
    bytes: u64,
    modified_seconds: u64,
    modified_nanoseconds: u32,
}

pub(super) fn default_model_manifest() -> Result<ModelManifest, String> {
    let manifest: ModelManifest = serde_json::from_str(MODEL_MANIFEST_JSON)
        .map_err(|error| format!("the embedded model manifest is invalid JSON: {error}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &ModelManifest) -> Result<(), String> {
    if manifest.schema_version != 1 || manifest.app_model_schema_version != 1 {
        return Err("the embedded model manifest uses an unsupported schema".to_string());
    }
    if manifest.model_id != DEFAULT_MODEL_ID || manifest.purpose.trim().is_empty() {
        return Err(
            "the embedded model manifest does not describe Koett's default model".to_string(),
        );
    }
    if manifest.artifacts.len() != 2 || manifest.files.len() != 5 {
        return Err("the embedded model manifest has an unexpected component count".to_string());
    }
    for (index, artifact) in manifest.artifacts.iter().enumerate() {
        if manifest.artifacts[..index]
            .iter()
            .any(|earlier| earlier.id == artifact.id)
        {
            return Err(format!(
                "the embedded model manifest repeats artifact {}",
                artifact.id
            ));
        }
        if artifact.id.is_empty()
            || artifact.bytes == 0
            || !is_sha256(&artifact.sha256)
            || !artifact.immutable_revision.contains(&artifact.sha256)
            || artifact.license.trim().is_empty()
            || !artifact.source_url.starts_with("https://")
            || !artifact.upstream_reference_url.starts_with("https://")
        {
            return Err(format!(
                "the embedded model manifest has invalid metadata for artifact {}",
                artifact.id
            ));
        }
    }
    for (index, file) in manifest.files.iter().enumerate() {
        if manifest.files[..index]
            .iter()
            .any(|earlier| earlier.path == file.path)
        {
            return Err(format!(
                "the embedded model manifest repeats file {}",
                file.path
            ));
        }
        if !safe_model_file_path(&file.path)
            || file.bytes == 0
            || !is_sha256(&file.sha256)
            || !manifest
                .artifacts
                .iter()
                .any(|artifact| artifact.id == file.artifact)
        {
            return Err(format!(
                "the embedded model manifest has invalid metadata for file {}",
                file.path
            ));
        }
    }
    for (artifact, path) in [
        (ASR_ARTIFACT, "model.int8.onnx"),
        (ASR_ARTIFACT, "tokens.txt"),
        (ASR_ARTIFACT, "test_wavs/0.wav"),
        (ASR_ARTIFACT, "test_wavs/1.wav"),
        (VAD_ARTIFACT, VAD_FILE),
    ] {
        if !manifest
            .files
            .iter()
            .any(|file| file.artifact == artifact && file.path == path)
        {
            return Err(format!(
                "the embedded model manifest is missing required file {path}"
            ));
        }
    }
    Ok(())
}

pub(super) fn manifest_artifact<'a>(
    manifest: &'a ModelManifest,
    id: &str,
) -> Result<&'a ModelArtifact, String> {
    manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.id == id)
        .ok_or_else(|| format!("the embedded model manifest is missing artifact {id}"))
}

fn safe_model_file_path(path: &str) -> bool {
    let mut found_component = false;
    for component in Path::new(path).components() {
        if !matches!(component, Component::Normal(_)) {
            return false;
        }
        found_component = true;
    }
    found_component
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn validate_model_files<'a>(
    directory: &Path,
    files: impl IntoIterator<Item = &'a ModelFile>,
    cancelled: &AtomicBool,
) -> Result<Vec<VerifiedFile>, String> {
    files
        .into_iter()
        .map(|expected| validate_model_file(directory, expected, cancelled))
        .collect()
}

fn validate_model_file(
    directory: &Path,
    expected: &ModelFile,
    cancelled: &AtomicBool,
) -> Result<VerifiedFile, String> {
    check_cancelled(cancelled)?;
    let path = directory.join(&expected.path);
    let path_before = fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "required model file {} is unavailable: {error}",
            path.display()
        )
    })?;
    if !path_before.file_type().is_file() {
        return Err(format!(
            "required model file {} is not a regular file",
            path.display()
        ));
    }
    let mut input = File::open(&path)
        .map_err(|error| format!("could not open model file {}: {error}", path.display()))?;
    let handle_before = input
        .metadata()
        .map_err(|error| format!("could not inspect model file {}: {error}", path.display()))?;
    let verified_before = verified_file(&expected.path, &handle_before)?;
    if verified_file(&expected.path, &path_before)? != verified_before {
        return Err(format!(
            "model file {} changed while it was opened for verification",
            path.display()
        ));
    }
    if verified_before.bytes != expected.bytes {
        return Err(format!(
            "model file {} failed its byte count: got {} of {} bytes",
            path.display(),
            verified_before.bytes,
            expected.bytes
        ));
    }

    let mut hasher = Sha256::new();
    let mut read_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancelled)?;
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("could not read model file {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        read_bytes += count as u64;
        hasher.update(&buffer[..count]);
    }
    if read_bytes != expected.bytes {
        return Err(format!(
            "model file {} changed during verification: got {read_bytes} of {} bytes",
            path.display(),
            expected.bytes
        ));
    }
    if digest_hex(&hasher.finalize()) != expected.sha256 {
        return Err(format!(
            "model file {} failed its SHA-256 check",
            path.display()
        ));
    }

    let handle_after = input
        .metadata()
        .map_err(|error| format!("could not inspect model file {}: {error}", path.display()))?;
    let path_after = fs::symlink_metadata(&path)
        .map_err(|error| format!("could not inspect model file {}: {error}", path.display()))?;
    if !path_after.file_type().is_file()
        || verified_file(&expected.path, &handle_after)? != verified_before
        || verified_file(&expected.path, &path_after)? != verified_before
    {
        return Err(format!(
            "model file {} changed during verification",
            path.display()
        ));
    }
    Ok(verified_before)
}

pub(super) fn validate_cached_model(
    directory: &Path,
    files: &[ModelFile],
    manifest_sha256: &str,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    if verification_stamp_matches(directory, files, manifest_sha256) {
        return Ok(());
    }
    let verified_files = validate_model_files(directory, files, cancelled)?;
    let _ = write_verification_stamp(directory, &verified_files, manifest_sha256);
    Ok(())
}

fn verification_stamp_matches(
    directory: &Path,
    files: &[ModelFile],
    manifest_sha256: &str,
) -> bool {
    let bytes = match fs::read(directory.join(VERIFICATION_STAMP_FILE)) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let stamp: VerificationStamp = match serde_json::from_slice(&bytes) {
        Ok(stamp) => stamp,
        Err(_) => return false,
    };
    if stamp.schema_version != 1 || stamp.manifest_sha256 != manifest_sha256 {
        return false;
    }
    let current = match collect_current_files(directory, files) {
        Ok(current) => current,
        Err(_) => return false,
    };
    stamp.files == current
}

fn collect_current_files(
    directory: &Path,
    files: &[ModelFile],
) -> Result<Vec<VerifiedFile>, String> {
    files
        .iter()
        .map(|expected| {
            let path = directory.join(&expected.path);
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!("could not inspect model file {}: {error}", path.display())
            })?;
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "required model file {} is not a regular file",
                    path.display()
                ));
            }
            verified_file(&expected.path, &metadata)
        })
        .collect()
}

fn verified_file(path: &str, metadata: &Metadata) -> Result<VerifiedFile, String> {
    let modified = metadata
        .modified()
        .map_err(|error| format!("could not read the modification time for {path}: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| format!("model file {path} has a modification time before the Unix epoch"))?;
    Ok(VerifiedFile {
        path: path.to_string(),
        bytes: metadata.len(),
        modified_seconds: modified.as_secs(),
        modified_nanoseconds: modified.subsec_nanos(),
    })
}

pub(super) fn write_verification_stamp(
    directory: &Path,
    verified_files: &[VerifiedFile],
    manifest_sha256: &str,
) -> Result<(), String> {
    let stamp = VerificationStamp {
        schema_version: 1,
        manifest_sha256: manifest_sha256.to_string(),
        files: verified_files.to_vec(),
    };
    let mut bytes = serde_json::to_vec(&stamp)
        .map_err(|error| format!("could not serialize the model verification stamp: {error}"))?;
    bytes.push(b'\n');
    let temporary = directory.join(format!(
        "{VERIFICATION_STAMP_FILE}.{}.part",
        std::process::id()
    ));
    let destination = directory.join(VERIFICATION_STAMP_FILE);
    let result = (|| {
        let mut output = File::create(&temporary)
            .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
        use std::io::Write as _;
        output
            .write_all(&bytes)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        output
            .sync_all()
            .map_err(|error| format!("could not finish {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &destination).map_err(|error| {
            format!(
                "could not install {} as {}: {error}",
                temporary.display(),
                destination.display()
            )
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn digest_hex(digest: &[u8]) -> String {
    digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        })
}

pub(super) fn manifest_sha256() -> String {
    digest_hex(&Sha256::digest(MODEL_MANIFEST_JSON.as_bytes()))
}

pub(super) fn quarantine_model_directory(directory: &Path) -> Result<PathBuf, String> {
    let parent = directory
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", directory.display()))?;
    let name = directory
        .file_name()
        .ok_or_else(|| format!("{} has no file name", directory.display()))?;
    for index in 0..1_000_u16 {
        let mut quarantine_name = name.to_os_string();
        quarantine_name.push(format!(".invalid-{}-{index}", std::process::id()));
        let quarantine = parent.join(quarantine_name);
        if quarantine.exists() {
            continue;
        }
        fs::rename(directory, &quarantine).map_err(|error| {
            format!(
                "could not quarantine invalid model cache {} as {}: {error}",
                directory.display(),
                quarantine.display()
            )
        })?;
        return Ok(quarantine);
    }
    Err(format!(
        "could not find a quarantine name for invalid model cache {}",
        directory.display()
    ))
}

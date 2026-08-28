use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use bzip2::Compression;
use bzip2::write::BzEncoder;
use sha2::{Digest, Sha256};

use super::manifest::{ModelArtifact, ModelFile};
use super::{
    ASR_ARTIFACT, DEFAULT_MODEL_ID, ModelManifest, VAD_ARTIFACT, VAD_FILE,
    create_install_transaction, default_model_manifest, digest_hex, install_archive,
    quarantine_model_directory, validate_cached_model, validate_model_files,
    write_verified_download,
};

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("koett-{label}-{}-{nonce}", std::process::id()))
}

fn file(path: &str, contents: &[u8]) -> ModelFile {
    ModelFile {
        artifact: "test".to_string(),
        path: path.to_string(),
        bytes: contents.len() as u64,
        sha256: digest_hex(&Sha256::digest(contents)),
    }
}

fn write_valid_files(directory: &std::path::Path, files: &[(&str, &[u8])]) {
    fs::create_dir_all(directory).unwrap();
    for (path, contents) in files {
        let destination = directory.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, contents).unwrap();
    }
}

fn artifact_file(artifact: &str, path: &str, contents: &[u8]) -> ModelFile {
    let mut file = file(path, contents);
    file.artifact = artifact.to_string();
    file
}

fn test_artifact(id: &str) -> ModelArtifact {
    ModelArtifact {
        id: id.to_string(),
        source_url: format!("https://example.test/{id}"),
        immutable_revision: format!("sha256:{}", "a".repeat(64)),
        bytes: 1,
        sha256: "a".repeat(64),
        license: "MIT".to_string(),
        upstream_reference_url: format!("https://example.test/{id}/source"),
    }
}

fn test_manifest(files: Vec<ModelFile>) -> ModelManifest {
    ModelManifest {
        schema_version: 1,
        app_model_schema_version: 1,
        model_id: DEFAULT_MODEL_ID.to_string(),
        purpose: "test".to_string(),
        artifacts: vec![test_artifact(ASR_ARTIFACT), test_artifact(VAD_ARTIFACT)],
        files,
    }
}

fn write_test_archive(parent: &std::path::Path, files: &[(&str, &[u8])]) -> std::path::PathBuf {
    let source = parent.join("archive-source");
    let model_root = source.join(DEFAULT_MODEL_ID);
    write_valid_files(&model_root, files);
    let archive_path = parent.join("model.tar.bz2");
    let output = File::create(&archive_path).unwrap();
    let encoder = BzEncoder::new(output, Compression::best());
    let mut archive = tar::Builder::new(encoder);
    archive
        .append_dir_all(DEFAULT_MODEL_ID, &model_root)
        .unwrap();
    let encoder = archive.into_inner().unwrap();
    encoder.finish().unwrap();
    fs::remove_dir_all(source).unwrap();
    archive_path
}

#[test]
fn embedded_manifest_matches_the_pinned_default_model() {
    let manifest = default_model_manifest().unwrap();

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.app_model_schema_version, 1);
    assert_eq!(manifest.model_id, DEFAULT_MODEL_ID);
    assert_eq!(manifest.artifacts.len(), 2);
    assert_eq!(manifest.files.len(), 5);
    assert!(
        manifest
            .files
            .iter()
            .any(|file| file.path == "test_wavs/0.wav")
    );
    assert!(
        manifest
            .files
            .iter()
            .any(|file| file.path == "test_wavs/1.wav")
    );
    assert!(manifest.artifacts.iter().all(|artifact| {
        artifact.immutable_revision.contains(&artifact.sha256)
            && artifact.bytes > 0
            && !artifact.license.is_empty()
            && artifact.source_url.starts_with("https://")
            && artifact.upstream_reference_url.starts_with("https://")
    }));
}

#[test]
fn cache_verification_rejects_a_locally_modified_file() {
    let directory = temporary_directory("modified-model");
    let expected = file("model.onnx", b"right");
    write_valid_files(&directory, &[("model.onnx", b"right")]);
    fs::write(directory.join("model.onnx"), b"wrong").unwrap();

    let error = validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap_err();

    assert!(error.contains("SHA-256"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cache_verification_rejects_a_truncated_file() {
    let directory = temporary_directory("truncated-model");
    let expected = file("model.onnx", b"complete");
    write_valid_files(&directory, &[("model.onnx", b"comp")]);

    let error = validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap_err();

    assert!(error.contains("byte count"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cache_verification_rejects_correct_names_with_wrong_content() {
    let directory = temporary_directory("wrong-model");
    let expected = file("model.onnx", b"aaaa");
    write_valid_files(&directory, &[("model.onnx", b"bbbb")]);

    let error = validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap_err();

    assert!(error.contains("SHA-256"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cache_verification_rejects_a_wrong_declared_byte_count() {
    let directory = temporary_directory("wrong-size-manifest");
    let mut expected = file("model.onnx", b"model");
    expected.bytes += 1;
    write_valid_files(&directory, &[("model.onnx", b"model")]);

    let error = validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap_err();

    assert!(error.contains("byte count"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cache_verification_rejects_a_wrong_declared_hash() {
    let directory = temporary_directory("wrong-hash-manifest");
    let mut expected = file("model.onnx", b"model");
    expected.sha256 = "0".repeat(64);
    write_valid_files(&directory, &[("model.onnx", b"model")]);

    let error = validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap_err();

    assert!(error.contains("SHA-256"));
    fs::remove_dir_all(directory).unwrap();
}

struct InterruptedReader {
    delivered: bool,
}

impl Read for InterruptedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.delivered {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "test interruption",
            ));
        }
        self.delivered = true;
        output[..4].copy_from_slice(b"part");
        Ok(4)
    }
}

#[test]
fn interrupted_download_removes_the_partial_file() {
    let directory = temporary_directory("interrupted-download");
    fs::create_dir_all(&directory).unwrap();
    let partial = directory.join("model.part");
    let result = write_verified_download(
        "test model",
        InterruptedReader { delivered: false },
        &partial,
        8,
        &digest_hex(&Sha256::digest(b"complete")),
        &AtomicBool::new(false),
        |_| {},
    );

    assert!(result.unwrap_err().contains("could not read"));
    assert!(!partial.exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cancelled_download_removes_the_partial_file() {
    let directory = temporary_directory("cancelled-download");
    fs::create_dir_all(&directory).unwrap();
    let partial = directory.join("model.part");
    let cancelled = AtomicBool::new(true);

    let error = write_verified_download(
        "test model",
        Cursor::new(b"complete"),
        &partial,
        8,
        &digest_hex(&Sha256::digest(b"complete")),
        &cancelled,
        |_| {},
    )
    .unwrap_err();

    assert_eq!(error, "model setup cancelled");
    assert!(!partial.exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn extracted_files_are_verified_before_cache_promotion() {
    let parent = temporary_directory("archive-verification");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join(DEFAULT_MODEL_ID);
    let transaction = create_install_transaction(&parent).unwrap();
    let archive = write_test_archive(
        &transaction,
        &[("model.int8.onnx", b"wrong"), ("tokens.txt", b"tokens")],
    );
    let vad = transaction.join("vad.part");
    fs::write(&vad, b"vad").unwrap();
    let manifest = test_manifest(vec![
        artifact_file(ASR_ARTIFACT, "model.int8.onnx", b"right"),
        artifact_file(ASR_ARTIFACT, "tokens.txt", b"tokens"),
        artifact_file(VAD_ARTIFACT, VAD_FILE, b"vad"),
    ]);
    let mut progress = |_| {};

    let error = install_archive(
        &archive,
        &vad,
        &destination,
        &manifest,
        &"a".repeat(64),
        &AtomicBool::new(false),
        &mut progress,
    )
    .unwrap_err();

    assert!(error.contains("SHA-256"));
    assert!(!destination.exists());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn verified_extracted_files_are_promoted_together() {
    let parent = temporary_directory("archive-promotion");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join(DEFAULT_MODEL_ID);
    let transaction = create_install_transaction(&parent).unwrap();
    let archive = write_test_archive(
        &transaction,
        &[
            ("model.int8.onnx", b"model"),
            ("tokens.txt", b"tokens"),
            ("test_wavs/0.wav", b"long sample"),
            ("test_wavs/1.wav", b"short sample"),
        ],
    );
    let vad = transaction.join("vad.part");
    fs::write(&vad, b"vad").unwrap();
    let manifest = test_manifest(vec![
        artifact_file(ASR_ARTIFACT, "model.int8.onnx", b"model"),
        artifact_file(ASR_ARTIFACT, "tokens.txt", b"tokens"),
        artifact_file(ASR_ARTIFACT, "test_wavs/0.wav", b"long sample"),
        artifact_file(ASR_ARTIFACT, "test_wavs/1.wav", b"short sample"),
        artifact_file(VAD_ARTIFACT, VAD_FILE, b"vad"),
    ]);
    let mut progress = |_| {};

    install_archive(
        &archive,
        &vad,
        &destination,
        &manifest,
        &"a".repeat(64),
        &AtomicBool::new(false),
        &mut progress,
    )
    .unwrap();

    validate_model_files(&destination, &manifest.files, &AtomicBool::new(false)).unwrap();
    assert!(destination.join(".koett-model-verification.json").is_file());
    assert!(!transaction.join(DEFAULT_MODEL_ID).exists());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn invalid_vad_never_creates_a_live_model_directory() {
    let parent = temporary_directory("vad-verification");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join(DEFAULT_MODEL_ID);
    let transaction = create_install_transaction(&parent).unwrap();
    let archive = write_test_archive(
        &transaction,
        &[("model.int8.onnx", b"model"), ("tokens.txt", b"tokens")],
    );
    let vad = transaction.join("vad.part");
    fs::write(&vad, b"wrong").unwrap();
    let manifest = test_manifest(vec![
        artifact_file(ASR_ARTIFACT, "model.int8.onnx", b"model"),
        artifact_file(ASR_ARTIFACT, "tokens.txt", b"tokens"),
        artifact_file(VAD_ARTIFACT, VAD_FILE, b"right"),
    ]);
    let mut progress = |_| {};

    let error = install_archive(
        &archive,
        &vad,
        &destination,
        &manifest,
        &"a".repeat(64),
        &AtomicBool::new(false),
        &mut progress,
    )
    .unwrap_err();

    assert!(error.contains("complete model transaction"));
    assert!(!destination.exists());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn a_valid_concurrent_winner_is_kept() {
    let parent = temporary_directory("concurrent-winner");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join(DEFAULT_MODEL_ID);
    let transaction = create_install_transaction(&parent).unwrap();
    let files = [
        ("model.int8.onnx", b"model".as_slice()),
        ("tokens.txt", b"tokens".as_slice()),
        (VAD_FILE, b"vad".as_slice()),
    ];
    write_valid_files(&destination, &files);
    let archive = write_test_archive(&transaction, &files[..2]);
    let vad = transaction.join("vad.part");
    fs::write(&vad, b"vad").unwrap();
    let manifest = test_manifest(vec![
        artifact_file(ASR_ARTIFACT, "model.int8.onnx", b"model"),
        artifact_file(ASR_ARTIFACT, "tokens.txt", b"tokens"),
        artifact_file(VAD_ARTIFACT, VAD_FILE, b"vad"),
    ]);
    let mut progress = |_| {};

    install_archive(
        &archive,
        &vad,
        &destination,
        &manifest,
        &"a".repeat(64),
        &AtomicBool::new(false),
        &mut progress,
    )
    .unwrap();

    validate_model_files(&destination, &manifest.files, &AtomicBool::new(false)).unwrap();
    assert!(transaction.join(DEFAULT_MODEL_ID).exists());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn concurrent_installs_use_different_transaction_directories() {
    let parent = temporary_directory("unique-transactions");
    fs::create_dir_all(&parent).unwrap();

    let first = create_install_transaction(&parent).unwrap();
    let second = create_install_transaction(&parent).unwrap();

    assert_ne!(first, second);
    assert!(first.is_dir());
    assert!(second.is_dir());
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn concurrent_complete_installs_accept_one_valid_winner() {
    let parent = temporary_directory("concurrent-installs");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join(DEFAULT_MODEL_ID);
    let first_transaction = create_install_transaction(&parent).unwrap();
    let second_transaction = create_install_transaction(&parent).unwrap();
    let model_files = [
        ("model.int8.onnx", b"model".as_slice()),
        ("tokens.txt", b"tokens".as_slice()),
    ];
    let first_archive = write_test_archive(&first_transaction, &model_files);
    let second_archive = write_test_archive(&second_transaction, &model_files);
    let first_vad = first_transaction.join("vad.part");
    let second_vad = second_transaction.join("vad.part");
    fs::write(&first_vad, b"vad").unwrap();
    fs::write(&second_vad, b"vad").unwrap();
    let manifest = test_manifest(vec![
        artifact_file(ASR_ARTIFACT, "model.int8.onnx", b"model"),
        artifact_file(ASR_ARTIFACT, "tokens.txt", b"tokens"),
        artifact_file(VAD_ARTIFACT, VAD_FILE, b"vad"),
    ]);
    let cancelled = AtomicBool::new(false);

    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            install_archive(
                &first_archive,
                &first_vad,
                &destination,
                &manifest,
                &"a".repeat(64),
                &cancelled,
                &mut |_| {},
            )
        });
        let second = scope.spawn(|| {
            install_archive(
                &second_archive,
                &second_vad,
                &destination,
                &manifest,
                &"a".repeat(64),
                &cancelled,
                &mut |_| {},
            )
        });
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });

    validate_model_files(&destination, &manifest.files, &cancelled).unwrap();
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn corrupt_cache_is_quarantined_and_a_valid_cache_can_replace_it() {
    let directory = temporary_directory("recover-model");
    let expected = file("model.onnx", b"valid");
    write_valid_files(&directory, &[("model.onnx", b"wrong")]);
    assert!(
        validate_model_files(
            &directory,
            std::slice::from_ref(&expected),
            &AtomicBool::new(false)
        )
        .is_err()
    );

    let quarantine = quarantine_model_directory(&directory).unwrap();
    assert!(!directory.exists());
    assert_eq!(fs::read(quarantine.join("model.onnx")).unwrap(), b"wrong");

    write_valid_files(&directory, &[("model.onnx", b"valid")]);
    validate_model_files(&directory, &[expected], &AtomicBool::new(false)).unwrap();

    fs::remove_dir_all(directory).unwrap();
    fs::remove_dir_all(quarantine).unwrap();
}

#[test]
fn verification_stamp_rechecks_a_normally_modified_file() {
    let directory = temporary_directory("verification-stamp");
    let expected = file("model.onnx", b"right");
    write_valid_files(&directory, &[("model.onnx", b"right")]);
    validate_cached_model(
        &directory,
        std::slice::from_ref(&expected),
        &"a".repeat(64),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(directory.join(".koett-model-verification.json").is_file());
    validate_cached_model(
        &directory,
        std::slice::from_ref(&expected),
        &"a".repeat(64),
        &AtomicBool::new(false),
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(directory.join("model.onnx"), b"wrong").unwrap();
    let error = validate_cached_model(
        &directory,
        &[expected],
        &"a".repeat(64),
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(error.contains("SHA-256"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires KOETT_MODEL_DIR and measures the local model cache"]
fn cache_validation_latency_probe() {
    let directory = std::path::PathBuf::from(std::env::var_os("KOETT_MODEL_DIR").unwrap());
    let manifest = default_model_manifest().unwrap();
    let manifest_sha256 = digest_hex(&Sha256::digest(super::MODEL_MANIFEST_JSON.as_bytes()));
    let cancelled = AtomicBool::new(false);

    let full_started = std::time::Instant::now();
    let verified_files = validate_model_files(&directory, &manifest.files, &cancelled).unwrap();
    let full = full_started.elapsed();
    super::write_verification_stamp(&directory, &verified_files, &manifest_sha256).unwrap();
    let stamped_started = std::time::Instant::now();
    validate_cached_model(&directory, &manifest.files, &manifest_sha256, &cancelled).unwrap();
    let stamped = stamped_started.elapsed();

    eprintln!(
        "model_cache_full_verify_ms={:.3} model_cache_stamp_verify_ms={:.3}",
        full.as_secs_f64() * 1_000.0,
        stamped.as_secs_f64() * 1_000.0
    );
}

#[test]
fn verified_download_accepts_the_expected_bytes_and_hash() {
    let directory = temporary_directory("valid-download");
    fs::create_dir_all(&directory).unwrap();
    let partial = directory.join("model.part");
    let contents = b"complete";
    let mut progress = Vec::new();

    write_verified_download(
        "test model",
        Cursor::new(contents),
        &partial,
        contents.len() as u64,
        &digest_hex(&Sha256::digest(contents)),
        &AtomicBool::new(false),
        |received| progress.push(received),
    )
    .unwrap();

    assert_eq!(fs::read(&partial).unwrap(), contents);
    assert_eq!(progress.last(), Some(&(contents.len() as u64)));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn pinned_model_name_matches_the_archive_root() {
    assert_eq!(
        DEFAULT_MODEL_ID,
        "sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8"
    );
}

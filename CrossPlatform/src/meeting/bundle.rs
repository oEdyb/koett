mod files;
pub(crate) mod marks;
pub(super) mod projections;
pub(crate) mod records;
pub(crate) mod scan;

#[cfg(test)]
mod tests;

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use ring::digest::{Context, SHA256, digest};
use serde::de::DeserializeOwned;

use super::{
    Backend, CloseGoal, MeetingId, Offset, RecoveryReason, SessionInfo, TrackSeal, Tracks,
};
pub(crate) use files::WriteError;
use files::{AtomicWriter, ensure_directory, private_directory, sync_directory};
use marks::MarkWriter;
use records::{
    Intent, Job, Manifest, ManifestFile, ProjectionFile, Projections, Publication, Receipt, Seal,
};

const MANIFEST: &str = "manifest.json";
const INTENT: &str = "intent.json";
const SEAL: &str = "seal.json";
const JOB: &str = "job.json";
const RECEIPT: &str = "receipt.json";
const TRANSCRIPT: &str = "transcript.md";
const PROJECTIONS: &str = "projections.json";
const LOCK: &str = ".lock";

pub(crate) struct Bundle {
    directory: PathBuf,
    manifest: Manifest,
    writer: AtomicWriter,
    _lock: File,
}

#[cfg(test)]
impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.directory.fmt(f)
    }
}

#[cfg(test)]
impl PartialEq for Bundle {
    fn eq(&self, other: &Self) -> bool {
        self.directory == other.directory
    }
}

impl Bundle {
    pub(crate) fn create(root: &Path, manifest: Manifest) -> io::Result<Self> {
        Self::create_with_writer(root, manifest, AtomicWriter::default())
    }

    fn create_with_writer(
        root: &Path,
        manifest: Manifest,
        writer: AtomicWriter,
    ) -> io::Result<Self> {
        ensure_directory(root)?;
        let directory = root.join(manifest.info.id.to_string());
        private_directory(&directory)?;
        let lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(LOCK))
            .and_then(|lock| {
                lock_owner(&lock)?;
                Ok(lock)
            })
            .map_err(|error| clean_failed_create(root, &directory, error))?;
        let initialize = || -> io::Result<()> {
            sync_directory(root)?;
            private_directory(&directory.join("audio"))?;
            writer.json(
                &directory,
                MANIFEST,
                &ManifestFile {
                    version: 1,
                    manifest: manifest.clone(),
                },
            )?;
            Ok(())
        };
        initialize().map_err(|error| clean_failed_create(root, &directory, error))?;
        Ok(Self {
            directory,
            manifest,
            writer,
            _lock: lock,
        })
    }

    pub(crate) fn open(root: &Path, id: MeetingId) -> io::Result<Self> {
        let directory = root.join(id.to_string());
        let lock = File::open(directory.join(LOCK))?;
        lock_owner(&lock)?;
        let manifest = read_manifest(&directory, id)?;
        Ok(Self {
            directory,
            manifest,
            writer: AtomicWriter::default(),
            _lock: lock,
        })
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub(crate) fn read_state(&self) -> scan::DiskState {
        scan::inspect_unlocked(&self.directory, self.manifest.clone())
    }

    pub(crate) fn read_intent(&self) -> io::Result<Option<Intent>> {
        read_optional_json(&self.directory.join(INTENT))
    }

    pub(crate) fn write_intent(&self, intent: &Intent) -> Result<(), WriteError> {
        self.writer.json(&self.directory, INTENT, intent)
    }

    pub(crate) fn write_seal(&self, seal: &Seal) -> Result<(), WriteError> {
        self.writer.json(&self.directory, SEAL, seal)
    }

    pub(crate) fn read_seal(&self) -> io::Result<Option<Seal>> {
        read_optional_json(&self.directory.join(SEAL))
    }

    pub(crate) fn create_job_directory(&self) -> io::Result<PathBuf> {
        let directory = self.directory.join("job");
        ensure_directory(&directory)?;
        Ok(directory)
    }

    pub(super) fn clear_job_directory(&self) -> io::Result<()> {
        match fs::remove_dir_all(self.directory.join("job")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    pub(crate) fn write_job(&self, job: &Job) -> Result<(), WriteError> {
        self.writer.json(&self.directory, JOB, job)
    }

    pub(crate) fn read_job(&self) -> io::Result<Option<Job>> {
        read_optional_json(&self.directory.join(JOB))
    }

    pub(crate) fn publish(
        &self,
        markdown: &str,
        publication: Publication,
    ) -> Result<Receipt, WriteError> {
        let receipt = Receipt {
            transcript_sha256: sha256(markdown.as_bytes()),
            publication,
        };
        match fs::remove_file(self.directory.join(JOB)) {
            Ok(()) => sync_directory(&self.directory).map_err(WriteError::BeforeRename)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(WriteError::BeforeRename(error)),
        }
        self.writer.json(&self.directory, RECEIPT, &receipt)?;
        self.writer
            .write(&self.directory, TRANSCRIPT, markdown.as_bytes())?;
        Ok(receipt)
    }

    pub(crate) fn write_projections(&self, projections: &Projections) -> Result<(), WriteError> {
        let receipt: Receipt =
            read_json(&self.directory.join(RECEIPT)).map_err(WriteError::BeforeRename)?;
        self.writer.json(
            &self.directory,
            PROJECTIONS,
            &ProjectionFile {
                transcript_sha256: receipt.transcript_sha256,
                projections: *projections,
            },
        )
    }

    pub(crate) fn apply_retention(&self) -> io::Result<()> {
        if self.manifest.keep_audio {
            return Ok(());
        }
        let receipt: Receipt = read_json(&self.directory.join(RECEIPT))?;
        if receipt.publication.fallback {
            return Ok(());
        }
        if !valid_transcript(&self.directory, &receipt)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "no committed transcript",
            ));
        }
        sync_directory(&self.directory)?;
        match fs::remove_dir_all(self.directory.join("audio")) {
            Ok(()) => sync_directory(&self.directory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn start_marks(&mut self) -> io::Result<MarkWriter> {
        MarkWriter::start(&self.directory, self.manifest.info.id)
    }
}

fn clean_failed_create(root: &Path, directory: &Path, error: io::Error) -> io::Error {
    match fs::remove_dir_all(directory).and_then(|()| sync_directory(root)) {
        Ok(()) => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!("{error}. Could not clean up the incomplete meeting bundle: {cleanup}"),
        ),
    }
}

fn lock_owner(lock: &File) -> io::Result<()> {
    retry_while_scanned(SCAN_LOCK_GRACE, || lock.try_lock()).map_err(lock_error)
}

fn retry_while_scanned(
    grace: std::time::Duration,
    mut attempt: impl FnMut() -> Result<(), TryLockError>,
) -> Result<(), TryLockError> {
    let deadline = std::time::Instant::now() + grace;
    loop {
        match attempt() {
            Err(TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            result => return result,
        }
    }
}

const SCAN_LOCK_GRACE: std::time::Duration = std::time::Duration::from_millis(50);

fn lock_error(error: TryLockError) -> io::Error {
    match error {
        TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
        TryLockError::Error(error) => error,
    }
}

fn read_manifest(directory: &Path, id: MeetingId) -> io::Result<Manifest> {
    let record: ManifestFile = read_json(&directory.join(MANIFEST))?;
    if record.version != 1 || record.manifest.info.id != id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "manifest version or id does not match the bundle",
        ));
    }
    Ok(record.manifest)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn read_optional_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match read_json(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[allow(dead_code)] // Used by meeting transcription and publication in S5/S6.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    digest_hex(digest(&SHA256, bytes))
}

fn digest_hex(hash: ring::digest::Digest) -> String {
    hash.as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_transcript(directory: &Path, receipt: &Receipt) -> io::Result<bool> {
    let mut file = match File::open(directory.join(TRANSCRIPT)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let mut context = Context::new(&SHA256);
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(digest_hex(context.finish()) == receipt.transcript_sha256);
        }
        context.update(&buffer[..count]);
    }
}

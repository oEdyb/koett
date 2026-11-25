use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

use serde::Serialize;

use super::MeetingId;

#[derive(Debug)]
pub(crate) enum WriteError {
    BeforeRename(io::Error),
    AfterRename(io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeRename(error) => write!(formatter, "write before rename: {error}"),
            Self::AfterRename(error) => write!(formatter, "directory sync after rename: {error}"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BeforeRename(error) | Self::AfterRename(error) => Some(error),
        }
    }
}

impl From<WriteError> for io::Error {
    fn from(error: WriteError) -> Self {
        let kind = match &error {
            WriteError::BeforeRename(error) | WriteError::AfterRename(error) => error.kind(),
        };
        Self::new(kind, error)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WriteBoundary {
    BeforeRename,
    AfterRename,
}

#[cfg(test)]
type Fault = dyn Fn(&str, WriteBoundary) -> io::Result<()> + Send + Sync;

#[derive(Default)]
pub(super) struct AtomicWriter {
    #[cfg(test)]
    pub(super) fault: Option<Box<Fault>>,
}

impl AtomicWriter {
    pub(super) fn json(
        &self,
        directory: &Path,
        name: &str,
        value: &impl Serialize,
    ) -> Result<(), WriteError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| WriteError::BeforeRename(io::Error::from(error)))?;
        self.write(directory, name, &bytes)
    }

    pub(super) fn write(
        &self,
        directory: &Path,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), WriteError> {
        let temporary = directory.join(format!(".{name}.tmp-{}", MeetingId::new()));
        write_temp(&temporary, bytes).map_err(WriteError::BeforeRename)?;
        let rename = || {
            #[cfg(test)]
            if let Some(fault) = &self.fault {
                fault(name, WriteBoundary::BeforeRename)?;
            }
            fs::rename(&temporary, directory.join(name))
        };
        if let Err(error) = rename() {
            let _ = fs::remove_file(temporary);
            return Err(WriteError::BeforeRename(error));
        }

        #[cfg(test)]
        if let Some(fault) = &self.fault {
            fault(name, WriteBoundary::AfterRename).map_err(WriteError::AfterRename)?;
        }
        sync_directory(directory).map_err(WriteError::AfterRename)
    }
}

pub(super) fn write_temp(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

pub(super) fn private_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}

pub(super) fn ensure_directory(path: &Path) -> io::Result<()> {
    match private_directory(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                })?;
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(error)?;
            ensure_directory(parent)?;
            ensure_directory(path)
        }
        Err(error) => Err(error),
    }
}

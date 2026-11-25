use std::ffi::CString;
use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
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
    #[cfg(test)]
    pub(super) fn checkpoint(&self, name: &str) {
        if let Some(fault) = &self.fault {
            let _ = fault(name, WriteBoundary::AfterRename);
        }
    }

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
        self.install(directory, name, bytes, |source, destination| {
            fs::rename(source, destination)
        })
    }

    pub(super) fn write_new(&self, directory: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
        self.install(directory, name, bytes, rename_new)
            .map_err(io::Error::from)
    }

    fn install(
        &self,
        directory: &Path,
        name: &str,
        bytes: &[u8],
        replace: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), WriteError> {
        let temporary = directory.join(format!(".{name}.tmp-{}", MeetingId::new()));
        write_temp(&temporary, bytes).map_err(WriteError::BeforeRename)?;
        let rename = || {
            #[cfg(test)]
            if let Some(fault) = &self.fault {
                fault(name, WriteBoundary::BeforeRename)?;
            }
            replace(&temporary, &directory.join(name))
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

fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // Both pointers refer to live, nul-terminated paths. The kernel checks for conflicts atomically.
    let status = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

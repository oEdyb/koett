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
        self.install(directory, name, bytes, 0o600, |source, destination| {
            fs::rename(source, destination)
        })
    }

    pub(super) fn write_export(
        &self,
        directory: &Path,
        name: &str,
        note: &str,
    ) -> Result<(), WriteError> {
        let note = note.as_bytes();
        clean_export_temps(directory).map_err(WriteError::BeforeRename)?;
        let candidates = note_names(name);
        for candidate in &candidates {
            if holds_note(&directory.join(candidate), note).map_err(WriteError::BeforeRename)? {
                return Ok(());
            }
        }
        let temporary = directory.join(format!("{EXPORT_TEMP}{}.tmp", MeetingId::new()));
        write_temp_with_mode(&temporary, note, 0o644).map_err(WriteError::BeforeRename)?;
        let placed = self.place_note(directory, &candidates, &temporary, note);
        if !matches!(placed, Ok(true)) {
            let _ = fs::remove_file(&temporary);
        }
        match placed {
            Ok(_) => {
                #[cfg(test)]
                if let Some(fault) = &self.fault {
                    fault(name, WriteBoundary::AfterRename).map_err(WriteError::AfterRename)?;
                }
                sync_directory(directory).map_err(WriteError::AfterRename)
            }
            Err(error) => Err(WriteError::BeforeRename(error)),
        }
    }

    fn place_note(
        &self,
        directory: &Path,
        candidates: &[String],
        temporary: &Path,
        note: &[u8],
    ) -> io::Result<bool> {
        #[cfg(test)]
        if let Some(fault) = &self.fault {
            fault(&candidates[0], WriteBoundary::BeforeRename)?;
        }
        for candidate in candidates {
            let target = directory.join(candidate);
            match rename_new(temporary, &target) {
                Ok(()) => return Ok(true),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if holds_note(&target, note)? {
                        return Ok(false);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Every note name for {} holds a different note",
                candidates[0]
            ),
        ))
    }

    fn install(
        &self,
        directory: &Path,
        name: &str,
        bytes: &[u8],
        mode: u32,
        replace: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), WriteError> {
        let temporary = directory.join(format!(".{name}.tmp-{}", MeetingId::new()));
        write_temp_with_mode(&temporary, bytes, mode).map_err(WriteError::BeforeRename)?;
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

const NOTE_COPIES: u32 = 9;

const EXPORT_TEMP: &str = ".koett-export-";

fn note_names(name: &str) -> Vec<String> {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    std::iter::once(name.to_owned())
        .chain((2..=NOTE_COPIES).map(|copy| format!("{stem} ({copy}).md")))
        .collect()
}

fn holds_note(path: &Path, note: &[u8]) -> io::Result<bool> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => return Ok(false),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Ok(false);
    }
    let mut bytes = Vec::with_capacity(note.len());
    io::Read::read_to_end(&mut io::Read::take(file, note.len() as u64 + 1), &mut bytes)?;
    Ok(bytes == note)
}

fn clean_export_temps(directory: &Path) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let koett_temp = entry.file_name().to_str().is_some_and(|name| {
            name.strip_prefix(EXPORT_TEMP)
                .and_then(|rest| rest.strip_suffix(".tmp"))
                .is_some_and(|id| MeetingId::parse(id).is_some())
        });
        if koett_temp && entry.file_type()?.is_file() {
            match fs::remove_file(entry.path()) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                result => result?,
            }
        }
    }
    Ok(())
}

pub(super) fn write_temp(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_temp_with_mode(path, bytes, 0o600)
}

fn write_temp_with_mode(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
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

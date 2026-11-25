use std::fmt;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use super::super::Offset;

pub(super) const DISK_WARNING: u64 = 1024 * 1024 * 1024;
const DISK_FLOOR: u64 = 256 * 1024 * 1024;
const DISK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaptureHealth {
    DiskLow { free_bytes: u64 },
    DiskHealthy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CaptureFaultReason {
    DiskLow { free_bytes: u64 },
    Io(String),
    WorkerPanicked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaptureFault {
    pub(crate) at: Offset,
    pub(crate) reason: CaptureFaultReason,
}

impl CaptureFault {
    pub(super) fn io(at: Offset, error: io::Error) -> Self {
        Self {
            at,
            reason: CaptureFaultReason::Io(error.to_string()),
        }
    }
}

impl fmt::Display for CaptureFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            CaptureFaultReason::DiskLow { free_bytes } => {
                write!(
                    formatter,
                    "Low disk space for meeting audio ({free_bytes} bytes free)"
                )
            }
            CaptureFaultReason::Io(issue) => formatter.write_str(issue),
            CaptureFaultReason::WorkerPanicked => {
                formatter.write_str("Meeting audio writer stopped unexpectedly")
            }
        }
    }
}

impl std::error::Error for CaptureFault {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CaptureNotice {
    Health(CaptureHealth),
    Fault(CaptureFault),
}

pub(super) struct DiskGuard {
    next_check: Instant,
    health: CaptureHealth,
}

impl DiskGuard {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_check: now + DISK_INTERVAL,
            health: CaptureHealth::DiskHealthy,
        }
    }

    pub(super) fn poll(
        &mut self,
        now: Instant,
        at: Offset,
        probe: impl FnOnce() -> io::Result<u64>,
    ) -> Result<Option<CaptureHealth>, CaptureFault> {
        if now < self.next_check {
            return Ok(None);
        }
        self.next_check = now + DISK_INTERVAL;
        let free_bytes = probe().map_err(|error| CaptureFault::io(at, error))?;
        if free_bytes < DISK_FLOOR {
            return Err(CaptureFault {
                at,
                reason: CaptureFaultReason::DiskLow { free_bytes },
            });
        }
        let health = if free_bytes < DISK_WARNING {
            CaptureHealth::DiskLow { free_bytes }
        } else {
            CaptureHealth::DiskHealthy
        };
        let changed = std::mem::discriminant(&self.health) != std::mem::discriminant(&health);
        self.health = health;
        Ok(changed.then_some(health))
    }
}

pub(crate) fn free_disk_bytes(path: &Path) -> io::Result<u64> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        use std::os::unix::ffi::OsStrExt;

        let path = CString::new(path.as_os_str().as_bytes())?;
        let mut info = MaybeUninit::<libc::statvfs>::uninit();
        // statvfs initializes the buffer on success.
        unsafe {
            if libc::statvfs(path.as_ptr(), info.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            let info = info.assume_init();
            let bytes = u128::from(info.f_bavail) * u128::from(info.f_frsize);
            Ok(u64::try_from(bytes).unwrap_or(u64::MAX))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

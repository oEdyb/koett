//! Keep the optional formatter invisible and bound to Koett's process lifetime.
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::Child;
use windows::Win32::{
    Foundation::HANDLE,
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
};
pub(crate) struct Job {
    handle: OwnedHandle,
}
impl Job {
    pub(crate) fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|e| e.to_string())?;
        let job = Self {
            handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                HANDLE(job.handle.as_raw_handle()),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(|e| e.to_string())?;
        Ok(job)
    }
    pub(crate) fn supervise(self, mut child: Child) -> Result<(Child, Self), String> {
        if let Err(error) = self.attach(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Could not supervise S1-mini: {error}"));
        }
        Ok((child, self))
    }
    pub(crate) fn attach(&self, child: &Child) -> Result<(), String> {
        unsafe {
            AssignProcessToJobObject(
                HANDLE(self.handle.as_raw_handle()),
                HANDLE(child.as_raw_handle()),
            )
        }
        .map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::windows::process::CommandExt, process::Command};
    #[test]
    fn closing_formatter_job_terminates_its_child() {
        let job = Job::new().unwrap();
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
            .spawn()
            .unwrap();
        job.attach(&child).unwrap();
        drop(job);
        assert_eq!(
            unsafe {
                windows::Win32::System::Threading::WaitForSingleObject(
                    HANDLE(child.as_raw_handle()),
                    3000,
                )
            },
            windows::Win32::Foundation::WAIT_OBJECT_0
        );
        child.wait().unwrap();
    }
}

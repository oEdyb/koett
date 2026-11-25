use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdout, Command, ExitStatus};

pub(crate) struct ChildGroup(Child);

impl ChildGroup {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        command.process_group(0).spawn().map(Self)
    }

    pub(crate) fn spawn_parent_bound(command: &mut Command) -> io::Result<Self> {
        let parent = std::process::id();
        // The parent may die between fork and prctl. Check again after installing the signal.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid().cast_unsigned() != parent {
                    return Err(io::Error::from(io::ErrorKind::Interrupted));
                }
                Ok(())
            });
        }
        Self::spawn(command)
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.0.try_wait()
    }

    pub(super) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.0.stdout.take()
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        if let Ok(group) = i32::try_from(self.0.id()) {
            unsafe { libc::killpg(group, libc::SIGKILL) };
        }
        let _ = self.0.wait();
    }
}

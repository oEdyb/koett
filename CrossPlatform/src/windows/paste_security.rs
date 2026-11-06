use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, IsValidSid,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel,
    },
    System::Threading::{OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION},
};

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn integrity(pid: u32) -> Result<u32, String> {
    let process = OwnedHandle(
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| format!("Cannot inspect paste target: {e}"))?,
    );
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) }
        .map_err(|e| format!("Cannot inspect paste permissions: {e}"))?;
    let token = OwnedHandle(token);
    let mut required = 0;
    let _ = unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &mut required) };
    if required < std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 || required > 65_536 {
        return Err("Paste target permissions are unavailable".into());
    }
    let mut storage = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            Some(storage.as_mut_ptr().cast()),
            required,
            &mut required,
        )
    }
    .map_err(|e| format!("Cannot read paste permissions: {e}"))?;
    let label = unsafe { &*storage.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
    if !unsafe { IsValidSid(label.Label.Sid) }.as_bool() {
        return Err("Paste target permissions are invalid".into());
    }
    let count = unsafe { *GetSidSubAuthorityCount(label.Label.Sid) };
    if count == 0 {
        return Err("Paste target has no integrity level".into());
    }
    Ok(unsafe { *GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)) })
}
fn allowed(sender: u32, target: u32) -> bool {
    target <= sender
}
pub(super) fn check(target_pid: u32) -> Result<(), String> {
    if allowed(integrity(std::process::id())?, integrity(target_pid)?) {
        Ok(())
    } else {
        Err("Elevated app: use Copy and paste manually.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elevated_targets_are_rejected_before_input() {
        assert!(!allowed(0x2000, 0x3000));
        assert!(!allowed(0x2000, 0x4000));
        assert!(allowed(0x2000, 0x2000));
        assert!(allowed(0x2000, 0x1000));
    }
    #[test]
    fn actual_current_process_token_is_readable() {
        assert!(integrity(std::process::id()).unwrap() >= 0x1000);
        check(std::process::id()).unwrap();
        assert!(check(0).is_err());
    }
}

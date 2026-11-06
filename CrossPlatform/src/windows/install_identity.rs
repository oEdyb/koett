use std::path::{Path, PathBuf};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::EnhancedStorage::{
    PKEY_AppUserModel_ID, PKEY_AppUserModel_RelaunchCommand,
    PKEY_AppUserModel_RelaunchDisplayNameResource, PKEY_AppUserModel_RelaunchIconResource,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};

fn candidate(executable: &Path) -> Option<PathBuf> {
    let version = executable.parent()?;
    let key = version.file_name()?.to_str()?;
    if key.len() != 16 || !key.bytes().all(|value| value.is_ascii_hexdigit()) {
        return None;
    }
    let versions = version.parent()?;
    if versions.file_name()? != "versions" {
        return None;
    }
    Some(versions.parent()?.join("Koett.exe"))
}

pub(super) fn launcher() -> Option<PathBuf> {
    candidate(&std::env::current_exe().ok()?).filter(|path| path.is_file())
}

pub(super) fn configure(window: HWND) -> windows::core::Result<()> {
    let Some(launcher) = launcher() else {
        return Ok(());
    };
    let icon = launcher.with_file_name("Koett.ico");
    let values = [
        (
            PKEY_AppUserModel_RelaunchCommand,
            format!("\"{}\"", launcher.display()),
        ),
        (
            PKEY_AppUserModel_RelaunchDisplayNameResource,
            "Koett".into(),
        ),
        (
            PKEY_AppUserModel_RelaunchIconResource,
            format!("{},0", icon.display()),
        ),
        (PKEY_AppUserModel_ID, "DybergCo.Koett".into()),
    ];
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(window)?;
        for (key, value) in values {
            store.SetValue(&key, &PROPVARIANT::from(value.as_str()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_versioned_installs_use_the_root_launcher() {
        assert_eq!(
            candidate(Path::new(
                r"C:\Users\Lab\Programs\Koett\versions\0123456789abcdef\Koett.exe"
            )),
            Some(PathBuf::from(r"C:\Users\Lab\Programs\Koett\Koett.exe"))
        );
        assert_eq!(candidate(Path::new(r"C:\Portable\Koett.exe")), None);
        assert_eq!(
            candidate(Path::new(r"C:\Koett\versions\other\Koett.exe")),
            None
        );
    }
}

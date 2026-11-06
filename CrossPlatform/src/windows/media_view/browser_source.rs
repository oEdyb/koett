use windows::{
    Win32::{
        System::Com::*,
        UI::{
            Accessibility::*,
            WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        },
    },
    core::Interface,
};
pub(super) fn current() -> Option<String> {
    current_for(unsafe { GetForegroundWindow() })
}
pub(super) fn current_for(expected: windows::Win32::Foundation::HWND) -> Option<String> {
    if unsafe { GetForegroundWindow() } != expected {
        return None;
    }
    let mut expected_pid = 0;
    unsafe {
        GetWindowThreadProcessId(expected, Some(&mut expected_pid));
    }
    let snapshot = crate::windows::pin_reader::Snapshot::foreground()?;
    let read = snapshot.read_source();
    let url = read.context?.url?;
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let automation: IUIAutomation2 =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).ok()?;
        automation.SetConnectionTimeout(100).ok()?;
        automation.SetTransactionTimeout(100).ok()?;
        let automation: IUIAutomation = automation.cast().ok()?;
        let walker = automation.ControlViewWalker().ok()?;
        let mut element = automation.GetFocusedElement().ok()?;
        if GetForegroundWindow() != expected
            || element.CurrentProcessId().ok()? as u32 != expected_pid
        {
            return None;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(800);
        for _ in 0..32 {
            if std::time::Instant::now() > deadline || element.CurrentIsPassword().ok()?.as_bool() {
                return None;
            }
            if element.CurrentControlType().ok()? == UIA_DocumentControlTypeId {
                let value = element
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                    .ok()?
                    .CurrentValue()
                    .ok()?
                    .to_string();
                let focused = crate::windows::pin_policy::web_url(&value)?;
                return (focused == url && GetForegroundWindow() == expected).then_some(url);
            }
            element = walker.GetParentElement(&element).ok()?;
        }
    }
    None
}

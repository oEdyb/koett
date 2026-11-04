# Isolated UIA focus probe

This is a separate managed Windows/Win32 diagnostic process, **not the Koett
executable**. Native HWND-backed Button Invoke activated the probe window.
The custom parent UIA fragment root with a virtual Button/Invoke child and
ProviderOwnsSetFocus preserved the editor HWND 262172 / PID 9036. Its log
records virtual SetFocus and Invoke without foreground activation.

The production Rust implementation follows this model, keeps DirectComposition
rendering and parent hit-testing, and removes the native child HWNDs and the
ineffective attempt to restore focus afterward. It still needs a new linked
CI executable runtime test. Settings and History retain native controls.

The raw COM adapter initializes nullable outputs explicitly. The generated
Result<Interface> traits cannot represent the required successful NULL result.
Tests check ABI GUID/vtable compatibility, NULL output initialization, and
visible action state/labels. Cached providers are disconnected at teardown.

Official contracts:
- https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-serversideprovider
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.provider.irawelementproviderfragment.setfocus
- https://learn.microsoft.com/en-us/windows/win32/api/uiautomationcore/nf-uiautomationcore-irawelementprovidersimple-getpatternprovider

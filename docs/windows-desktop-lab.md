# Windows desktop lab

Run `Koett.exe --desktop-lab` in an isolated Windows user profile. This explicitly
skips the speech engine/model download and starts the real native desktop shell.
It is a visual/accessibility fixture, not evidence of microphone or ASR behavior.
Normal launches ignore the fixture message entirely.

Send `WM_APP + 0x52` (`0x8052`) to class `KoettOwnerWindow` with wParam:

| Value | Action |
| --- | --- |
| 1 | Start silent recording UI, preserve current foreground target |
| 2 | Transcribing UI with frozen strip |
| 3 | Done with fixture text |
| 4 | Persistent not-pasted fixture with Copy/Paste |
| 5 | Persistent rescue error |
| 6 | Five-second error |
| 7 | Hide pill |
| 8 | Open real native Settings |
| 9 | Open real native History |
| 10 | Toggle paused UI |

Take a guest screenshot and foreground HWND/PID before and after every step.
Test ordinary startup and tray behavior with a normal launch separately.
The lab does not simulate speech recognition, delivery acknowledgments or save
success. Use actual recordings and fault injection in Milestone 2 for those gates.

Pill action controls are owner-drawn native Win32 BUTTONs, so standard Windows
accessibility providers expose their labels and Invoke behavior. Their runtime
Narrator behavior and focus preservation still require guest verification.

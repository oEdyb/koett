# Mac permission prompts

What macOS asks for, why it stays granted, and what Koett cannot avoid.
Measured on macOS 26.6 (Darwin 25.6) on 2026-11-02 from the `com.apple.TCC`
and `replayd` unified logs across three reinstalls (18:57, 19:15, 19:17).

## Grants survive reinstalls

macOS stores each grant against the app's bundle ID and its designated
requirement (`codesign -d -r- /Applications/Koett.app`). With Developer ID the
requirement names the team, not the build:

```
identifier "com.olledyberg.Koett" and anchor apple generic and
certificate 1[field.1.2.840.113635.100.6.2.6] and
certificate leaf[field.1.2.840.113635.100.6.1.13] and
certificate leaf[subject.OU] = LF8KF3G42Q
```

Every rebuild signed with `Developer ID Application: Dyberg & Co AB` keeps its
Microphone, Accessibility, Input Monitoring, Post Event and Screen Recording
grants. On 2026-11-02 every Koett TCC check after each reinstall returned
allowed, with no prompt.

Switching identity (Developer ID, either Apple Development certificate, or
ad-hoc) changes the requirement, and macOS then asks for every permission
again. `install-macos.sh` refuses that switch unless
`KOETT_ALLOW_SIGNATURE_CHANGE=1` is set.

## What Koett asks for, and when

- Microphone, Accessibility, Screen Recording: only from the permissions
  window's Allow button, once. Startup and the menu check silently and open
  System Settings instead of raising a second system prompt.
- The permissions window polls status with `AVCaptureDevice`,
  `AXIsProcessTrusted` and `CGPreflightScreenCaptureAccess`. None of them
  prompt. Its ScreenCaptureKit probe runs only after Koett has asked and the
  preflight still reads false.
- Automation (Apple Events): only media transcription, through `osascript`,
  once per browser. Screen-context pins read the tab URL through Accessibility
  (`AXWebArea` `AXURL`), so they add no prompt.

## What macOS forces

- **ScreenCaptureKit picker-bypass alert.** The first time Koett captures the
  screen without the system picker (drawing, Ask), macOS shows "Koett is
  requesting to bypass the system private window picker and directly access
  your screen and audio." It returns about once a month. It is keyed to the
  bundle ID and survived the 19:17 reinstall. Apple's only exemptions are
  `SCContentSharingPicker` (the user picks a window or display per capture,
  which breaks draw-while-dictating) or the
  `com.apple.developer.persistent-content-capture` entitlement, which Apple
  grants to screen-sharing products. Koett qualifies for neither. Choose Allow.
- **Screen Recording after a first grant** usually applies only to a new
  process. The permissions window offers Relaunch.
- **A new Automation prompt per browser** the first time media transcription
  targets it.

## Diagnose a prompt

```sh
/usr/bin/log show --last 1h --predicate 'subsystem == "com.apple.TCC" AND eventMessage CONTAINS "AUTHREQ_PROMPTING"' --style compact
/usr/bin/log show --last 1h --predicate 'process == "replayd" AND eventMessage CONTAINS "SCAlert"' --style compact
/usr/bin/log show --last 1h --predicate 'process == "securityd" AND eventMessage CONTAINS "displaying keychain prompt"' --style compact
```

The first names the app and service of each TCC prompt, the second shows
ScreenCaptureKit alerts, and the third shows Keychain dialogs. Call `log`
through `/usr/bin/log`; zsh has a builtin with the same name.

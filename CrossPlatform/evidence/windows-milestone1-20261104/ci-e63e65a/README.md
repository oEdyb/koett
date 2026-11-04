# Unmodified CI artifact e63e65a — partial runtime checks

Windows 11 evaluation guest, KVM, 4 vCPU/8 GiB, 2560×1600 display.
Archive SHA-256 matches the included CI receipt. Executable SHA-256:
`f855664b41103048b045b135e4221f54881ec5531224aa7af86b01aed5c04644`.

The unmodified artifact renders a visible 600×96 physical-pixel pill at
200% scaling (300×48 logical pixels). It preserves foreground HWND 197274,
PID 7408 for all 20 typed characters and each fixture state. The editor
contains the typed phrase. Done hides after the 2.1-second observation.
Screenshots cover recording, transcribing, done, not-pasted, rescue error,
transient error, paused, drawing glyph and media result. These are desktop
lab fixtures; they do not establish working ASR, media processing or ink.

Narrator PID 10216 runs while keyboard Tab visits Start at login (CheckBox),
Save (Button), Change shortcut (Button), and returns to Start at login.
Foreground remains Koett HWND 328626, PID 5976. This establishes native
keyboard/accessibility focus and names with Narrator active. Spoken output
has not yet been listened to, so this is not a complete Narrator pass.

Remaining issues: waveform uses coral instead of Mac gray/dim bars; source
fix pending a rebuilt artifact. Dark mode, reduced motion, visible states
at 100/150%, tray menu actions, startup-off reboot, and History export still
need completion. PR remains draft.

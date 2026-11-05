# Mac automatic paste readiness

## Cause addressed

The previous Mac guard sampled combined session modifier flags and the focused
AX window once. Any held modifier or temporary AX window read failure immediately
rejected the paste, even if the same target became ready a moment later. All such
rejections returned the same Boolean. The LaunchServices app's stdout/stderr were
`/dev/null`, so exact reasons for historical reports cannot be reconstructed from
its past print statements. No historical failure is attributed to a particular
cause without a retained record.

Apple's [event-source state documentation](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid)
distinguishes hardware state from combined software/hardware state and provides
independent state tables. Koett now creates paste events with a private source
and gates on hardware modifier flags. Combined session flags remain diagnostics;
Koett's synthetic Command/Control is not treated as a physical held key.

## Behavior

- Wait up to 1.5 seconds, asynchronously, for physical modifiers to release or a
  temporarily unavailable original AX window to respond. Poll every 20 ms; each
  AX message is still bounded. Do not block audio, the menu or cancellation.
- Recheck the original process/launch identity and available original window on
  every poll and immediately before posting. Focus/window changes, missing
  targets, Koett focus and denied Accessibility/post-event permission reject
  immediately. No permission request API is invoked.
- The initial wait happens before claiming delivery, so Escape/Cancel can still
  discard everything. Drawing text-stage readiness is checked separately after
  the image read. A newer clipboard owner still wins during that wait.
- Post once after readiness. Preserve paste-before-storage ordering, one save,
  raw text/ASR recovery boundaries and ephemeral successful audio.
- Rejections keep Not pasted + Copy/Paste recovery. The menu's latest result
  records the actual reason. The pill's approved 300×48 layout/timing is unchanged.
- Manual Paste Last uses the same bounded readiness and original-target guard.

## Inspect exact future reasons

`/Applications/Koett.app/Contents/MacOS/koett --control status` includes the latest
`paste` object, current `pasteAccessGranted` preflight and an optional `pasteLogError`. `posted` means events were posted;
it is not proof the target consumed text. macOS supplies no universal paste receipt.
A later application-side rejection cannot be honestly reported as a confirmed paste.

Private rotating JSON-lines logs:

```
~/Library/Application Support/Koett/Diagnostics/Paste.jsonl
~/Library/Application Support/Koett/Diagnostics/Paste.previous.jsonl
```

Each file rotates around 256 KiB. Writes run on a utility queue after the
in-memory outcome is recorded. Files are 0600; their directory is 0700. Entries
contain timestamp, stage, posted/blocked outcome, reason, wait duration/reason,
physical/combined flags and expected/current application IDs/PIDs. No dictated
text, clipboard bytes, screenshots, window titles, URLs or ordinary typed keys
are logged. Test controllers default to no on-disk log; tests inject a temporary
directory explicitly.

Reasons: accessibilityDenied, noTarget, focusChanged, ownApp, windowChanged,
targetNotReady, modifiersHeld, eventCreationFailed, clipboardChanged,
clipboardUnavailable, cancelled. Newer-copy preservation is not overwritten to
force delivery. Permanent rejection is explicit instead of an unexplained no-op.

## Verification

Deterministic fixtures cover held-key release, transient AX readiness, focus
changes while waiting and before posting, bounded timeout, missing permission,
synthetic session flags, private event-source creation, cancellation, persistent
recovery reason, private rotating metadata logs and newer clipboard ownership
during the drawing wait. No real transcript or user-facing injected keystroke
is used. These fixtures reproduce the one-shot rejection class; they do not
identify which historical dictations encountered it.

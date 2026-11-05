# Native M5 controlled-input Stop → target audit

This harness changes **private source copies only**. It is not part of the Koett
product target. The original source commits are `c72f83415b1d760c30316f07fc776f09b0ebdc0b`
and `85dde5153e0fd4cf50ed3b91813ffc9166d779fc`. The latter includes the proposed
CTC-default change. These are instrumented source builds, not the released binary.

## Endpoint and input seam

The start timestamp is taken immediately before the production controller's
`toggleRecording` Stop call. The end is recorded immediately after the **foreign
process's real `NSTextView.paste` calls `super.paste`**. Clipboard reads, receipt
serialization and clipboard restoration happen after that timestamp.

Both source versions still use their real recorder, file writer, Stop/close,
production transcription/preparation/delivery, general pasteboard and actual
Command-V events. No decoder, text result, paste event or target insertion is
stubbed. No separately measured ASR and paste durations are added together.

The controlled seam replaces PCM **before recording-file writes**:

- Old: a real `AudioQueueProcessingTap` replaces input samples before encoding.
  Its queue handle is obtained from the stopped `AVAudioRecorder` implementation,
  at a probe-verified private offset. The helper refuses builds other than macOS
  26.6.2 / `25G83`; this private access must never enter production.
- Current: the live `MicrophoneRecorder` input callback replaces samples directly
  before its existing `MicrophoneFileSink.append` call.
- Both: the same SHA-verified public 16 kHz WAV is loaded into memory. The same C
  generator supplies 300 ms of leading zeroes, then the fixture, then zeroes.
  It linearly resamples for the current recorder's native 48 kHz input. Old input
  is 16 kHz. Actual production file formats and post-Stop conversion stay intact.
- Capture runs on the real live input clock. Stop occurs after the generator has
  delivered the entire fixture, plus 150 ms and the pre-Stop modifier normalization.
  Per-receipt injected frames and sample rate are retained.

The old processing tap can affect Audio Queue buffering; its observed maximum
processing block is 1024 frames. The current input callback keeps its existing
buffer allocation and file sink. No claim of zero instrumentation cost or exact
unmodified-release buffer behavior is made.

Thus this measures **native M5 controlled-input Stop → insertion**, excluding
microphone acquisition, room acoustics and physical keyboard dispatch. It does
not establish unmodified shipping-binary microphone latency, compositor rendering,
a T3/browser/rich-image paste endpoint or the unknown August 18 filmed commit.

## Other instrumentation and exclusions

`old-source.patch` and `current-source.patch` contain every change to tracked
product files; `AuditFixture.swift` is the one added source file. They:

1. Inject memory-only `AuditDefaults` and an isolated Foundation home. `CFFIXED_USER_HOME`
   alone did not isolate the preferences daemon, so both typed reads and writes
   are handled in memory. Current permission-onboarding storage is also injected.
2. Require explicit `KOETT_AUDIT_HOST=1` and the exact private home before startup.
3. Start the existing delegate once and add file-based Start/Stop/progress/abort
   controls. The deep private home exceeds the existing production control
   socket path limit; that CLI control endpoint is not tested. These controls
   invoke the real production toggle method; startup/loading are
   outside warm timing. Source Stop methods themselves are unchanged.
4. Normalize synthetic modifier state before the timed Stop. This models released
   shortcut keys without overriding current production paste-readiness checks.
5. Tag the real paste key-down event with the exact clipboard write generation.
   The old build also refuses delivery unless the owned receiver PID has focus.
6. Keep raw formatting, empty dictionary/replacements, drawing/pins and media pause
   off. This is not an optional-cleanup or drawing performance benchmark.

The native target is a separate regular AppKit process with an alpha-zero window.
WindowServer may constrain its coordinates; **zero alpha**, not its requested
negative coordinates, makes it invisible. It uses the ordinary Edit/Paste responder
chain. Its ephemeral Dock presence is not an installed app.

The collector performs three warm repetitions per length/build. Build order
alternates old/current, current/old, old/current. Each group starts a private app,
warms the model and a complete five-second pipeline, then records 5/30/120/600 s.
It does not infer performance from the earlier guest observations.

## Freshness and host-state guards

The original general clipboard representations are held **only in receiver memory**.
Before every capture the receiver is emptied and the clipboard receives a fresh
unrelated UUID sentinel. Acceptance requires all of:

- a target timestamp after this Stop; the request UUID is driver bookkeeping,
  not independent source-to-receiver correlation;
- real paste-event PID equal to this private source app;
- event-tagged app write generation equal to clipboard generation, newer than sentinel;
- exactly equal receiver/clipboard payload and the expected **spoken** marker;
- matching driver/loaded-input/manifest hashes and a completed input generation;
- reference-completed text length, fixture-specific final phrases and paired
  build/repeat payload lengths within 1%, checked by `verify-host.py`;
- source capture generations in order 2/3/4/5, after warm-up generation 1;
- successful clipboard restoration, compared representation-for-representation in memory.

Nothing adds a nonce to output text. Repeated identical audio cannot satisfy this
probe through an unchanged clipboard. Physical input-clock progress is verified
separately from clipboard generation and payload length.

During capture the receiver checks HID idle time for unexpected host input and
aborts on loss of owned-target focus. It never types into another visible app.
Cleanup restores only an owned clipboard generation; newer external copies win.
It restores previous app focus only while the receiver still has focus. The
collector holds the shared benchmark file lock and a temporary sleep assertion.
No host audio driver is installed and no system audio route is changed by this seam.
The receiver must be allowed to finish cleanup before it is closed, because its
clipboard backup is intentionally never written to disk.

## Reproduce in a newly authorized exclusive host slot

Use only a confirmed unattended Mac with existing microphone/Accessibility/event
permissions for the matching Developer ID requirement. Coordinate all other builds,
model work and VMs before running. The prepare command **builds only**; it does not
install, launch, grant permissions or alter the installed application.

Generate the existing public fixtures and marked-fixture manifest first using
`Benchmarks/Dictionary/prepare-latency.py` and `../prepare-guarded.py`. Keep the
same frozen WAV files for both builds. The runner expects them in
`.build/performance/guarded-fixtures/`.

```sh
python3 Benchmarks/Dictionary/PasteAudit/Host/prepare-host.py \
  --identity '<existing matching Developer ID identity>' \
  --model-cache "$HOME/Library/Application Support/FluidAudio"
python3 .build/performance/host-repro/run-host.py
```

Verify all 24 receipts independently after the collector finishes:

```sh
python3 Benchmarks/Dictionary/PasteAudit/Host/verify-host.py \
  .build/performance/host-repro/native-paste.json \
  .build/performance/guarded-fixtures/manifest.json
```

The driver, app state, native receipts, warm-ups and logs remain under that private
root. The source bundles are never installed. After verifying cleanup, unregister
only these private bundle paths from Launch Services and leave the installed app
and its data alone. Never copy a Koett History, Last Transcript or real dictionary
into the test home or report.

## Instrumentation and review limits

Both private source apps are hardened but have **no audio-input entitlement**.
Only the receiver uses `driver.entitlements`. Existing authorization therefore
does not prove physical microphone acquisition; buffers can be silent before
replacement. That acquisition is excluded. No new host permission is granted.
Future capture protocols should log authorization state before starting.

Old uses the system default input; current selects the built-in microphone.
A read-only host query during the suite returned `BuiltInMicrophoneDevice` as
the default input UID. Sample rate alone does not identify the physical device.
The harness memory defaults override the shortcut to Control-Shift-K, but host
trials invoke file controls, so neither shortcut dispatch is measured.

The old-only processing tap is destroyed with the queue during real recorder
Stop. Its extra teardown is **inside** the timer and was not measured separately;
this can bias against old. Small five-second differences cannot support a faster
shipping-app verdict. The modifiers were already up before each measured Stop;
the production 1.5-second modifier-wait branch is not exercised. Stop trials use
empty dictionaries; populated correction cost is measured separately.

Power/thermal state and the installed Koett event-tap state were not recorded.
Lengths are in fixed order and there are only three repetitions per cell.
Cached CTC files are preserved. The strengthened verifier was added after the
live collector started and checks every saved receipt before final acceptance.

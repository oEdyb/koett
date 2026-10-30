# Linux spoken Ask increment

Spoken Ask is optional and defaults off. Katie is the default; the five voice IDs,
endpoint, API version and Sonic model match `CartesiaSpeechOutput.swift`.
This increment does not complete the Linux port or establish appearance approval.

The runtime prewarms a validated TLS WebSocket during question capture. Text
updates reach the panel before the speech lead buffer processes each delta.
The lead accepts only the opening paragraph, up to three complete sentences,
removes Markdown and URLs, and stops at code fences. Final incomplete opening
text is flushed when the answer finishes. Speech failures have an independent
panel notice; they do not replace the answer or disable Copy.

Configuration is `speech.json` (enabled and voice only). The Cartesia key is a
separate Secret Service item under `com.olledyberg.Koett.Speech`, provider
`cartesia`. Upstream Tungstenite traces raw handshake bytes. Linux pins log 0.4.34 with
compile-time warning/error-only logging (including debug builds), and marks the
key header sensitive. This deliberately removes debug/trace diagnostics from
log-based dependencies so enabling a logger cannot expose authentication.
No plaintext fallback, key arguments, key URL parameters, provider
error bodies or audio files are used. UI and CLI call the same configuration
and Secret Service functions:

```
koett ask-speech-status
koett ask-speech-configure < speech.json
koett ask-key cartesia < secret
```

Speech uses pinned tokio-tungstenite 0.30.0 (registry MSRV 1.85), rustls native
roots, UUID 1.29.0 (registry MSRV 1.89), and existing CPAL 0.18.2. Native playback is mono Float32 at 44100 Hz.
The input channel holds eight items; the audio queue holds three seconds.
Messages, decoded chunks and total duration are bounded. Context IDs, base64,
frame alignment, sample rate when provided, and finite normalized samples are
validated. Backpressure has a deadline. Handshake, send, stalled receive, idle
prewarm and playback drain have deadlines. Cancellation drops the socket and
CPAL stream, clears the queue and joins the worker. Each operation owns its
queue and context, preventing late audio from entering another operation.

## Evidence boundaries

Evidence directory: `CrossPlatform/evidence/parity-speech-20261030`.
The isolated fixture uses a private CA, local WSS server, private Secret Service,
and actual CPAL output into `agent_lab_audio`. It asserts exact voice/model/
context/output format, connection before text, measured nonzero PCM, then silence.
The cancellation variant asserts worker return within 150 ms and samples the
actual queue to confirm it stays within the three-second bound.
No real Cartesia key, paid request, physical audio or user transcript is needed.

**Cartesia live compatibility is unverified.** The provider's current docs show
`sonic-latest` examples and both string and structured voice representations;
this implementation preserves the exact Mac model and structured voice request.
Private protocol acceptance is not evidence that the account supports that model.
Native missing-key acceptance passed real local ASR, one portal frame, private
HTTPS SSE, full/partial exact Copy, permission recovery, cancellation and later
dictation. Native speech component acceptance passed private WSS playback,
text-before-audio, stripped lead, exact clipboard Copy, cancel and provider-error
retention. Light/dark/narrow/keyboard settings and native playing/cancel/error
captures were inspected. These runs used the explicit plain renderer fallback;
they do not repeat the earlier isolated rich-WebKit acceptance. No live Cartesia
request or appearance approval is claimed. A native cancel/prewarm/cancel replacement run observed silence after cancel
and both server connections closing. Cancellation during handshake and first
audio waits passed the same 150 ms worker-join bound. Handshake, first-audio
and stalled-read timeout fixtures passed. Physical-device
behavior and live Cartesia remain outside private acceptance.

Primary references checked during implementation:
- https://docs.cartesia.ai/api-reference/tts/websocket
- https://docs.cartesia.ai/api-reference/tts/working-with-web-sockets/context-flushing-and-flush-i-ds
- https://github.com/cartesia-ai/cartesia-python/blob/main/README.md
- https://docs.rs/tokio-tungstenite/0.30.0/tokio_tungstenite/

Flush IDs are not needed: this client has a single context per connection and
never multiplexes sentences into separate playback contexts.

## Replay inside the private lab

Run the new Python protocol fixture with `agent-lab run koett-speech -- python3
.../CrossPlatform/scripts/test_linux_speech.py`. `KOETT_SPEECH_CANCEL=1` tests
bounded cancellation; `KOETT_SPEECH_NATIVE=1` tests the actual Ask panel plus
speech and clipboard. Add `KOETT_SPEECH_ERROR=1` for provider failure. Native
captures refuse overwriting prior captures; set `KOETT_SPEECH_EVIDENCE` to a
fresh directory before a repeat. `KOETT_SPEECH_TIMEOUT_MODE=handshake|first_audio|stalled_read` exercises deadline
closure; it can be combined with `KOETT_SPEECH_CANCEL=1` to interrupt a wait.
The endpoint override exists only under Rust `cfg(test)` and
is restricted to localhost WSS; the product has no endpoint override.

The existing `test_linux_ask_acceptance.py` accepts `--speech-missing-key` to
repeat native Ask with speech enabled and no Cartesia key. It asserts the
independent notice alongside the completed exact Copy answer.

## Parent review follow-up

Two fresh reviews found Cancel stopped actual audio but left Speaking visible;
the component fixture manually cleared it and masked production behavior.
Parent cancellation events now clear speech status and AskView also clears it
on the Cancelled phase. The native fixture no longer supplies the extra clear.
Focused speech tests pass; strict Clippy and Windows source checks were rerun.
The native cancellation fixture needs a fresh run before final acceptance.

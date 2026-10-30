# Full Linux parity with Koett on macOS

Authorized by Olle on 2026-10-29: port every user-facing Mac feature, and use the
actual Mac product as the interaction/visual reference. The prior GTK restyle
was explicitly rejected. Passing correctness gates does not establish visual approval.

## Completion contract

1. Linux exposes installed/downloadable local models and safe idle switching,
   showing exact active selection/progress/errors. No JSON editing is required.
2. Raw/local S1-mini output and all official style/structure/context controls work,
   preserving raw/final text and raw fallback on any cleanup failure.
3. Ask records a local voice question, explicitly includes one in-memory screen
   capture, streams sanitized rich answers, supports providers/model/endpoint/key
   configuration and cancellation, and offers optional interruptible spoken replies.
4. Media transcription accepts supported media, bounds fetch/conversion/cancellation,
   uses warm local ASR, saves source metadata/results and retains exact recovery.
5. All Mac user-facing menu/settings/shortcut/recovery/permission/setup actions
   have working Linux counterparts. Platform-specific engines must be proven on
   Linux or documented as blocked with a precise reason, never simulated.
6. The compact menu/formatting/settings/Ask surfaces follow actual Mac source and
   available actual captures: hierarchy, geometry, typography, controls, waveform.
   Native screenshots must show a substantive change from the rejected GTK pass.
7. Critical journeys pass in private agent-lab with isolated state/audio/input;
   exact text/recovery, cancellation, privacy and capture ownership remain intact.
   Required Rust/platform checks and two fresh held-out reviews precede shipping.

Canonical behavior: docs/feature-contracts.md and Sources/Koett/, including
KoettMenu, FormattingPopover, RecordingOverlay, AssistantPanel and HistoryWindow.
Inventory each source feature as implemented / pending / verified / blocked,
with source pointers and real proof. Model experiments are not installed defaults.
Do not change selected defaults without an explicit user model selection.

## Execution order

- First: model management and the compact Mac-referenced control surface.
- Then: local formatting, bounded media processing, Ask/screen/provider integration,
  optional speech and experimental engine parity, with narrow ownership boundaries.
- Finally: exhaustive parity inventory, real native UI/critical-journey proof,
  independent review, idle-safe installation and feature-branch publication.

Use existing Rust/GTK platform seams unless research proves they cannot serve the
visual/interaction requirements. No wholesale framework replacement by assumption.
New dependencies must be exact-pinned and researched; preserve ASR-only native
build receipts and Windows checking. Cloud/API keys go in the Linux secret service,
never settings JSON/logs/evidence. No host GUI/audio/input/clipboard tests, no
reading private transcripts, no sending live screenshots or audio to providers.
Public fixtures/local protocol fixtures can establish wiring without paid requests;
real external acceptance requiring credentials remains explicitly unverified.

## Guidance provenance

Fetched oEdyb/skills origin/main at b8028e7 on 2026-10-29. The development router,
workflow and frontend guidance have no committed difference from the local HEAD;
local uncommitted guidance remains preserved. Applied the fetched frontend guide
and existing local Linux playbook. The Mac app, not generic admin references,
is the primary reference for this authorized parity task.

## First increment locally implemented and verified — 2026-10-29

Local model catalog, verified v2 preparation, idle switching with retained previous
worker, saved explicit selection and active identity are implemented in the shared
feature checkout. Compact model/settings controls retain GTK and add no future
feature placeholders. See `linux-parity-inventory.md` for source mapping, exact
remaining parity and verification status. Full Linux parity remains incomplete.

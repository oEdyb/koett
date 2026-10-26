# Koett: Linux port, native UI and searchable history

Status: researched proposal, awaiting implementation-goal approval. As of 2026-10-26.
Context and source receipts: [research.md](research.md).

## Outcome

Deliver usable Koett on GNOME Wayland, KDE Wayland, niri/wlroots and X11.
Prioritize the Linux port and UI. Replace `Transcripts.md` as the active history
store on Linux and macOS with searchable local history. Add the accepted hold/toggle,
word-replacement and optional music controls after the core path works. Defer
unspecified novel features until the port/UI milestone has passed.

## Architecture and boundaries

Keep native Swift on macOS and the existing single Rust application on Linux.
Preserve Windows behavior and build compatibility. Propose GTK4 for Linux's small
history/settings window; use SwiftUI/AppKit for its macOS counterpart. GTK4 stays
Linux-only. Keep recording/inference outside UI and storage threads.

History uses the same versioned SQLite schema on both platforms. One row per final
result: stable ID, timestamp, model, exact final text, optional raw text. Index
timestamp plus ID for chronological paging; use FTS5 for text search. Bound pages
to 50 snippets, then load full text for the selected entry. Parameterize SQL and
quote literal user searches for FTS rather than exposing its query syntax by accident.
Keep migration records separate from transcript rows. Preserve identical repeated
dictations; text equality must not deduplicate legitimate recordings.

Use rusqlite `=0.41.0` with verified bundled FTS5 support for Rust and system SQLite
through a small Swift system-library target on macOS. Proposed GTK4 pin: `=0.11.5`.
Before changing the lockfile, check each exact dependency's source, MSRV, native
minimum, feature flags and licenses. Retain the current engine/dependency pins.
Read the documentation links in research.md and existing local docs for the APIs
each task uses. No speculative dependency upgrade is part of this plan.

## Product behavior

- A compact native window opens from the app launcher or tray/menu. Linux must
  retain useful controls when the desktop has no tray host, including stock GNOME.
- History shows recent snippets with date/time, search, a selected transcript,
  Copy and Export. Settings contain the required recording/input controls.
- Start with a quiet system-native light/dark interface, concise labels, clear
  focus, keyboard navigation and screen-reader names. Avoid a dashboard or sidebar
  of future features. History queries/imports must not stall recording or the UI.
- Paste remains prompt. Storage follows delivery on the successful path; a paste
  failure still saves the result. Keep copyable text and durable recovery on errors.
- Keep history until the user chooses otherwise; no automatic retention/deletion
  policy was requested. Markdown and plain-text export remain explicit actions.
- Hold starts once on press and stops once on release. Toggle remains available.
  Lost release/focus/session events must not leave capture stuck. If the chosen
  desktop mechanism cannot report releases, show the limitation and keep toggle.
- Replacements use explicit deterministic precedence, phrase boundaries and a
  single pass over original ASR text. Keep raw text. Cover Unicode and overlapping
  phrases. Do not silently run an LLM or alter the selected model.
- Optional MPRIS control pauses only players Koett found playing and successfully
  paused; resume at capture stop/cancel, independently of ASR. Respect player
  disappearance and observable user changes. Document that MPRIS cannot reveal
  every user action that leaves a player in the same paused state.

## Phase 1: establish the Linux base

1. Inspect refs and Mac recovery changes before choosing a branch. Create a new
   local integration branch, preserving remote branches and existing worktrees.
   Read `AGENTS.md`, `docs/feature-contracts.md` and
   `CrossPlatform/evidence/blocked-604ed15/README.md`. Record the branch decision
   in `docs/decisions.md`. Verify with Git diff/status; do not merge the public PR.
2. Recover or reproduce KTT-002. Planned boundary:
   `CrossPlatform/Cargo.toml`, `CrossPlatform/Cargo.lock`,
   `CrossPlatform/scripts/build_asr_native.py`,
   `CrossPlatform/scripts/test_build_asr_native.py`, and its native-build manifest.
   Review the missing patch before selecting its real paths; prefer recovered work
   over duplicate tooling. Reproduce pinned ASR-only source builds, final linking
   and sample inference. Inspect the final executable for blocked components and
   record exact artifact/source digests. This is a separate work package; it does
   not prevent storage/UI development. Never label an uninspected build distributable.
3. Separate Linux desktop adapters in small steps, moving existing behavior first:
   `CrossPlatform/src/linux.rs`, `CrossPlatform/src/linux/portal.rs`,
   `CrossPlatform/src/linux/x11.rs`, `CrossPlatform/src/linux/control.rs`.
   Add capability detection, explicit start/stop/toggle commands to the running
   process, and a documented compositor shortcut fallback where portals fail.
   Validate peer/session ownership for any local control endpoint. Verify existing
   Rust tests and one controlled native niri session.
4. Add niri/wlroots output fallback with explicit capability selection:
   `CrossPlatform/src/linux/output.rs`, `CrossPlatform/src/linux/portal.rs`,
   `CrossPlatform/src/linux.rs`, `CrossPlatform/README.md`.
   Use the available virtual-keyboard path only where supported; retain portal
   and X11 adapters. Keep clipboard-only recovery usable when injection fails.
   Never add input-group access or a privileged daemon as the default. Test
   terminal versus GUI paste, held modifiers, Unicode and clipboard survival.

## Phase 2: storage and safe legacy import

1. Implement Rust storage: `CrossPlatform/Cargo.toml`, `CrossPlatform/Cargo.lock`,
   `CrossPlatform/src/history.rs`, `CrossPlatform/src/history/store.rs`,
   `CrossPlatform/src/history/tests.rs`. Use real temporary SQLite databases.
   Test paging with equal timestamps, literal search, Unicode, restart, duplicate
   retries, transaction interruption and durable fallback. New writes use one
   canonical store; remove obsolete append callers after cutover.
2. Implement importer: `CrossPlatform/src/history/import.rs`,
   `CrossPlatform/src/history/import_tests.rs`, `CrossPlatform/src/paths.rs`,
   `CrossPlatform/src/history.rs`. Read legacy input incrementally, preserve
   original bytes and identify source content/offsets for idempotent retries.
   Handle header/no-header formats and Mac raw/cleaned blocks. Preserve ambiguous
   or malformed blocks for review; never discard them or infer invented metadata.
   Test repeated import, interrupted import, repeated identical entries, embedded
   headings, large input, invalid encoding and source changes between attempts.
3. Add Swift storage using the same schema: `Package.swift`,
   `Sources/CSQLite/module.modulemap`, `Sources/CSQLite/shim.h`,
   `Sources/Koett/TranscriptStore.swift`, `Tests/KoettTests/TranscriptStoreTests.swift`.
   Split the importer/tests into a following <=5-file task. Preserve last-result
   recovery and the newer overnight branch's retry semantics. Test the same
   imported/generated archive against both implementations on macOS and Linux.

## Phase 3: native windows and integration

1. Linux window foundation: `CrossPlatform/Cargo.toml`, `CrossPlatform/Cargo.lock`,
   `CrossPlatform/src/linux.rs`, `CrossPlatform/src/linux/window.rs`,
   `CrossPlatform/src/linux/history_view.rs`. GTK owns its main thread; bridge
   runtime events explicitly. Search/paging run outside it; cancel or ignore stale
   search results. Wire launcher and tray to the same running application.
2. Linux controls: `CrossPlatform/src/linux/settings_view.rs`,
   `CrossPlatform/src/linux/window.rs`, `CrossPlatform/src/settings.rs`,
   `CrossPlatform/src/linux.rs`. Make default/selected microphone, recording mode,
   shortcut and autostart accessible without editing JSON. Preserve settings.
3. Mac window: `Sources/Koett/HistoryWindow.swift`,
   `Sources/Koett/HistoryModel.swift`, `Sources/Koett/KoettMenu.swift`,
   `Tests/KoettTests/HistoryModelTests.swift`. Replace Open Transcripts with the
   native history window and match Linux search/copy/export behavior.
4. Visually inspect both native windows using real desktop screenshots and
   accessibility automation. Verify light/dark, narrow windows, long transcripts,
   empty/error states, keyboard-only use and recording while searching.
   Benchmark a generated 100,000-entry archive; record open/search timings and
   bounded query/UI memory. Target <200 ms warm first-page and common search
   queries on this machine, documenting fixture and hardware.

## Phase 4: accepted dictation conveniences

1. Hold/toggle: `CrossPlatform/src/settings.rs`, `CrossPlatform/src/state.rs`,
   `CrossPlatform/src/runtime.rs`, `CrossPlatform/src/linux/control.rs`,
   `CrossPlatform/src/linux/portal.rs`. Use Activated/Deactivated when supported;
   test duplicate events, disconnect, release after failure, mode switching and
   desktop shortcut conflicts. Preserve the existing Mac implementation.
2. Personal replacements: `CrossPlatform/src/replacements.rs`,
   `CrossPlatform/src/settings.rs`, `CrossPlatform/src/linux/settings_view.rs`,
   `CrossPlatform/src/runtime.rs`, `CrossPlatform/src/lib.rs`. Verify boundaries,
   case, Unicode, overlap and non-cascading rules through the delivered/stored text.
3. Optional music control: `CrossPlatform/src/linux/media.rs`,
   `CrossPlatform/src/linux.rs`, `CrossPlatform/src/linux/settings_view.rs`,
   `CrossPlatform/src/settings.rs`. Use the existing D-Bus dependency surface;
   test a disposable MPRIS player and capture success, failure and cancellation.

## Phase 5: packaging and verification

Update Linux packaging/install documentation for the actual GTK/native runtime
floor and all adapters. Split package changes into <=5-file tasks. Build portable
packages against a declared supported baseline and test installed artifacts,
including desktop entry, single instance, permission prompts and restart.

Run `cargo fmt --check`, `cargo test --locked`,
`cargo clippy --all-targets -- -D warnings`, `cargo build --release --locked`,
and `cargo check --locked --target x86_64-pc-windows-msvc` from CrossPlatform.
Run required build-receipt, supply-chain, workflow and native artifact checks.
On macOS run the root AGENTS.md Swift, release, installer and plist checks.
Run `git diff --check`. No old test receipt counts as verification of new changes.

The desktop matrix includes GNOME Wayland without a tray extension, KDE Wayland,
niri plus another wlroots desktop, and an X11 session. For each verify installed
launch, hotkey recording, local inference, browser and terminal paste, Unicode,
copy fallback, search/copy/export, permission rejection and restart. Include a
long recording and at least one real microphone session. Use VMs/virtual audio
where useful; label synthetic tests and hardware checks separately.

macOS compilation/UI checks require a Mac execution environment. Discover what
is available before implementation; if unavailable, preserve completed Linux
work and report the exact outstanding Mac gates. Never claim both platforms done
based on Linux tests. Public release, production-branch pushes, PR merge and
destructive removal of original archives remain separate approvals.

## Progress

- [x] 2026-10-26: inspect repository branches, storage and local desktop capabilities.
- [x] 2026-10-26: review Voxtype and official storage/desktop documentation.
- [x] 2026-10-26: confirm broad Linux support, shared history scope and UI priority.
- [ ] Approve implementation goal; then execute phases with current verification.

# Koett decisions

## 2026-10-26: integrate the existing native port with current Mac recovery

The Linux UI/history work branches from `origin/codex/overnight-2026-10-03`
at b6e2c73, which contains the current Mac microphone/recovery fixes. Integrate
the existing Rust port from `origin/feat/windows-linux-v0.1` at e85d0d0.
Keep Mac Sources/Tests from the newer branch during conflict resolution;
optional multilingual model changes from the draft are outside this task.
Do not merge or update the public draft PR as part of this local integration.

SQLite/FTS5 replaces the active growing Markdown archive on Linux and macOS.
The user confirmed in-app recent entries, search, copy and export, preserving
original legacy files. GTK4 supplies Linux windows; SwiftUI/AppKit supplies Mac
windows. This adds no browser runtime to dictation. See plan.md for sequencing.

## 2026-10-26: verified source versions own imported search rows

Schema v2 tracks each legacy path's verified content version and its imported IDs.
Re-importing an appended archive replaces obsolete imported search rows atomically,
while native recordings, other active source references and original bytes remain
protected. Earlier development schemas lack provenance, so their existing rows
are protected during upgrade rather than guessed away. History opens and queries
never scan the legacy archive; startup imports run in the background.

Linux GUI verification uses the private `agent-lab koett` compositor, input,
audio and portals. Host desktop checks require explicit authorization. This is a
test environment choice; the product still uses native GTK/Swift windows.

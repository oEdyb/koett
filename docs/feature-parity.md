# Feature parity

Checked in code on 2026-11-03. Update a row when a feature changes.

| Feature | Mac | Linux |
|---|---|---|
| Dictionary: editor, import, export | Yes: menu → Dictionary… | Yes: Settings → Dictionary |
| Dictionary during transcription | Parakeet v2: CTC boosting, adoption, spelling. Nemotron: spelling only | v2 and v3: hotwords, adoption, spelling. 110M (default) and Nemotron: spelling only |
| Spelling and S1-mini order | Before and after cleanup | After cleanup only |
| Fix a word | Yes | Yes |
| Literal replacements | Yes: Dictionary → Replacements | Yes |
| Draw while dictating | Yes | Yes where layer-shell plus grim or portal exist (checked on KWin; not GNOME or X11) |
| Shift-click pins | Yes | Yes on niri: app and title always; Chrome page link and selection after its one-time setup; link or image under the pointer only on floating windows. Pin without context on other desktops |
| Pill: rolling strip, pause, Paste, Done | Yes; no pause on Nemotron | Yes |
| Silence: VAD trim, dots, no warnings | Yes; Nemotron skips the trim | Yes (empty-result path not traced) |
| Recovery: last transcript, failed audio, failed text | Yes | Yes |
| Permissions onboarding | Yes: first-launch window with live status | No window; portals ask on use |
| History: search, Copy, Export | Yes | Yes |
| Media transcription | Focused browser tab only | Focused document or typed URL |
| Open media transcripts | Yes | Yes |
| Ask with spoken replies | Yes | Yes |
| S1-mini formatting | Yes | Yes |
| Microphone choice, toggle or hold | Yes | Yes |
| Esc cancels recording | Yes | X11 only; Wayland uses the pill Cancel or `koett cancel` |
| Parakeet v3 in the app (Swedish) | No: v2 and Nemotron; `koett-transcribe` has v3 | Yes |
| Parakeet 110M | No: used only for boosting | Yes |
| Pause media while dictating | No: MediaRemote is entitlement-locked; a media key could start playback | Yes (MPRIS) |
| Video transcript CLI (`koett-transcribe`) | Yes | No |
| Start-at-login toggle | No: on at install | Yes |
| Microphone level preview | No | Yes |
| Control CLI | `--import-dictionary` only | Yes (`koett` commands) |
| Retry Save and recovery status | No: Copy Unsaved Transcripts only | Yes |
| Image paste chord | Per app | One global setting |

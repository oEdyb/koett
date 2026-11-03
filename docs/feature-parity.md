# Feature parity

Checked in code on 2026-11-03 against `origin/main`. Update a row when a feature changes.

| Feature | Mac | Linux |
|---|---|---|
| Dictionary: editor, import, export | Yes: menu → Dictionary… | Yes: Settings → Dictionary |
| Dictionary during transcription | Yes: CTC boosting, term adoption, spelling | Yes: hotword decoder, term adoption, spelling |
| Fix a word | Yes | Yes |
| Literal replacements | Yes: Dictionary → Replacements | Yes |
| Draw while dictating | Yes | Yes (KWin, niri; not GNOME or X11) |
| Shift-click pins | Yes | Not on `main` yet; in progress |
| Pill: rolling strip, pause, Paste, Done | Yes | Yes |
| Silence: VAD trim, dots, no warnings | Yes | Yes |
| Recovery: last transcript, failed audio, failed text | Yes | Yes |
| Permissions onboarding | Yes: first-launch window with live status | No window; portals ask on use |
| History: search, Copy, Export | Yes | Yes |
| Media transcription | Yes | Yes |
| Ask with spoken replies | Yes | Yes |
| S1-mini formatting | Yes | Yes |
| Microphone choice, toggle or hold, Esc cancel | Yes | Yes |
| Parakeet v3 in the app (Swedish) | No: v2 and Nemotron; `koett-transcribe` has v3 | Yes |
| Parakeet 110M | No: used only for boosting | Yes |
| Pause media while dictating | No: MediaRemote is entitlement-locked; a media key could start playback | Yes (MPRIS) |
| Video transcript CLI (`koett-transcribe`) | Yes | No |

# Linux parity: every currently open row

Audited 2026-11-04 against every Linux cell and gap/acceptance note in
`feature-parity.md`, on main `d899540` plus the GNOME focus-identity change.
“Yes” in that table means implemented; it does not erase a named acceptance gap.
Rows with no Linux gap identified there or in the linked receipts are omitted.
This audit records limitations, not new acceptance evidence or OS impossibility.

## Dictation and recovery

1. **Toggle / hold / configurable shortcuts** — desktop portal/compositor
   availability and physical keyboard/layout acceptance remain desktop-dependent.
2. **Pill rolling strip, timer, sketch bands** and **Pill hover Cancel, Pause,
   History** — layer-shell implementation exists; the equivalent GNOME/X11 pill
   remains missing. Normal-window feedback and explicit controls are fallbacks.
3. **Silence trimming / no silence warnings** — real 110M/synthetic silence
   checks passed; physical microphone and other-model acceptance remain open.
4. **Escape cancel** — Wayland observation requires keyboard read permission.
   Denied/revoked physical-device acceptance remains open; pill/CLI Cancel works
   without that permission. No claim of universal global Esc is made.
5. **Paste into original target** — niri/KWin/X11 guards exist; optional GNOME
   focus identity now has native evidence. Other Wayland adapters, full GNOME
   portal delivery and real target consumption remain open. No atomic receipt.
6. **Terminal versus GUI paste chord** — exact per-app rules now have optional
   GNOME identity as well as niri/KWin. Real terminal/editor image consumption,
   missing adapters and clipboard/portal backend coverage remain open.
7. **Last transcript / Copy and Paste Last** — guarded CLI/controller recovery
   exists; real niri/X11 target consumption and image recovery matrix remain open.
8. **Crash during active capture** — no durable audio journal. Shared Mac/Linux
   backlog needs an explicit bounded retention/privacy contract first.
9. **Long live dictation** — no fresh 30-minute physical microphone session;
   memory, progress, pause/cancel, dictionary and chunk boundaries need checking.
10. **Permission onboarding** — first visible launch and live status exist;
    real mic/keyboard/screenshot grant, denial and revocation remain unaccepted.
    GNOME extension setup is explicit/manual; a native setup/status action is
    still future integration work.

## Models and words

11. **CPU / GPU / ARM64** — Linux CPU is implemented. Other Linux architecture/
    accelerator claims require native builds and graph/device benchmarks; no
    accelerator or ARM64 acceptance is inferred from x64 CPU tests.
12. **Acoustic dictionary bias + adoption** — real 110M sidecar benchmark and
    bounded raw fallback shipped in #65. Hardware accuracy/long-session adoption
    remains open, and Nemotron still has spelling-only behavior on Mac/Linux.
    That is not acoustic hotwords. Long chunks may fall back to raw by design.

## Drawing, pins and clipboard

13. **Drawing, hold key, pass-through otherwise** — GNOME overlay/input/capture
    and X11 overlay backends remain missing. GNOME identity alone does not close
    drawing. Layer-shell/KWin evidence does not prove those desktops work.
14. **Bare Shift without interrupting capitals/chords** — deliberate 120 ms
    click gap removed; the smaller key-sample/surface-commit race remains.
    Physical multiple keyboards, IME/layouts, lost release and compositor input
    acceptance remain open. Briefly hold before clicking as the current fallback.
15. **Undo ink** — recording-scoped pending/captured undo exists; real capture,
    tray and ordinary-editor acceptance remains open. Global editor Undo is not
    intercepted.
16. **Multi-monitor / mixed DPI / hotplug** — live GDK refresh and private KWin
    virtual-display tests passed. Physical mixed DPI, portal crop alignment and
    hotplug during an active niri pick remain unaccepted.
17. **Word times, sketch linking, numbered contact sheet** — implemented, but
    some model times are estimated. Real long-path timing/capture alignment
    remains acceptance work; estimated times must retain that label.
18. **Shift-click pins + app/title** — niri/KWin work; GNOME pointer context and
    other compositor adapters still need implementation/provider acceptance.
19. **Browser URL / selection** — Chrome accessibility setup is explicit;
    other providers/desktops and unavailable metadata need acceptance or an
    opt-in bridge. Image-only context remains available when data is absent.
20. **Link/image under pointer, tiled windows** — optional niri adapter passed
    native tiled/scrolling tests. Stock tiled niri omits coordinates. Production
    adapter packaging/deployment, mixed DPI and complete overlay/capture/paste
    acceptance remain open, as do GNOME and other missing geometry adapters.
21. **YouTube timestamp link** — matched AT-SPI player/URL works in the verified
    fixture; other browser/provider acceptance and an explicit bridge when the
    range is absent remain open. Never invent a playback time.
22. **Password avoidance / bounded opt-in selection** — rejection and budgets
    exist; real password/selection provider matrix and stale-window checks on
    other desktops remain open. Never synthesize Copy to read selection.
23. **Private/incognito URL exclusion** — automatic private-mode detection is
    absent on Mac/Linux. Explicit image-only pins are the closest alternative;
    automatic exclusion needs a reliable private-mode signal/explicit bridge.
24. **First paste image then bullets** — staged chords exist, including optional
    GNOME identity. Native GNOME portal grants/delivery, KWin backend fallback
    limits, real consumption and per-stage race acceptance remain open.
25. **Repeat paste PNG + HTML + text bundle** — owner implemented; actual agent
    editor, native rich text, plain editor and terminal consumption matrix open.
26. **Preserve newer clipboard copy** — owner/race guards implemented; real
    reader/ownership-transfer/portal timing coverage remains open.
27. **Image privacy / lifetime** — memory-only except deleted temporary portal
    files. Real portal denial/revocation and lifetime/error-path acceptance remain
    open; this is not permission to persist images in history.
28. **Click / spoken-cue keyframes** — explicitly absent on Mac/Linux; shared
    product backlog needs an exact cue behavior/acceptance contract first.

## History, media, Ask and automation

29. **Focused browser media transcription** — implemented; fresh focused URL/
    selection/provider acceptance on other desktops remains open alongside pins.
30. **Pause media during dictation** — MPRIS implemented. Unsupported/non-MPRIS
    browser/player acceptance or explicit provider integration remains open;
    never send a toggle that can start paused media.
31. **Vimeo anonymous fetch** — Linux not freshly reverified; Mac evidence is
    extractor HTTP 401. Track/recheck upstream and retain user-supplied local
    files; never silently import browser cookies.
32. **Ask text + one screenshot, providers/models/keys** — implemented;
    screenshot grant/denial/revocation and GNOME integration acceptance remain
    open, with no claim that provider code proves desktop capture works.
33. **Ask spoken replies / interrupt** — implemented; installed-panel speech/
    interruption acceptance remains open.
34. **Ask Markdown/math/local renderer** — installed host preflight still
    reports rich runtime unavailable; plain fallback retained. Complete WebKitGTK
    helpers/resources and live installed-panel Markdown/math acceptance remain.
35. **Dark/light, high contrast, scaling, reduced motion** — native evidence
    exists; live theme changes, contrast/scaling, narrow/long text and real
    keyboard/screen-reader matrix remain open.
36. **Signed install / updates / architectures** — current-PC source install
    verified, including idle upgrade/data preservation. Clean user, offline,
    update/recovery and distro/runtime packaging acceptance remain open. This
    includes source-installed CLI setup; it is not a public multi-distro release.

## Next implementation

GNOME recording pill/drawing surfaces, pass-through and input, memory-only
capture exclusion, cancellation and pointer context are next. Verify each in
native GNOME before changing its parity cell. X11 drawing and other compositor
adapters stay explicit. No feature has been declared impossible on Linux.

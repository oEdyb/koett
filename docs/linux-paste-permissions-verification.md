# Linux image chords and permissions

2026-11-04. Independent row-4 change based on main. Not installed or merged.

## Behavior and defaults

`image_paste_shortcuts` is an optional map in settings.json. Exact niri app IDs
select Ctrl+V or Ctrl+Shift+V for contact-sheet images. Unknown apps retain Ctrl+V;
text continues to use the independent `paste_shortcut`. Both portal and wtype
image paths resolve the same rule. Existing settings load with an empty map.
The target-window guard remains; no app-name substring detection or double chord.
Other desktops without trustworthy app identity retain Ctrl+V. Their identity
adapter and X11 drawing/image path remain in the broader Linux parity backlog.

Edit rules in Settings → Dictation, one `app ID = Ctrl+Shift+V` or `app ID = Ctrl+V`
per line. Empty means defaults. Duplicate IDs/unsupported chords are rejected.
The agent equivalent is `koett menu-image-paste APP_ID ctrl_v|ctrl_shift_v|default`;
`menu-status` returns both text and image preferences. Writes retain the existing
settings lock and editor merge. No history/database schema change.

Permissions opens from the native Menu or `koett permissions`. The running app
provides `koett permissions-status` JSON. Keyboard availability and shortcut
registration refresh while the window is visible. Neither means that every
physical key or paste target has been tested. Keyboard setup is optional for
normal dictation and grants read access to the user's other processes too.

Microphone checking opens the existing selected-input preview, which yields to
recording and releases capture on close. It does not invent an OS-wide permission
flag. Screen capture runs only after Check capture, is refused while busy, and
discards the image. The [Screenshot portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
may ask for permission. No automatic screenshot or first-run blocking screen.
This is a manual status/actions window; no automatic first-run onboarding claim.

## Fresh checks

- Formatting, Rust tests, Clippy and release build passed. One full run hit an
  existing formatting timeout-fixture connection reset. Its server closed the
  tokenizer connection after one read without consuming the body. The test-only
  fixture now drains the declared request before replying; its timeout assertion
  and production formatting code are unchanged.
- Library suite: 374 passed, 14 ignored, plus native ABI test passed. New coverage
  checks exact app matching, unknown defaults, invalid/duplicate rules, legacy
  settings and preservation of independent settings changes.
- Windows cross-check attempted; native MSVC/ASR build tools are unavailable on
  this Linux host. Windows runtime behavior is not claimed.
- Private KWin lab `koett-row4-sol`, copied 110M model, isolated settings/data and
  silent microphone: Permissions window and live JSON worked; opening Audio from
  it worked; image rule save/reopen worked; the text chord stayed Ctrl+V; invalid
  image chords left the saved rule intact. CLI setting/removal was checked too.
- [Screenshots](../CrossPlatform/evidence/paste-permissions-20261104/) were
  inspected for the permissions window and image-rule editor. The final UI run
  used a private fake `grim` returning a fixture PNG to check the explicit capture
  button, asynchronous result and last_capture_check JSON. Its available label is
  a **fixture result, not proof of an OS permission grant**. A real capture attempt
  stalled the private portal; that lab was reset. Real grants/revocation remain
  unverified, as does a full image paste into a real terminal/browser.

Builds used CARGO_BUILD_JOBS=4, nice -n 10 and user scopes with MemoryMax=12G.
Output stayed in the worktree. No host input, installation, or subagents.

## Deferred order

Next are row 5 (benchmarked acoustic dictionary bias, raw deadline fallback,
spelling before/after S1-mini), then row 6 (remaining Linux audit rows).
After row 6, return to stock niri tiled geometry and GNOME drawing overlays.
Neither is closed by this PR; feature-parity.md names the required integration.

# Stop modifiers and focus recovery

The installed Linux journal contained one explicit text-delivery refusal:
`modifiers_held`, with the original target still focused and only Control down.
The next delivery attempt succeeded after release. A posted key is not proof of
application consumption; the logs do not identify which stop control was used.
Raw host receipts remain private. No History text was printed.

## Change

Increase the bounded readiness window from one to two seconds. Control, Shift,
Alt and Meta still block posting until released. The target must remain the
original window, settle, and pass the final focus/modifier check. No modifier is
bypassed or forcibly released. A key that remains down ends in the existing
Not pasted / Paste / Copy recovery UI. No blind refocus, extra pill control,
clipboard-format change, or retry of an already posted text step was added.

The regression covers Shift, Control and Alt remaining down past the old
one-second deadline, release and stable readiness before the new deadline, and
final refusal if a modifier or different target appears before posting.

## Private real-target matrix

All 40 journeys passed (20 per compositor, 10 per target). See `matrix.jsonl` for synthetic-fixture metadata, without dictation contents.
Each case runs the actual Koett app with copied models, local ASR and an isolated
microphone graph, then checks the native paste event, actual editor contents,
one saved result and unchanged existing draft content. No message was sent.

The matrix covers T3 Code's real composer and a Chrome rich editor on:

- KWin with two 1280 x 800 virtual outputs: the recording pill is dragged from
  the editor's output to the other output, without changing the focused target.
- Nested niri with one winit output: the pill is dragged within that output.
  Two-screen coverage comes from KWin, not from niri's single-output backend.

For each compositor and target, ten journeys cover:

- Tray Stop, configured Ctrl+Shift+Space shortcut, and a separate F9 stop binding.
- Main-window Stop, visible recovery, then Paste back into the original editor.
- Control, Shift and Alt released 1.2 seconds after delivery begins.
- Each of those keys held beyond the two-second deadline, then recovery Paste.

There is no Stop button on the recording pill. Its drag grip is tested directly.
KWin main-window Stop is a pointer click; niri focuses the main window and
activates its real accessible Stop button. The private headless desktop has no
tray panel, so tray Stop invokes the actual exported DBusMenu click action.
KWin uses its portal shortcut; niri uses compositor bindings and native fallback
paste, matching the installed host's delivery route.

The held-key fixture intercepts EVIOCGKEY on an isolated fake keyboard device.
It exercises Koett's production physical-key sampler and full delivery path;
it does not inject a keyboard device into the shared host kernel. Pointer and
shortcut events use the private compositor. The same patched GTK runtime as the
installed app is used. All processes, model writes and browser profiles are
private; no host typing, focus or clipboard changes.

Main-window Stop takes focus, so it must not type into Koett or guess where to
refocus: it shows Not pasted with Paste and Copy. After selecting the editor,
Paste delivers once. Tray/shortcut/keybind and nonactivating pill drag preserve
the editor and paste automatically. Held keys never cause typing while down.

Local formatting, 426 Rust tests (19 ignored), Clippy and release build passed.
These checks do not prove physical multi-monitor hardware or microphone behavior.
Windows/MSVC checking is left to CI; no Swift changes or Mac runtime tests.
Mac source has a blanket physical-modifier guard with a 1.5-second wait, so held
Command can cause an analogous refusal. Control is not Command on Mac.

Fixture versions: KWin 6.8.2, niri 26.04 (8ed0da4), Chrome 156.0.8078.47.
Audio sample SHA-256: `9aa02898d5d7df779206009689369b314ff835149ed488da79e60b9d8a49310a`.

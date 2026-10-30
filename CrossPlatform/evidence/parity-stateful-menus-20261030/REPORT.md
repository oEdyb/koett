# Stateful native menus — focused implementation receipt

Scope: Linux GTK/GIO and ksni menu actions, control socket and CLI only. Existing uncommitted Last, shortcut capture generation/cancellation and runtime work preserved. No toolkit modification, host installation, publishing, cloud request, model download or schema change.

## Implemented

- GTK Preferences groups model, formatting, microphone, recording mode, login and Settings/Shortcuts. Recovery remains grouped. Tray recovery grouped while durable Copy/Paste Last remain directly accessible.
- Native GIO string/boolean state and ksni radio/checkmark items reflect saved preferences, not speculative clicks. Polling picks up external preference changes. Model title names the active engine separately from the desired model radio selection. Model choice opens the existing Models UI for explicit Use/setup; no silent download or default change.
- Real CPAL input IDs, System default and unavailable saved input appear in both menus. Enumeration is a background worker; selection validation is on the controller, outside GTK/tray callbacks. No preview stream is created by menus. Existing preview ownership implementation remains intact.
- Controller-serialized mode/microphone/login actions reload current preferences and change only their owned field. Login updates the existing autostart operation, rolls back registration on preference-save failure, and displays registration state. This is not a proof against arbitrary simultaneous writers outside the controller.
- Formatting Raw/S1-mini and style/structure/context CLI patches use the existing runtime operation and preparation path. Async acceptance explicitly says `change_requested`; checks reflect only saved state. Existing compact formatting controls remain reachable.
- Pending capture, model/media admission and formatting preparation protect preference changes before the worker's next status. Model/format operations reject a pending capture. Existing central shortcut cancellation and weak callbacks preserved.
- Ask/Cancel Ask and configuration entry, About/version, History, media, recovery and guarded Quit remain available. Ask labels and cancellation sensitivity follow published Ask phase. No paid Ask request was exercised.
- Discoverable `koett menu-help`: `menu-status`, `menu-mode toggle|hold`, `menu-microphone default|CPAL_ID`, `menu-login on|off`, `menu-format raw|s1-mini`, `menu-format style VALUE`, `menu-format structure VALUE`, `menu-format context VALUE`, `menu-open models|formatting|about|settings`. JSON errors include a fix; existing commands remain compatible. Status contains no transcript or provider configuration.

## Native evidence

Only owned `agent-lab koett-stateful-menus`, private XDG/DBus/clipboard/input/PulseAudio, cached 110M model and silent private monitor input. Fixture cleans up its app, watcher and accessibility registry. Lab stopped at handoff.

`final-light.log` and `final-dark.log` exercise persistence, unrelated-field preservation, invalid microphone rejection, failed save preserving selection, keyboard menu navigation, pointer Toggle selection, real tray D-Bus GetLayout/Event checked-state changes, selection of `alsa:lab` before the next recording, busy rejection, Raw selection, About/version and narrow capture. Earlier `verified-*.log` also contain the selected private CPAL capture checks. App log recorded `microphone=Private lab microphone`, 48 kHz, zero signal. No host microphone stream used.

Selected inspected images: `final-{light,dark}-{mode,about,narrow}.png`. Checked Hold, native About 0.1.0 and the narrow layout were inspected. Narrow captures include the native root menu. Earlier `verified-*` narrow images show the underlying compact window. These are Linux behavior evidence, not Mac visual approval or whole-port parity.

The exact tested release binary SHA256 before the final formatting patch extension was `63b21d92a2bef5ba67b31262ada83bf1e6c4327ba35404f2e5864d8ed867db53`.

## Integrated verification

Before disappearance of the native library directory: 28 focused Linux tests passed, 2 existing ignored tests; strict all-target Clippy, locked release, Rust 1.92 source check and Windows source check passed. Tests ran serially because existing process-global busy state is shared across tests; no assertion removed. Recovery dispatch assertions now traverse the intentional submenu.

The missing ASR-only ABI0 cache was recovered from verified repository/Docker
floor artifacts; its provenance and exact archive hashes are in
`../native-library-restore-20261029/receipt.json`. This is a compatible pinned
floor-cache recovery, not a claim of byte identity to a deleted cache whose old
hashes were unavailable. The final linked release now succeeds.

The final full Rust suite passes **144 tests**, with **10 existing opt-in/benchmark
ignores**. Strict all-target Clippy, locked release, Rust1.92 source checking and
Windows source checking (including test code), formatting and diff checks pass.
The selected Python unit gates pass **35 tests**. Windows runtime remains untested.

`integration/` records real native runs with the privately patched GTK runtime
from `../gtk-accessibility-fix-20261030/REPORT.md`. Dark native acceptance was
repeated after the shared Settings transaction change. The light run covers the
same menu visuals; its metadata identifies the earlier application binary. The
runtime/library and application hashes are explicit in each JSON receipt. The
selected captures were inspected. Both runs check model selection, formatting
patches, state/persistence/failure, keyboard/pointer and real tray actions, private
microphone capture, busy rejection, About and narrow layout. The microphone
assertion now requires the capture log to identify the exact private device,
rather than merely checking saved configuration. No physical microphone was used.

Review found a settings lost-update race. All in-process Settings read/modify/write
callers now share `SettingsEdit`; its guard survives commits and failures so actual
autostart rollback stays serialized. GTK stale-draft merging occurs under that
guard; slow device enumeration and model preparation stay outside it. Deterministic
concurrency/failure tests pass. Two independent follow-up reviews found no remaining
transaction defect. A Windows validation regression was also fixed: unused Ask/media
bindings no longer reserve otherwise valid Windows dictation shortcuts. A controlled
worker test verifies that a second formatting patch is rejected while the first is
pending, preventing stale nested formatting updates.

The direct `menu-model compact|parakeet_v2|nemotron` command is now discoverable.
It uses the existing runtime switch path, returns `change_requested`, and exposes
completion through menu-status. Native acceptance switches the cached Compact model;
unit coverage verifies v2 dispatch and invalid/busy rejection. No live preference
or default-model change occurred. All formatting field CLI patches were exercised
natively with formatting disabled; no unwanted S1 download was started.

## Remaining acceptance limits

- Model radio choices open existing Models UI and require its explicit Use action. Direct CLI Compact switching is verified here; v2/Nemotron runtime behavior remains covered by their earlier engine receipts, not this new command journey.
- Provider preparation/streaming Ask menu states and S1-mini preparation were not exercised in this run. No paid/cloud/model preparation requests were authorized by the private fixture.
- No stress proof for arbitrary simultaneous Settings/external-process saves; controller patches preserve other fields loaded at admission, and native fixture verifies an unrelated preference survives.
- Full native pointer save-failure and external Settings-editor roundtrip remain unverified; save failure was injected through the same controller CLI operation, and external CLI changes were checked through real tray D-Bus state.
- The private patched GTK handles the demonstrated #8444 failure and is identified in the new runtime receipts. System GTK and the installed app remain unchanged; runtime packaging and broad platform acceptance remain open.

API references: pinned gtk4 0.11.5/GIO 0.22 source `SimpleAction::new_stateful`, `set_state`, `connect_activate`; pinned ksni 0.3.6 `RadioGroup`, `CheckmarkItem`, `SubMenu` source documentation. Official GIO MenuItem action/target documentation: https://docs.gtk.org/gio/class.MenuItem.html . No dependency upgrade.

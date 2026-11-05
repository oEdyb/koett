# Held Control paste refusal

Installed Linux source: c0343ae, version 5285c248118055abdf994d7f,
PID 3002123. Journal reviewed from unit koett-host-c0343ae-active.service
since that installation, using delivery_step metadata only. 55 traced steps
include 27 posted image/text steps and one refused text step. A key post does
not prove the target consumed it; these logs cannot enumerate silent target-side
rejections. No History text was printed.

The one explicit delivery refusal:

- Step 44, text, 168 bytes, begin timestamp 1793841943231 ms.
- guard_timeout/end at 1793841944241 ms: outcome refused,
  reason modifiers_held, expected Some(74), observed Some(74), modifiers Some(2).
- No paste_post or native key was sent. Mask 2 means physical Control.
- Step 45, same 168-byte result, subsequently posted at 1793841950537 ms:
  outcome ok, reason key_posted_native, expected/observed 74/74, modifiers 0.

Both supported Linux paste chords already require Control. The guard treated
that compatible modifier as an incompatible extra key. The change permits
Control alone in readiness and final-post checks; Shift/Alt/Meta still block,
and another focused window is still refused. The regression uses exactly the
recorded window/mask, then checks the final post and extra-modifier/window
refusals. It does not synthesize physical keyboard events on the host.

The installed recording pill is nonfocusable and uses keyboard-none layer mode;
it has no Stop control. The main Koett window's recording button sends Toggle,
and that window can take focus. The log does not identify the stop input source,
so this refusal cannot honestly be attributed to clicking Stop. The requested
real T3/Chrome stop-button, shortcut/keybind and dragged multi-monitor matrix is
pending; this PR stays draft. No host install or typing.

Mac source has a blanket physical modifier check too, with a 1.5 s wait.
Holding Command (part of Command+V) may have an analogous refusal; Control is
not equivalent to Command on Mac. This is source analysis, not a Mac reproduction.
Swift is unchanged.

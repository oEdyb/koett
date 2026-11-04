# Request-triggered replay: rejected

This is a private-lab investigation, not a working replay feature. PR #48 remains
draft. All earlier application changes are withdrawn, including the extra Done
control and text-only default. Application source matches the base branch.

The passive native example clipboard_replay_probe owns an offer and logs MIME,
byte length and read time. It has no production hook and requires a matching
agent-lab environment before it can open a window. probe.py applies candidate
replay decisions with compositor-delivered keys only inside private Chrome.
targets.py tests actual T3 Code 0.1.2, Chrome and kitty. No message was sent.

## Findings

- A successful paste and a canceled trusted Ctrl+V plus a background wl-paste
  reader produced the same text/image/text request sequence. The target stayed
  focused and the clipboard remained owned. Chrome prefetched text even when
  its key handler canceled paste. The background reader fetched the remaining
  formats, indistinguishable at the owner callback.
- A one-shot prototype follow-up after that canceled gesture inserted an
  unexpected image. Deduplicating the replay does not fix this false positive.
- A text read was not the format actually inserted: Chrome read text first but
  inserted an image. Trying the replay's image leg from that inference produced
  a second image. The text leg was stopped because duplication was established.
- Moving to another field in the same browser window preserved the window ID.
  A window-only replay gate then pasted unwanted text into that new field.
  Production focus checking was never bypassed or altered; the experiment
  demonstrates the limits of the proposed observable window match.
- External clipboard replacement detached the provider; exact newer-copy bytes
  remained selected. The passive provider never reclaimed them. This checks
  ownership observation, not a race-free replay writer.

## Static offers

| Offer | T3 composer | Chrome native editor | kitty paste |
| --- | --- | --- | --- |
| PNG + plain text | image, no text | image, no text | exact text |
| HTML with inline PNG + plain text | text, no image | image and text | exact text |

Recipient-side support is needed for a reliable paired single paste in T3.
A passive bundle lets the receiver handle one real paste instead of the sender
guessing intent and injecting another key. The wl-copy fallback would also need
a multi-format owner. No such implementation is enabled by this investigation.

## Reproduce and scope

The two Python files are snapshots of the actual PC fixture and contain its
explicit cache paths. Run the example with PNG and log paths, then optional
html to select the rich offer. The example requires the owned KWin lab
koett-repeat-replay; the scripts invoke its private compositor input helper.

All output lived in ~/.cache/koett-repeat-replay. The PNG came from a private
Koett dictation/drawing with a copied Parakeet model, private audio and /dev
isolation for the existing focused-drawing fallback. No physical keyboard,
host clipboard, host focus or host pointer was used. The scripts start throwaway
browser/server profiles and do not send a T3 message. Ownership and focus
experiments ran on the clipboard probe, not the installed application.

Builds used CARGO_BUILD_JOBS=4, nice 10 and a 12 GiB systemd scope.
receipt.json contains the normalized results; no real History text was read.
The Python evidence checks guard the counterexamples, not hypothetical feature
correctness. The prior baseline format-choice receipts are in
docs/evidence/linux-sketch-clipboard/targets.json (repository root).

The portal specification describes SelectionTransfer with a session handle,
MIME type and serial. None identifies an accepted paste or focused input:
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html
Pinned source: ashpd 0.13.13, desktop/clipboard.rs.

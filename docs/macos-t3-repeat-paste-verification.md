# Mac repeat drawing paste in T3

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

The native T3 Router 0.1.25 composer passed three real Command-V pastes in a
private macOS 15.7.7 guest. The copied app bundle passed strict signature
verification. Its app.asar SHA256 matched the installed Mac bundle:
`48555eeb32fe16d24155c7be3f7e5ae259bbc86868ce75c2fbb7edc836443877`.
No account/provider was connected and no message was sent.

## Clipboard and receiver evidence

One NSPasteboard item carried 34 bytes plain text, 426,561 bytes PNG,
568,853 bytes HTML and 6,243,270 bytes TIFF. Before/after changeCount was 33;
all item/type byte counts and SHA256 values were unchanged. The item came from
a prior real Koett pin/capture/Stop/paste receipt at
`06302a2bf2dd57d23313d67994690e1fb69eaefc`. Its clipboard implementation matched
frozen v0.3.0 source `115d6907a76c3c0b3f2696797a1b5bd0ddce67c9`.
This was receiver/repeat acceptance, not a new automatic-delivery run into T3
or a claim that this guest binary was the frozen ZIP.

A passive capturing listener observed each native paste without changing it.
Every event offered text/plain, text/html and Files; HTML text and plain text
matched exactly, with one image/png file of 426,561 bytes. The real composer
retained one, two, then three copies of the text. T3 represented each initial
hyphen as a proper list marker; the text was not lost. It kept one identical
image attachment on repeat; after removal, the third paste attached it again.
The actual composer screenshot was inspected.

The earlier image-only report is not reproduced by this combination.
`a21112c` already included text in its generated HTML, so missing HTML text
cannot be inferred as that historical incident's cause. No contemporaneous
receiver event/version receipt was retained for that incident.

[Raw receipt](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/acceptance.json),
[clipboard metadata](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/clipboard-before.json),
[screenshot](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/composer.png), and
[SHA256 manifest](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/sha256.json).

## Reproduce in a disposable guest

Use the included [guest driver](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/guest-repeat-test.py),
CDP collectors, clipboard snapshot source and CGEvent helper source. It refuses
to run outside the admin test account. Compile/authorize the event helper only
in that guest; its expected path is /Users/admin/guest-gesture-v5. Place the
Python/Swift collectors under /Users/admin/pin-privacy-fixture and mount an
output share named receipts. Open a fresh guest T3 Router 0.1.25 with
--remote-debugging-port=9229 --remote-debugging-address=127.0.0.1, dismiss its
setup, and start without a project. The paired [PNG](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/fixture.png) and
[text](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-t3-repeat-20261106/fixture.txt) preserve the exact public
fixture. For a receiver-only replay, publish them with production
SketchClipboard.write before running the driver; its size/text assertions
deliberately bind this receipt, not arbitrary user clipboard data.
No target component, paste handler or application pipeline is replaced.

Physical keyboard/Option/Shift behavior, the real user's active conversation,
connected-provider/send serialization and other T3 versions remain untested.
The host's screen, clipboard, input, installed Koett and frozen ZIP were untouched.

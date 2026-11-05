# Release privacy audit

Current-tree cleanup removes build-local home prefixes from 126 text files
(the initial audit counted 125; a later source document added another). Historic
log and receipt paths use `<build-home>`; their original binary/source digests
are retained. Runnable Python fixtures resolve the current user's home instead
of hard-coding a developer's account. The file-URL rejection test uses a generic
example path. No public Git history is rewritten.

## Removed screenshots

These three images were opened and visually inspected before removal:

- `CrossPlatform/evidence/niri-geometry-20261104/native-browser.png`: a nested
  niri window with two synthetic test panes. The left pane has two example
  links and a color swatch; the right is a Chrome geometry fixture with orange
  and blue regions and an example hover URL. Chrome's address bar exposes a
  developer's local worktree file URL. No private conversation, transcript,
  account page, key or other personal document is visible.
- `CrossPlatform/evidence/parity-stateful-menus-20261030/integration/menus-native-light-final/current-light-mode.png`:
  Koett's empty History window, “No transcripts yet”, with Recording mode open
  and Hold selected. The bottom status message exposes a local settings-file
  path. No transcript, credential or personal document is visible.
- `CrossPlatform/evidence/sketch-20261102/17-old-build-virtual-shift-draws.png`:
  Chrome's synthetic Paste target with a red roughly circular stroke and Koett's
  recording pill at 0:02 with drawing marker 1. The address bar exposes the
  local fixture file URL. The target is otherwise empty; no transcript,
  credential or personal document is visible.

Their exposed information is a local account name, home directory and
project/test-directory names. Removing them from the current tree does not
remove their blobs from earlier commits or forks.

## History scan

A checksum-verified upstream Gitleaks 8.30.1 scanned all fetched public refs with
`git --log-opts='--all --full-history' --redact=100 --ignore-gitleaks-allow
--max-archive-depth=2`. No raw candidate values were copied into this report.

It returned five generic API-key findings across four evidence JSON files.
Each finding was checked in its original commit: all five are SHA-256 file or
static-library digests, not credential values. The three native receipts name
`libsherpa-onnx-c-api.a`; the downloader source inventory contains source-file
hashes. No credential has been confirmed by this scan. That does not prove
that every form of sensitive information is absent: secret detectors do not
establish that a transcript, image or personal prose is safe to publish.

A separate OCR pass read all 656 unique image blobs reachable through those
public refs. It flagged only the same three home-path screenshots; its email,
author-name and provider-key patterns found no additional images. OCR is not
a semantic review of every pixel or every transcript. No additional credential
or personal transcript has been confirmed by these checks.

The screenshot-history review and its remaining limits are recorded in the
release audit. Do not describe a partial content review as “nothing sensitive
in history”. No external service was contacted to validate any candidate key.

## Evidence storage recommendation

Stop committing raw evidence folders by default. Keep small, reviewed textual
receipts and purpose-built synthetic fixtures in Git. Store screenshots,
recordings, core dumps and verbose logs in private artifact storage or private CI
with an explicit retention period. Public CI artifacts need the same sanitation
as Git; publish only deliberately reviewed examples.

This reduces accidental disclosure and permanent clone growth. It does not
justify deleting existing evidence en masse or rewriting history without a
separate decision. This cleanup removes only the three specified screenshots
and redacts current text files; other evidence remains available for review.

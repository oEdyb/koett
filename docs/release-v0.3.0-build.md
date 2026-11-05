# v0.3.0 final build sequence

Status: preparation only. **No tag, draft release, upload or publication is
part of this sequence.** Existing candidate receipts do not certify final assets.

## 1. Close product changes before the final three PRs

Require merged source for:

- #108: Linux dictionary encoder sharing and its existing measured verdict.
- #109: keep the Linux model loaded across dictionary edits.
- #105: Mac acoustic dictionary default/performance change.
- #110: Mac cached login-status reads.
- #116: Mac explicit microphone, shortcut and screen capability checks.
- #112: preserve model destination errors; packaged 8 MiB ENOSPC retest passes.
- #117: GNOME 46 portal/focus compatibility and packaged extension.
- Any additional required Mac fixes identified by the Mac release owner.

Require review and green native jobs for changed code. The GNOME local-build
acceptance must be repeated with the approved packaged candidate. Keep existing
performance/soak evidence with its owner; do not launch another large benchmark.

Take current Mac readiness, signing/notarization evidence and unresolved GUI
checks directly from the Mac release owner's handoff. An earlier notarized ZIP
is a comparison input, not the final ZIP. Neither this document nor Linux CI
can change the Mac readiness decision.

Current Mac-owner handoff: fixes verified; final release ZIP pending. The
capability checks were exercised in the clean macOS 15 guest, with all three
rows showing Ready; final ZIP/source/notary identity and remaining acceptance
must still come from that owner. The previously notarized candidate is not
substituted for the final frozen-source artifact.

## 2. Merge documentation and version changes last

After product fixes settle, update each branch from current main and preserve
both platform audits. Resolve README conflicts by keeping drawing controls,
GNOME extension/portal setup and current runtime prerequisites together.

Merge in this order after checks/review:

1. #90: drawing-first README and platform setup.
2. #97: scoped audit, receipts, install/update/uninstall requirements and this plan.
3. #103: 0.3.0 native version metadata and generated inventories.

Keep #90/#97/#103 open until this point. Refresh the audit decision before its
merge; do not erase NOT TESTED limitations or turn old receipts into new results.
Ensure the README's download-status sentence stays truthful before publication
and does not imply an old binary has drawing. Use wording tied to the two old
version numbers rather than a claim that every currently published download
lacks the feature. Do not promise that a release URL already exists.

Verify source reports version 0.3.0 on both platforms, generated inventories
match source, and no dependency pin changed incidentally. Reconcile README setup
with the final package's included GNOME extension. Mac signing/build instructions
come from the Mac owner; do not run the installed-app installer as a packaging
shortcut.

## 3. Freeze one source identity

Fetch main after the last merge. Record its full 40-character SHA as `FINAL_SHA`.
Require #108/#109/#105/#110/#116/#112/#117 merge commits to be ancestors. Record the
final three merge commits as well. Check out that commit in clean, isolated
build checkouts on both machines; record clean-tree state.

Do not reuse a pre-merge PR merge ref, dirty local build, old Mac ZIP or old
Linux artifact. A product or README change after the freeze requires a new SHA
and replacement of both platform builds. Final verification results can remain
in a separate review branch/artifact so recording evidence does not silently
change the commit intended for the assets.

## 4. Build Linux from the frozen commit

The workflow is `.github/workflows/cross-platform.yml`. It supports manual
dispatch, does not run on tag creation and does not publish a GitHub release.

Dispatch against main only while main equals `FINAL_SHA`, or use an approved
branch pinned to it:

```sh
gh workflow run cross-platform.yml --repo oEdyb/koett --ref main
gh run list --repo oEdyb/koett --workflow cross-platform.yml
gh run view RUN_ID --repo oEdyb/koett --json headSha,status,conclusion,url,jobs
```

Require `headSha == FINAL_SHA` and all required jobs green, including both native
Linux architectures, MSRV, Windows and supply chain. No `--watch` or polling loop.

Download the artifact groups `Koett-Linux-x86_64-system-runtime` and
`Koett-Linux-arm64-system-runtime` into separate private cache directories:

```sh
gh run download RUN_ID --repo oEdyb/koett --name Koett-Linux-x86_64-system-runtime --dir ASSET_DIR/linux-x86_64
gh run download RUN_ID --repo oEdyb/koett --name Koett-Linux-arm64-system-runtime --dir ASSET_DIR/linux-arm64
```

Preserve each generated archive name:
`Koett-Linux-{x86_64,aarch64}-system-runtime-SHORTSHA.tar.gz`, plus its `.sha256`
and `.build-receipt.json`. In each directory, run `sha256sum -c *.sha256`.
Require every receipt's `source.commit` to equal `FINAL_SHA`. Check the extracted
candidate identity, manifests, notice hashes, 0.3.0 version and GNOME extension
payload. Keep ARM64 native CI distinct from untested ARM64 desktop use.

Any needed local build uses four jobs, nice 10 and a 12 GiB build scope, with
output under the worktree or private cache. No /tmp build output, model downloads,
installed-app replacement or physical keyboard changes.

## 5. Build Mac from the same commit

Give the Mac owner the exact `FINAL_SHA`, final source/version expectation and
Linux run URL. Require a clean isolated build from that SHA, producing:

- `Koett-0.3.0-macOS-arm64.zip` and its `.sha256`.
- Packaged source stamp matching `FINAL_SHA` and version/build metadata.
- Developer ID signature/hardened-runtime checks, notarization submission marked
  Accepted, stapled ticket, strict codesign, stapler and Gatekeeper evidence.
- Exact ZIP hash and executable UUID, with the tested extraction tied to them.
- Final packaged-app GUI results and honest remaining hardware/screen limits.

Do not replace the installed Mac app. Never call the ZIP notarized based only on
an old submission or notarization of a different hash. Do not assemble a release
without this ZIP alongside both Linux archives.

## 6. Rerun the README against final assets

Use fresh private homes/users/sessions and the final README only. Supply the
verified final asset files as the download input; record that the future public
release URL cannot be exercised before publication authorization. Use copied
models/audio where required by lab rules and explicitly distinguish that path
from a real first model download already covered by recovery evidence.

On Linux x86-64, check runtime preflight, install, 0.3.0 launch, Settings/History,
plain dictation and exact consumed text. In supported KDE/niri sessions, verify
opt-in drawing, pins, lock and actual image/text paste. In real GNOME Wayland,
install/enable the packaged extension, relogin, set a native custom shortcut if
needed, grant Remote Interaction and Clipboard Access, and confirm real GTK
paste. Confirm unsupported drawing remains off/unavailable. Repeat the small
8 MiB destination-error check against the final package. Check upgrade and
app-only uninstall preserve synthetic data; retain explicit architecture limits.

The Mac owner repeats the corresponding packaged ZIP journey on supported test
systems and reports remaining physical-input/screen requirements. Reuse existing
performance evidence unless the final code introduces a reason for a small
specific regression check.

Prepare a reviewed, sanitized `verification.json` containing the common source
SHA, CI URL, asset hashes, Mac signature/notary identity, tested environments and
limits. Keep raw screenshots, audio, home paths and History text private.

## Stop point

Report final-asset results and the GO/NO-GO decision with concrete remaining
gates. Keep assets staged privately. **Do not tag, create even a draft release,
upload release assets, mark Latest or publish without explicit authorization.**

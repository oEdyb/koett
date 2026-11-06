# Historical verification evidence

Screenshots, lab logs and receipts from `CrossPlatform/evidence/` and `docs/evidence/` remain in the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). Removing them from the current tree does not rewrite git history or change released artifacts. Historical document links point to that snapshot. Their commands refer to its source tree.

To reproduce historical checks privately:

```sh
mkdir -p "$HOME/.cache/koett-evidence-v0.3.1"
git archive v0.3.1 | tar -x -C "$HOME/.cache/koett-evidence-v0.3.1"
```

Run historical commands from that exported tree, with copied test models and an isolated desktop. They are old verification scripts, not installation instructions or current passing results. Existing history retains any historical sensitive material; this cleanup makes no privacy-erasure claim.

Keep new screenshots, logs and receipts in a private cache or CI artifacts. Do not commit the evidence directories again. Current deterministic inventories live in `CrossPlatform/supply-chain/generated/`; distributable licence notices remain under `CrossPlatform/packaging/`. A synthetic PNG in `CrossPlatform/tests/fixtures/` supplies the capture decoder test without a desktop screenshot.

This cleanup removes 2,176 historical lab files and moves three maintained inventory files. It replaces 62 relative evidence links with pinned archive links; no existing linked target was missing at the archived commit. Forty-six documents identify historical paths and commands explicitly.

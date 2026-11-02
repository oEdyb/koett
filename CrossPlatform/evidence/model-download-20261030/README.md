# Visible model downloads

GTK and tray model selections previously discarded the selected ID and only
opened another picker. Download status was also inside the scrolled model list,
below the initially visible rows. The menus now open setup and request the chosen
model; download status, percentage bar and Cancel remain outside the scroll area.
The existing worker validates IDs, rejects busy changes and preserves the prior
engine/preferences after cancellation.

Private native dark/light checks invoked the actual GTK menu action, downloaded
part of the official pinned v2 archive, verified nonzero accessible progress and
its on-screen bounds, cancelled setup and confirmed the compact engine/preference
remained active. Captures inspected after the footer fix are included. No host
model switch, physical input/audio or foreground test was performed. Private
models were copied and checked against the manifest; lab koett-model-download
was stopped. The first probe's visibility flag passed even though the bar lay
outside the scroll viewport; the final check adds screen bounds and nonzero value.

Two fresh code reviews found no actionable issues. Full Rust suite:146 passed,
10 existing opt-in/benchmark ignores. Strict Clippy, release, MSRV1.92,
Windows1.98 source, Ruff, formatting and diff checks pass. The final footer
layout was exercised natively; no new GTK4.6 GUI or physical tray click proof.

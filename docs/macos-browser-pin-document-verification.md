# Chrome document URL fallback

PR #132 reads the HTTP(S) document URL from Chrome/Chromium's direct
window-child accessibility group when the renderer web area is not available.
It reuses the existing ancestor walk, adds no wait, and never treats a nested
page group or an unrelated application as that document source. Web-area
metadata still takes priority. The Incognito filter runs afterward as before.

## Evidence

The prior private-pin guest check initially returned only a Chrome window title.
The AX ancestor receipt showed a scroll area and groups, no web area, while the
direct AXWindow child already exposed the real document AXURL. Chrome rejected
AXManualAccessibility at that time, so the existing success-only retry did not
obtain a page tree. Warming the tree worked for the separate PR #131 privacy
check; it is not required by this URL fallback.

25 impacted Swift tests passed, zero failures. Release product build passed in
52.81 seconds; diff check passed. No benchmark or full-suite repeat.
A signed app from `06302a2bf2dd57d23313d67994690e1fb69eaefc` passed first-pin
acceptance after a confirmed clean Chrome restart in a private macOS 15.7.7
guest. There was no browser AX pre-warm. The normal pin included the synthetic
document URL/title; the Incognito pin retained only Google Chrome. Real
Shift-click, screen capture, Stop, Command-V and History ran for both cases;
each receiver matched the saved text and each added exactly one History row.
English Chrome 157.0.8103.39; executable SHA256:
`d16c783f3dc346b71ad95e114a809917cf94a9b9caae2c801bd8d48b5b91ffca`.

This fallback supplies document URL/title, not missing link/image/selection or
YouTube player data. Those still depend on the provider's web tree. Other
browsers/channels/languages, physical keys/microphones and monitor hardware
are untested. The installed app and frozen v0.3.0 ZIP were not modified.

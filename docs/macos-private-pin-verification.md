# Mac private-window pin metadata

PR #131 recognizes the English Chrome/Chromium accessible-window Incognito
marker. It removes page title, URL, selection, link, image and video metadata,
retaining only the app name. A missing Chrome window identity also omits page
metadata. The deliberately captured screenshot is unchanged.

This uses Chromium's [accessible-window formatter](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/ui/views/frame/browser_view.cc):
`GetAccessibleWindowTitleForChannelAndProfile` appends the Incognito resource
when `IsIncognitoProfile()` is true. The English
[`IDS_ACCESSIBLE_INCOGNITO_WINDOW_TITLE_FORMAT`](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/app/generated_resources.grd)
resource ends in ` (Incognito)`. Koett checks that browser-owned window title,
not a page's text, and checks only the known Chrome/Chromium bundle IDs.

## Checks

29 impacted Swift tests passed, zero failures. Release product build passed in
47.46 seconds; diff check passed. Unit coverage includes all page-field removal,
ordinary page titles containing the word, missing window identity, supported
bundle IDs and unrelated applications.

A signed build from `0b17309e8714bc76c711464221e04c24d91e1333` ran in a private
macOS 15.7.7 guest with English Chrome 157.0.8103.39. Real Shift-click pins and
production capture/Stop/Command-V passed normal → Incognito → normal windows.
Normal pins retained the synthetic URL/title/selection; Incognito retained only
Google Chrome. Every result matched the receiver and added one History row.
Executable SHA256: `1e09ba334a4290f617e0236e41d79270a791386aec19a54cd3beb009869009dc`.

The guest browser accessibility tree was warmed before these privacy checks.
A stale restore-pages bubble was dismissed; an earlier cold-tree pin supplied
only a window title, so browser-tree readiness remains a separate issue.
Other browsers, translated private-mode markers, Chrome channels besides stable,
physical input and mixed-display hardware are untested. Use explicit image-only
pins for those private windows. No Automation permission, installed-app change,
full-suite repeat, benchmark or frozen v0.3.0 ZIP change was introduced.

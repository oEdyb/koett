# Pause supported media on Mac

The opt-in menu action **Pause Supported Media While Recording** defaults off,
matching Linux. It uses the Accessibility permission Koett already needs and
never requests Automation access. A bounded worker searches visible accessible
controls in Music, Spotify, VLC, QuickTime and YouTube in supported browsers.
It presses only an enabled button explicitly labeled Pause (English or Swedish).
YouTube additionally requires the `ytp-play-button` class. Play and ambiguous
Play/Pause controls are ignored. Stop/cancel cancels the scan. No keyboard media
key, player launch, automatic Play, or automatic resume is sent.

Resume manually. Unsupported players, hidden/inaccessible controls, other UI
languages and unavailable Accessibility access are safe no-ops. These are
implementation limits, not proof that media pause is impossible on macOS.
Real player acceptance remains open; policy tests alone do not prove AX trees.
The scan is limited to 800 ms, 1,000 nodes per player and 50 ms per AX message.

Apple documents [AXUIElementPerformAction](https://developer.apple.com/documentation/applicationservices/axuielementperformaction(_:_:))
and the [AXPress action](https://developer.apple.com/documentation/applicationservices/kaxpressaction).
The public MediaPlayer playback interface controls a supplied player; it does
not establish a general remote-control interface for all third-party players.

The earlier blanket “MediaRemote is locked” note was too broad: the restriction
concerns now-playing reads. [Epicenter's dated source decision](https://github.com/EpicenterHQ/epicenter/blob/main/docs/adr/0018-macos-resume-is-gated-on-a-coreaudio-output-read.md)
reports that explicit commands still work, but MediaRemote remains private.
Koett does not add that unsupported dependency or its Perl entitlement workaround.
Without a reliable same-player identity/state receipt, automatic resume could
start media the user wanted paused. Manual resume is the deliberate alternative.

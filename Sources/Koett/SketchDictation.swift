import AppKit
import CoreGraphics

/// Text and optional contact sheet for one dictation.
struct SketchedDictation {
    let text: String
    let rawText: String?
    let sheet: Data?
    let frameCount: Int
}

extension KoettController {
    static let sketchCaptureWait: Duration = .seconds(8)

    var effectiveDrawKey: DrawKey { DrawKey.effective(drawKey, dictation: dictationShortcut) }

    func drawMenuItem() -> NSMenuItem {
        let menu = NSMenu(title: "Draw on Screen")
        let toggle = menuItem(
            title: "Draw While Dictating",
            action: #selector(toggleDrawWhileDictating),
            selected: drawWhileDictating
        )
        toggle.isEnabled = state == .ready
        menu.addItem(toggle)
        menu.addItem(.separator())
        for key in DrawKey.allCases {
            let item = menuItem(
                title: "Draw Key: \(key.displayName)",
                action: #selector(selectDrawKey(_:)),
                selected: effectiveDrawKey == key
            )
            item.representedObject = key.rawValue
            item.isEnabled = state == .ready && !key.conflicts(with: dictationShortcut)
            menu.addItem(item)
        }
        menu.addItem(.separator())
        let chordItem = menuItem(title: "", action: #selector(toggleSketchPasteChord(_:)), selected: false)
        chordItem.tag = Self.sketchPasteChordTag
        menu.addItem(chordItem)
        refreshSketchPasteChordItem(chordItem)
        let root = NSMenuItem(title: "Draw on Screen", action: nil, keyEquivalent: "")
        root.submenu = menu
        root.state = drawWhileDictating ? .on : .off
        return root
    }

    static let sketchPasteChordTag = 0x5343

    /// Names the app the sheet would paste into now and its chord. The status
    /// menu does not activate Koett, so the frontmost app is the paste target.
    func refreshSketchPasteChordItem(_ item: NSMenuItem) {
        let app = NSWorkspace.shared.frontmostApplication
        guard let bundleID = app?.bundleIdentifier, bundleID != Bundle.main.bundleIdentifier else {
            item.title = "Sketch Image Paste: Control-V in Terminals"
            item.representedObject = nil
            item.isEnabled = false
            item.state = .off
            return
        }
        let chord = SketchPasteChord.forApp(bundleID, overrides: SketchPasteChord.overrides(in: .standard))
        item.title = "Paste Sketch Image with Control-V in \(app?.localizedName ?? bundleID)"
        item.representedObject = bundleID
        item.isEnabled = true
        item.state = chord == .control ? .on : .off
    }

    func refreshSketchPasteChordItem(in menu: NSMenu) {
        let draw = menu.items.first { $0.submenu?.title == "Draw on Screen" }?.submenu
        if let item = draw?.item(withTag: Self.sketchPasteChordTag) {
            refreshSketchPasteChordItem(item)
        }
    }

    @objc func toggleSketchPasteChord(_ sender: NSMenuItem) {
        guard let bundleID = sender.representedObject as? String else { return }
        SketchPasteChord.toggle(bundleID, in: .standard)
        refreshSketchPasteChordItem(sender)
    }

    @objc func toggleDrawWhileDictating() {
        guard state == .ready else { return }
        drawWhileDictating.toggle()
        UserDefaults.standard.set(drawWhileDictating, forKey: DrawKey.enabledDefaultsKey)
        if drawWhileDictating && !CGPreflightScreenCaptureAccess() {
            _ = CGRequestScreenCaptureAccess()
        }
        rebuildMenu()
    }

    @objc func selectDrawKey(_ sender: NSMenuItem) {
        guard state == .ready,
              let raw = sender.representedObject as? String,
              let key = DrawKey(rawValue: raw) else { return }
        drawKey = key
        UserDefaults.standard.set(key.rawValue, forKey: DrawKey.defaultsKey)
        rebuildMenu()
    }

    /// Every dictation opens a new sketch scope, with drawing on or off.
    func startSketching(session: UInt64, recorder: MicrophoneRecorder) {
        sketchFrames.begin(session)
        guard drawWhileDictating, let startedAt = recordingOverlay.recordingStartedAt else {
            sketchOverlay.discard()
            return
        }
        let overlay = recordingOverlay
        sketchOverlay.start(SketchOverlay.Session(
            id: session,
            key: effectiveDrawKey,
            frames: sketchFrames,
            audioClock: { [weak recorder] in recorder?.currentTime ?? 0 },
            onUpdate: { update in overlay.applySketch(update, session: session) }
        ), startedAt: startedAt)
    }

    func discardSketches(_ session: UInt64) {
        sketchOverlay.discard()
        sketchFrames.discard(session)
    }

    /// Waits for the dictation's captures and builds the bullets and sheet.
    /// Without sketches the text is unchanged.
    func sketchedDictation(
        text: String,
        rawText: String?,
        words: [TimedWord],
        session: UInt64
    ) async -> SketchedDictation {
        if await !sketchFrames.wait(session, timeout: Self.sketchCaptureWait) {
            fputs("Warning: sketch captures timed out; pasting the finished ones\n", stderr)
            sketchOverlay.abandonPendingCaptures(session)
        }
        let frames = sketchFrames.take(session)
        guard !frames.isEmpty else {
            return SketchedDictation(text: text, rawText: rawText, sheet: nil, frameCount: 0)
        }
        let summary = SketchLinking.keyframeSummary(
            finalText: text,
            words: words,
            anchors: frames.map(\.start)
        )
        let sheet = SketchSheet.contactSheet(frames.map(\.image)).flatMap(SketchSheet.png)
        if sheet == nil { fputs("Warning: contact sheet could not be drawn\n", stderr) }
        return SketchedDictation(text: summary, rawText: text, sheet: sheet, frameCount: frames.count)
    }

    /// Scrawl order: the numbered sheet first, then the matching text.
    func pasteSketchSheet(_ sketched: SketchedDictation) async {
        guard let sheet = sketched.sheet else { return }
        let target = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        let chord = SketchPasteChord.forApp(target, overrides: SketchPasteChord.overrides(in: .standard))
        let pasted = await SketchPaste.paste(png: sheet) { postPasteKey(flags: chord.flags) }
        if !pasted { fputs("Warning: contact sheet not pasted\n", stderr) }
        print("SKETCHES PASTED (\(sketched.frameCount)) with \(chord.displayName) into \(target ?? "unknown app")")
    }
}

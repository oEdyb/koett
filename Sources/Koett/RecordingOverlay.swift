import AppKit
import AVFoundation
import Foundation

enum RecordingOverlayFormat {
    static func elapsedTime(_ duration: TimeInterval) -> String {
        let totalSeconds = max(0, Int(duration))
        return String(format: "%d:%02d", totalSeconds / 60, totalSeconds % 60)
    }

    static func level(forDecibels decibels: Float) -> CGFloat {
        guard decibels.isFinite else { return 0 }
        let clamped = min(0, max(-50, decibels))
        let linear = CGFloat((clamped + 50) / 50)
        return pow(linear, 0.75)
    }

    static func transcriptPreview(_ text: String) -> String {
        let normalized = text
            .split(whereSeparator: \Character.isWhitespace)
            .joined(separator: " ")
        let limit = 96
        guard normalized.count > limit else { return normalized }
        return String(normalized.prefix(limit))
    }

    /// The meter is sampled at this rate so the timeline has no gaps.
    static let sampleInterval: TimeInterval = 1.0 / 30.0

    /// Reduced motion keeps a static strip that is redrawn once per second.
    static func redrawInterval(reduceMotion: Bool) -> TimeInterval {
        reduceMotion ? 1 : 0
    }
}

enum RecordingOverlayOutcome: Equatable {
    case result(String)
    case failure(String)

    var message: String {
        switch self {
        case .result(let message), .failure(let message):
            message
        }
    }

    var menuTitle: String {
        let prefix = switch self {
        case .result: "Last result: "
        case .failure: "Last error: "
        }
        let normalized = message
            .split(whereSeparator: \Character.isWhitespace)
            .joined(separator: " ")
        let limit = 72
        guard normalized.count > limit else { return prefix + normalized }
        return prefix + normalized.prefix(limit) + "…"
    }
}

/// Actions the hover row offers while a dictation records.
struct RecordingControls {
    let modelName: String
    let canPause: Bool
    let cancel: () -> Void
    var pauseChanged: (Bool) -> Void = { _ in }
}

/// A panel that shows above every app but never becomes key or main.
final class PillPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class RecordingOverlayController: NSObject {
    /// A pasted result stays this long unless hovered, then fades.
    static let doneDisplayDuration: TimeInterval = 1.75
    /// Media results take longer to arrive, so they stay longer.
    static let mediaDisplayDuration: TimeInterval = 10
    static let morphDuration: TimeInterval = 0.24
    static let fadeDuration: TimeInterval = 0.25

    let pillView = PillView(
        frame: NSRect(x: 0, y: 0, width: PillView.width, height: PillView.rowHeight)
    )
    private lazy var panel: NSPanel = makePanel()
    private weak var recorder: MicrophoneRecorder?
    private var startedAt: TimeInterval?
    /// The dictation whose sketch bands the pill shows. Kept after Stop so late
    /// captures can still renumber; replaced by the next dictation.
    private(set) var sketchSession: UInt64?
    private var timer: Timer?
    private var lastRedraw: TimeInterval = 0
    private var dismissWorkItem: DispatchWorkItem?
    /// Set when a dismiss came due while hovered; the pill leaves once the pointer does.
    private var dismissOnExit = false
    private var morphTimer: Timer?
    private var morphStartedAt: TimeInterval = 0
    private var visibleGeneration = 0
    /// True from `show` until the pill is hidden or starts fading out.
    private(set) var isPresented = false
    private var pauseChanged: ((Bool) -> Void)?
    private var resultText: String?
    private var resultPNG: Data?
    private(set) var latestOutcome: RecordingOverlayOutcome?
    var isTrackingRecording: Bool { recorder?.isRecording == true }
    private(set) var isShowingTranscribing = false
    var onLatestOutcomeChange: (() -> Void)?
    var onOpenHistory: (() -> Void)?
    var onPasteAgain: (() -> Void)?

    override init() {
        super.init()
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(displayOptionsDidChange(_:)),
            name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
            object: NSWorkspace.shared
        )
        pillView.onHoverChange = { [weak self] in self?.hoverChanged() }
        pillView.onDismiss = { [weak self] in self?.fadeOut() }
        pillView.pauseButton.action = { [weak self] in self?.togglePause() }
        pillView.historyButton.action = { [weak self] in
            guard let self else { return }
            onOpenHistory?()
        }
        pillView.copyButton.action = { [weak self] in self?.copyResult(to: .general) }
        pillView.pasteButton.action = { [weak self] in
            self?.hide()
            self?.onPasteAgain?()
        }
    }

    /// Milliseconds since the current dictation started, or nil when idle.
    var sketchElapsedMs: UInt64? {
        guard isTrackingRecording, let startedAt else { return nil }
        return Self.milliseconds(ProcessInfo.processInfo.systemUptime - startedAt)
    }

    /// The pill clock's origin (`systemUptime`) for the current dictation.
    var recordingStartedAt: TimeInterval? { startedAt }

    func start(recorder: MicrophoneRecorder, controls: RecordingControls? = nil, sketchSession: UInt64? = nil) {
        stop()
        self.recorder = recorder
        self.sketchSession = sketchSession
        pauseChanged = controls?.pauseChanged
        startedAt = ProcessInfo.processInfo.systemUptime
        pillView.timeline.reset()
        pillView.elapsed = 0
        pillView.hasRecordingControls = controls != nil
        pillView.canPause = controls?.canPause ?? false
        pillView.modelName = controls?.modelName ?? ""
        pillView.cancelButton.action = controls.map { controls in { controls.cancel() } }
        show(.recording(paused: false))
        startTimer()
    }

    /// Hides the pill. The finished timeline stays for the transcribing and done states.
    func stop() {
        if isTrackingRecording, let startedAt {
            pillView.elapsed = ProcessInfo.processInfo.systemUptime - startedAt
        }
        hide()
    }

    func showStatus(_ text: String) {
        sketchSession = nil
        pillView.timeline.reset()
        show(.status(text))
    }

    func showProgress(_ text: String, accessibilityLabel: String, fraction: Double) {
        sketchSession = nil
        pillView.timeline.reset()
        show(.progress(text: text, fraction: fraction))
        pillView.statusElement.setAccessibilityLabel(accessibilityLabel)
    }

    func showTransientStatus(_ text: String, duration: TimeInterval = 2) {
        showStatus(text)
        dismiss(after: duration)
    }

    /// Keeps the pill visible between the stop sound and the pasted text.
    func showTranscribing() {
        show(.transcribing(fraction: nil))
        isShowingTranscribing = true
        startTimer()
    }

    /// Moves the decode playhead. Fractions only come from the ASR engine.
    func setDecodeProgress(_ fraction: Double) {
        guard isShowingTranscribing, fraction.isFinite,
              case .transcribing(let current) = pillView.content else { return }
        let next = min(1, max(current ?? 0, fraction))
        pillView.content = .transcribing(fraction: next)
    }

    /// Hides the transcribing pill unless a result or error replaced it.
    func endTranscribing() {
        guard isShowingTranscribing else { return }
        stop()
    }

    /// Morphs the transcribing pill, in place and at the same size, into the
    /// pasted result with Copy, then fades it out shortly after.
    func showDone(text: String, png: Data? = nil) {
        guard isShowingTranscribing else { return }
        let from = pillView.content
        resultText = text
        resultPNG = png
        show(.done(preview: RecordingOverlayFormat.transcriptPreview(text)))
        startMorph(from: from)
        dismiss(after: Self.doneDisplayDuration)
    }

    /// Replaces the transcribing pill when the text was saved but not pasted.
    /// Returns false when another message already replaced it.
    @discardableResult
    func showNotPasted(text: String, png: Data? = nil, reason: String? = nil) -> Bool {
        guard isShowingTranscribing else { return false }
        resultText = text
        resultPNG = png
        let from = pillView.content
        show(.notPasted)
        startMorph(from: from)
        recordFailure(reason.map { "Not pasted · \($0) · Text saved" } ?? "Not pasted · text saved")
        return true
    }

    func showResult(_ text: String, message: String) {
        sketchSession = nil
        pillView.timeline.reset()
        resultText = text
        resultPNG = nil
        show(.media(preview: message))
        latestOutcome = .result(message)
        onLatestOutcomeChange?()
        dismiss(after: Self.mediaDisplayDuration)
    }

    /// Shows an error in the same 48 px row. One that holds text or audio to
    /// rescue stays until clicked or replaced; others leave after five seconds.
    func showError(_ text: String, keepUntilClicked: Bool = false) {
        sketchSession = nil
        pillView.timeline.reset()
        show(.error(text))
        recordFailure(text)
        if !keepUntilClicked { dismiss(after: 5, holdWhileHovered: false) }
    }

    func recordFailure(_ text: String) {
        latestOutcome = .failure(text)
        onLatestOutcomeChange?()
    }

    /// Adds or replaces a sketch band. Returns false when no dictation is active
    /// or the event is malformed.
    @discardableResult
    func publishSketch(_ event: SketchEvent) -> Bool {
        guard event.isValid, isTrackingRecording, pillView.timeline.apply(event) else { return false }
        pillView.needsDisplay = true
        return true
    }

    /// Applies a sketch change for `session`, also after Stop (captures can
    /// finish or fail while transcribing) until another dictation starts.
    @discardableResult
    func applySketch(_ update: SketchUpdate, session: UInt64) -> Bool {
        guard session == sketchSession else { return false }
        let applied: Bool
        switch update {
        case .upsert(let event):
            applied = event.isValid && pillView.timeline.apply(event)
        case .removed(let index):
            applied = pillView.timeline.remove(index: index)
        }
        if applied { pillView.needsDisplay = true }
        return applied
    }

    // MARK: Private

    private func show(_ content: PillContent) {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        dismissOnExit = false
        stopMorph()
        visibleGeneration += 1
        isPresented = true
        panel.alphaValue = 1
        isShowingTranscribing = false
        if case .recording = content {} else {
            timer?.invalidate()
            timer = nil
            recorder = nil
        }
        pillView.copyTitle = "Copy"
        pillView.content = content
        layoutPanel()
        panel.orderFrontRegardless()
    }

    private func hide() {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        dismissOnExit = false
        stopMorph()
        visibleGeneration += 1
        isPresented = false
        timer?.invalidate()
        timer = nil
        recorder = nil
        isShowingTranscribing = false
        resultText = nil
        resultPNG = nil
        pillView.setHovered(false)
        panel.orderOut(nil)
        panel.alphaValue = 1
    }

    /// Hides like `hide()`, but the panel fades out instead of vanishing.
    private func fadeOut() {
        guard !reduceMotion, panel.isVisible else {
            stop()
            return
        }
        let generation = visibleGeneration + 1
        let panel = panel
        hide()
        panel.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = Self.fadeDuration
            panel.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.visibleGeneration == generation else { return }
                panel.orderOut(nil)
                panel.alphaValue = 1
            }
        }
    }

    private func hoverChanged() {
        layoutPanel()
        if !pillView.hovered && dismissOnExit {
            dismissOnExit = false
            dismiss(after: 0.8)
        }
    }

    private func startMorph(from: PillContent) {
        guard case .transcribing = from, !reduceMotion else { return }
        pillView.morphFrom = from
        pillView.morph = 0
        morphStartedAt = ProcessInfo.processInfo.systemUptime
        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.advanceMorph() }
        }
        RunLoop.main.add(timer, forMode: .common)
        morphTimer = timer
    }

    private func advanceMorph() {
        let t = min(1, (ProcessInfo.processInfo.systemUptime - morphStartedAt) / Self.morphDuration)
        pillView.morph = CGFloat(Self.easeOut(t))
        if t >= 1 { stopMorph() }
    }

    private func stopMorph() {
        morphTimer?.invalidate()
        morphTimer = nil
        pillView.morphFrom = nil
        pillView.morph = 1
    }

    static func easeOut(_ t: Double) -> Double {
        let t = min(1, max(0, t))
        return 1 - pow(1 - t, 3)
    }

    private func togglePause() {
        guard let recorder, recorder.isRecording, pillView.canPause else { return }
        setPaused(!recorder.isPaused)
    }

    func setPaused(_ paused: Bool) {
        guard let recorder, recorder.isRecording, pillView.canPause else { return }
        recorder.isPaused = paused
        pauseChanged?(paused)
        pillView.content = .recording(paused: paused)
    }

    func copyResult(to pasteboard: NSPasteboard = .general) {
        guard let resultText else { return }
        if SketchClipboard(text: resultText, png: resultPNG).write(to: pasteboard) {
            pillView.copyTitle = "Copied"
            dismissOnExit = false
            dismiss(after: 1, holdWhileHovered: false)
        } else {
            pillView.copyTitle = "Copy failed"
        }
    }

    private func dismiss(after delay: TimeInterval, holdWhileHovered: Bool = true) {
        dismissWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            guard let self else { return }
            if holdWhileHovered && pillView.hovered && pillView.acceptsMouse {
                dismissOnExit = true
            } else {
                fadeOut()
            }
        }
        dismissWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: workItem)
    }

    @objc private func tick(_ timer: Timer) {
        let now = ProcessInfo.processInfo.systemUptime
        switch pillView.content {
        case .recording:
            guard let recorder, recorder.isRecording, let startedAt else {
                stop()
                return
            }
            let level = recorder.isPaused ? 0 : RecordingOverlayFormat.level(forDecibels: recorder.averagePower)
            pillView.timeline.push(elapsedMs: Self.milliseconds(now - startedAt), level: Double(level))
            pillView.elapsed = now - startedAt
        case .transcribing:
            guard !reduceMotion else { return }
            pillView.spin = CGFloat(now * 5).truncatingRemainder(dividingBy: 2 * .pi)
        default:
            timer.invalidate()
            return
        }
        if now - lastRedraw >= RecordingOverlayFormat.redrawInterval(reduceMotion: reduceMotion) {
            lastRedraw = now
            pillView.needsDisplay = true
        }
    }

    @objc private func displayOptionsDidChange(_ notification: Notification) {
        lastRedraw = 0
        pillView.needsDisplay = true
    }

    private var reduceMotion: Bool {
        NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
    }

    private func startTimer() {
        timer?.invalidate()
        lastRedraw = 0
        let interval = RecordingOverlayFormat.sampleInterval
        let timer = Timer(
            timeInterval: interval,
            target: self,
            selector: #selector(tick(_:)),
            userInfo: nil,
            repeats: true
        )
        timer.tolerance = interval / 10
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
        timer.fire()
    }

    private static func milliseconds(_ seconds: TimeInterval) -> UInt64 {
        UInt64(max(0, seconds * 1_000))
    }

    private func makePanel() -> NSPanel {
        let panel = PillPanel(
            contentRect: NSRect(origin: .zero, size: pillView.frame.size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.isFloatingPanel = true
        panel.becomesKeyOnlyIfNeeded = true
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.acceptsMouseMovedEvents = true
        panel.level = .screenSaver
        panel.collectionBehavior = [
            .canJoinAllSpaces,
            .canJoinAllApplications,
            .fullScreenAuxiliary,
            .stationary,
            .ignoresCycle,
        ]
        panel.animationBehavior = .none
        panel.isReleasedWhenClosed = false
        panel.contentView = pillView
        return panel
    }

    /// Sizes the panel for the current content. The bottom edge stays put,
    /// so the strip never moves under the pointer when the action row appears.
    private func layoutPanel() {
        let size = NSSize(width: PillView.width, height: pillView.preferredHeight)
        panel.ignoresMouseEvents = !pillView.acceptsMouse
        guard let screen = NSScreen.main ?? NSScreen.screens.first else {
            panel.setContentSize(size)
            pillView.frame = NSRect(origin: .zero, size: size)
            return
        }
        let visible = screen.visibleFrame
        panel.setFrame(
            NSRect(x: visible.midX - size.width / 2, y: visible.minY + 72, width: size.width, height: size.height),
            display: true
        )
        pillView.frame = NSRect(origin: .zero, size: size)
    }
}

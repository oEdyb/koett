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
}

/// A panel that shows above every app but never becomes key or main.
final class PillPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class RecordingOverlayController: NSObject {
    static let resultDisplayDuration: TimeInterval = 10

    let pillView = PillView(
        frame: NSRect(x: 0, y: 0, width: PillView.width, height: PillView.rowHeight)
    )
    private lazy var panel: NSPanel = makePanel()
    private weak var recorder: MicrophoneRecorder?
    private var startedAt: TimeInterval?
    private var timer: Timer?
    private var lastRedraw: TimeInterval = 0
    private var dismissWorkItem: DispatchWorkItem?
    private var resultText: String?
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
        pillView.onHoverChange = { [weak self] in self?.layoutPanel() }
        pillView.pauseButton.action = { [weak self] in self?.togglePause() }
        pillView.historyButton.action = { [weak self] in
            guard let self else { return }
            switch pillView.content {
            case .done, .notPasted, .media: hide()
            default: break
            }
            onOpenHistory?()
        }
        pillView.copyButton.action = { [weak self] in self?.copyResult() }
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

    func start(recorder: MicrophoneRecorder, controls: RecordingControls? = nil) {
        stop()
        self.recorder = recorder
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
        pillView.timeline.reset()
        show(.status(text))
    }

    func showProgress(_ text: String, accessibilityLabel: String, fraction: Double) {
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

    /// Replaces the transcribing pill after a successful paste.
    func showDone(text: String) {
        guard isShowingTranscribing else { return }
        resultText = text
        show(.done(duration: pillView.elapsed))
        dismiss(after: Self.resultDisplayDuration)
    }

    /// Replaces the transcribing pill when the text was saved but not pasted.
    /// Returns false when another message already replaced it.
    @discardableResult
    func showNotPasted(text: String) -> Bool {
        guard isShowingTranscribing else { return false }
        resultText = text
        show(.notPasted)
        recordFailure("Not pasted · text saved")
        dismiss(after: Self.resultDisplayDuration)
        return true
    }

    func showResult(_ text: String, message: String) {
        pillView.timeline.reset()
        resultText = text
        show(.media(preview: message))
        latestOutcome = .result(message)
        onLatestOutcomeChange?()
        dismiss(after: Self.resultDisplayDuration)
    }

    func showError(_ text: String) {
        showStatus(text)
        recordFailure(text)
        dismiss(after: 5)
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

    // MARK: Private

    private func show(_ content: PillContent) {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
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
        timer?.invalidate()
        timer = nil
        recorder = nil
        isShowingTranscribing = false
        resultText = nil
        pillView.setHovered(false)
        panel.orderOut(nil)
    }

    private func togglePause() {
        guard let recorder, recorder.isRecording, pillView.canPause else { return }
        recorder.isPaused.toggle()
        pillView.content = .recording(paused: recorder.isPaused)
    }

    private func copyResult() {
        guard let resultText else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        if pasteboard.setString(resultText, forType: .string) {
            pillView.copyTitle = "Copied"
            dismiss(after: 1.5)
        } else {
            pillView.copyTitle = "Copy failed"
        }
    }

    private func dismiss(after delay: TimeInterval) {
        dismissWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            guard let self else { return }
            if pillView.hovered && pillView.showsActions {
                dismiss(after: 1)
            } else {
                stop()
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

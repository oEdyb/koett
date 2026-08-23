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
}

@MainActor
final class RecordingOverlayController: NSObject {
    private static let fullSize = NSSize(width: 236, height: 48)
    private static let resultSize = NSSize(width: 250, height: 48)

    private let meterView = RecordingMeterView(
        frame: NSRect(origin: .zero, size: fullSize)
    )
    private lazy var panel: NSPanel = makePanel()
    private weak var recorder: AVAudioRecorder?
    private var timer: Timer?
    private var dismissWorkItem: DispatchWorkItem?
    private var smoothedLevel: CGFloat = 0

    func start(recorder: AVAudioRecorder) {
        stop()
        self.recorder = recorder
        smoothedLevel = 0
        meterView.reset()
        setPanelSize(Self.fullSize)
        panel.ignoresMouseEvents = true
        positionPanel()
        panel.orderFrontRegardless()

        let timer = Timer(
            timeInterval: 1.0 / 30.0,
            target: self,
            selector: #selector(refresh(_:)),
            userInfo: nil,
            repeats: true
        )
        timer.tolerance = 1.0 / 300.0
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
        timer.fire()
    }

    func stop() {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        timer?.invalidate()
        timer = nil
        recorder = nil
        meterView.onCopy = nil
        panel.orderOut(nil)
    }

    func showStatus(_ text: String) {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        timer?.invalidate()
        timer = nil
        recorder = nil
        meterView.showStatus(text)
        setPanelSize(Self.fullSize)
        panel.ignoresMouseEvents = true
        positionPanel()
        panel.orderFrontRegardless()
    }

    func showProgress(_ text: String, fraction: Double) {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        timer?.invalidate()
        timer = nil
        recorder = nil
        meterView.showProgress(text, fraction: fraction)
        setPanelSize(Self.fullSize)
        panel.ignoresMouseEvents = true
        positionPanel()
        panel.orderFrontRegardless()
    }

    func showTransientStatus(_ text: String, duration: TimeInterval = 2) {
        showStatus(text)
        dismiss(after: duration)
    }

    func showResult(_ text: String, message: String) {
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        timer?.invalidate()
        timer = nil
        recorder = nil
        meterView.showResult(message)
        setPanelSize(Self.resultSize)
        meterView.onCopy = { [weak self] in
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            guard pasteboard.setString(text, forType: .string) else { return }
            self?.meterView.showCopied()
            self?.dismiss(after: 1)
        }
        panel.ignoresMouseEvents = false
        positionPanel()
        panel.orderFrontRegardless()
    }

    func showError(_ text: String) {
        showStatus(text)
        dismiss(after: 5)
    }

    private func dismiss(after delay: TimeInterval) {
        dismissWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self] in
            self?.stop()
        }
        dismissWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: workItem)
    }

    @objc private func refresh(_ timer: Timer) {
        guard let recorder, recorder.isRecording else {
            stop()
            return
        }

        recorder.updateMeters()
        let target = RecordingOverlayFormat.level(
            forDecibels: recorder.averagePower(forChannel: 0)
        )
        smoothedLevel = (smoothedLevel * 0.55) + (target * 0.45)
        meterView.update(
            level: smoothedLevel,
            elapsed: recorder.currentTime
        )
    }

    private func makePanel() -> NSPanel {
        let panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: Self.fullSize),
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
        panel.contentView = meterView
        return panel
    }

    private func setPanelSize(_ size: NSSize) {
        panel.setContentSize(size)
        meterView.frame = NSRect(origin: .zero, size: size)
    }

    private func positionPanel() {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let frame = screen.visibleFrame
        panel.setFrameOrigin(NSPoint(
            x: frame.midX - (panel.frame.width / 2),
            y: frame.minY + 72
        ))
    }
}

@MainActor
private final class RecordingMeterView: NSView {
    private enum Content {
        case recording
        case status(String)
        case progress(text: String, fraction: CGFloat)
        case result(message: String, copied: Bool)
    }

    private static let barCount = 17
    private var levels = [CGFloat](repeating: 0, count: barCount)
    private var elapsedText = "0:00"
    private var content = Content.recording
    private var copyHovered = false
    private var copyTrackingArea: NSTrackingArea?
    var onCopy: (() -> Void)?

    override var isOpaque: Bool { false }

    func reset() {
        content = .recording
        setAccessibilityElement(false)
        setAccessibilityValue(nil)
        copyHovered = false
        levels = [CGFloat](repeating: 0, count: Self.barCount)
        elapsedText = "0:00"
        needsDisplay = true
    }

    func showStatus(_ text: String) {
        content = .status(text)
        setAccessibilityElement(true)
        setAccessibilityRole(.staticText)
        setAccessibilityLabel(text)
        setAccessibilityValue(nil)
        copyHovered = false
        onCopy = nil
        needsDisplay = true
    }

    func showProgress(_ text: String, fraction: Double) {
        let clamped = CGFloat(min(1, max(0, fraction)))
        content = .progress(text: text, fraction: clamped)
        setAccessibilityElement(true)
        setAccessibilityRole(.progressIndicator)
        setAccessibilityLabel(text)
        setAccessibilityValue("\(Int((clamped * 100).rounded())) percent")
        copyHovered = false
        onCopy = nil
        needsDisplay = true
    }

    func showResult(_ message: String) {
        content = .result(message: message, copied: false)
        copyHovered = false
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel("\(message). Copy transcript")
        setAccessibilityValue(nil)
        window?.invalidateCursorRects(for: self)
        needsDisplay = true
    }

    func showCopied() {
        guard case .result = content else { return }
        content = .result(message: "Copied", copied: true)
        copyHovered = false
        setAccessibilityLabel("Transcript copied")
        window?.invalidateCursorRects(for: self)
        needsDisplay = true
    }

    func update(level: CGFloat, elapsed: TimeInterval) {
        levels.removeFirst()
        levels.append(level)
        elapsedText = RecordingOverlayFormat.elapsedTime(elapsed)
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        let pillRect = bounds.insetBy(dx: 1, dy: 1)
        let pill = NSBezierPath(
            roundedRect: pillRect,
            xRadius: pillRect.height / 2,
            yRadius: pillRect.height / 2
        )
        NSColor(srgbRed: 0.075, green: 0.075, blue: 0.08, alpha: 0.97).setFill()
        pill.fill()
        NSColor.white.withAlphaComponent(0.10).setStroke()
        pill.lineWidth = 1
        pill.stroke()

        switch content {
        case .recording:
            drawRecording()
        case .status(let text):
            drawStatus(text)
        case .progress(let text, let fraction):
            drawProgress(text: text, fraction: fraction)
        case .result(let message, let copied):
            drawResult(message: message, copied: copied)
        }
    }

    override func mouseDown(with event: NSEvent) {
        guard case .result(_, copied: false) = content,
              copyButtonRect.contains(convert(event.locationInWindow, from: nil)) else {
            return
        }
        onCopy?()
    }

    override func mouseEntered(with event: NSEvent) {
        guard case .result(_, copied: false) = content else { return }
        copyHovered = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        copyHovered = false
        needsDisplay = true
    }

    override func updateTrackingAreas() {
        if let copyTrackingArea {
            removeTrackingArea(copyTrackingArea)
        }
        let trackingArea = NSTrackingArea(
            rect: copyButtonRect,
            options: [.mouseEnteredAndExited, .activeAlways],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea)
        copyTrackingArea = trackingArea
        super.updateTrackingAreas()
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        guard case .result(_, copied: false) = content else { return }
        addCursorRect(copyButtonRect, cursor: .pointingHand)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func accessibilityPerformPress() -> Bool {
        guard case .result(_, copied: false) = content else { return false }
        onCopy?()
        return true
    }

    private var copyButtonRect: NSRect {
        NSRect(x: bounds.maxX - 90, y: 8, width: 80, height: 32)
    }

    private func drawRecording() {
        let coral = NSColor(
            srgbRed: 1.0,
            green: 0.31,
            blue: 0.26,
            alpha: 1
        )
        coral.setFill()
        NSBezierPath(
            ovalIn: NSRect(x: 16, y: bounds.midY - 4, width: 8, height: 8)
        ).fill()

        let barWidth: CGFloat = 3
        let barStep: CGFloat = 6
        let waveformStartX: CGFloat = 39
        for (index, level) in levels.enumerated() {
            let height = 3 + (level * 24)
            let rect = NSRect(
                x: waveformStartX + (CGFloat(index) * barStep),
                y: bounds.midY - (height / 2),
                width: barWidth,
                height: height
            )
            let bar = NSBezierPath(roundedRect: rect, xRadius: 1.5, yRadius: 1.5)
            if (6...10).contains(index) {
                coral.setFill()
            } else {
                NSColor.white.withAlphaComponent(0.82).setFill()
            }
            bar.fill()
        }

        NSColor.white.withAlphaComponent(0.13).setFill()
        NSBezierPath(rect: NSRect(x: 153, y: 12, width: 1, height: 24)).fill()

        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .medium),
            .foregroundColor: NSColor.white.withAlphaComponent(0.90),
        ]
        let timerText = NSAttributedString(string: elapsedText, attributes: attributes)
        let timerSize = timerText.size()
        timerText.draw(at: NSPoint(
            x: 194 - (timerSize.width / 2),
            y: bounds.midY - (timerSize.height / 2)
        ))
    }

    private func drawStatus(_ text: String) {
        let coral = NSColor(
            srgbRed: 1.0,
            green: 0.31,
            blue: 0.26,
            alpha: 1
        )
        coral.setFill()
        NSBezierPath(
            ovalIn: NSRect(x: 17, y: bounds.midY - 4, width: 8, height: 8)
        ).fill()

        drawText(
            text,
            in: NSRect(x: 36, y: 0, width: 184, height: bounds.height),
            alignment: .left
        )
    }

    private func drawProgress(text: String, fraction: CGFloat) {
        let trackRect = NSRect(x: 36, y: 10, width: 184, height: 3)
        let track = NSBezierPath(
            roundedRect: trackRect,
            xRadius: trackRect.height / 2,
            yRadius: trackRect.height / 2
        )
        NSColor.white.withAlphaComponent(0.14).setFill()
        track.fill()

        if fraction > 0 {
            let fillRect = NSRect(
                x: trackRect.minX,
                y: trackRect.minY,
                width: trackRect.width * fraction,
                height: trackRect.height
            )
            let fill = NSBezierPath(
                roundedRect: fillRect,
                xRadius: fillRect.height / 2,
                yRadius: fillRect.height / 2
            )
            Self.coral.setFill()
            fill.fill()
        }

        drawText(
            text,
            in: NSRect(x: 36, y: 12, width: 184, height: 32),
            alignment: .left
        )
    }

    private func drawResult(message: String, copied: Bool) {
        drawTranscriptMark()
        drawText(
            message,
            in: NSRect(
                x: 44,
                y: 0,
                width: copyButtonRect.minX - 54,
                height: bounds.height
            ),
            alignment: .left
        )

        let button = NSBezierPath(
            roundedRect: copyButtonRect,
            xRadius: copyButtonRect.height / 2,
            yRadius: copyButtonRect.height / 2
        )
        let buttonAlpha: CGFloat = copied ? 0.16 : (copyHovered ? 0.20 : 0.12)
        NSColor.white.withAlphaComponent(buttonAlpha).setFill()
        button.fill()

        let symbolName = copied ? "checkmark" : "doc.on.doc"
        let pointSize = NSImage.SymbolConfiguration(pointSize: 13, weight: .semibold)
        let color = NSImage.SymbolConfiguration(paletteColors: [
            copied ? Self.coral : NSColor.white.withAlphaComponent(0.92),
        ])
        guard let symbol = NSImage(
            systemSymbolName: symbolName,
            accessibilityDescription: copied ? "Copied" : "Copy transcript"
        )?.withSymbolConfiguration(pointSize.applying(color)) else {
            return
        }
        let action = copied ? "Copied" : "Copy"
        let actionAttributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12.5, weight: .semibold),
            .foregroundColor: NSColor.white.withAlphaComponent(0.94),
        ]
        let actionText = NSAttributedString(string: action, attributes: actionAttributes)
        let symbolSize = symbol.size
        let actionSize = actionText.size()
        let spacing: CGFloat = 6
        let contentWidth = symbolSize.width + spacing + actionSize.width
        let startX = copyButtonRect.midX - (contentWidth / 2)
        symbol.draw(in: NSRect(
            x: startX,
            y: copyButtonRect.midY - (symbolSize.height / 2),
            width: symbolSize.width,
            height: symbolSize.height
        ))
        actionText.draw(at: NSPoint(
            x: startX + symbolSize.width + spacing,
            y: copyButtonRect.midY - (actionSize.height / 2)
        ))
    }

    private static let coral = NSColor(
        srgbRed: 1.0,
        green: 0.31,
        blue: 0.26,
        alpha: 1
    )

    private func drawTranscriptMark() {
        let heights: [CGFloat] = [7, 13, 19, 13, 7]
        let barWidth: CGFloat = 2.5
        let gap: CGFloat = 2
        Self.coral.setFill()
        for (index, height) in heights.enumerated() {
            let rect = NSRect(
                x: 16 + (CGFloat(index) * (barWidth + gap)),
                y: bounds.midY - (height / 2),
                width: barWidth,
                height: height
            )
            NSBezierPath(
                roundedRect: rect,
                xRadius: barWidth / 2,
                yRadius: barWidth / 2
            ).fill()
        }
    }

    private func drawText(
        _ text: String,
        in rect: NSRect,
        alignment: NSTextAlignment,
        size: CGFloat = 13
    ) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = alignment
        paragraph.lineBreakMode = .byTruncatingTail
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: size, weight: .medium),
            .foregroundColor: NSColor.white.withAlphaComponent(0.90),
            .paragraphStyle: paragraph,
        ]
        let string = NSAttributedString(string: text, attributes: attributes)
        let textSize = string.size()
        string.draw(in: NSRect(
            x: rect.minX,
            y: rect.midY - (textSize.height / 2),
            width: rect.width,
            height: textSize.height
        ))
    }
}

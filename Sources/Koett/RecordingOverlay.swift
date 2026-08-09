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
}

@MainActor
final class RecordingOverlayController: NSObject {
    private static let size = NSSize(width: 236, height: 48)

    private let meterView = RecordingMeterView(frame: NSRect(origin: .zero, size: size))
    private lazy var panel: NSPanel = makePanel()
    private weak var recorder: AVAudioRecorder?
    private var timer: Timer?
    private var smoothedLevel: CGFloat = 0

    func start(recorder: AVAudioRecorder) {
        stop()
        self.recorder = recorder
        smoothedLevel = 0
        meterView.reset()
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
        timer?.invalidate()
        timer = nil
        recorder = nil
        panel.orderOut(nil)
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
            contentRect: NSRect(origin: .zero, size: Self.size),
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

    private func positionPanel() {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let frame = screen.visibleFrame
        panel.setFrameOrigin(NSPoint(
            x: frame.midX - (Self.size.width / 2),
            y: frame.minY + 72
        ))
    }
}

@MainActor
private final class RecordingMeterView: NSView {
    private static let barCount = 17
    private var levels = [CGFloat](repeating: 0, count: barCount)
    private var elapsedText = "0:00"

    override var isOpaque: Bool { false }

    func reset() {
        levels = [CGFloat](repeating: 0, count: Self.barCount)
        elapsedText = "0:00"
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
}

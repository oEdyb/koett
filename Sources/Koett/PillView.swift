import AppKit

enum PillContent: Equatable {
    case recording(paused: Bool)
    case transcribing(fraction: Double?)
    case done(duration: TimeInterval)
    case notPasted
    case media(preview: String)
    case status(String)
    case progress(text: String, fraction: Double)
}

/// The recording pill: a whole-dictation timeline in a 48 px row, with an
/// action row above it. The bottom row never moves when the action row shows.
@MainActor
final class PillView: NSView {
    static let width: CGFloat = 300
    static let rowHeight: CGFloat = 48
    static let actionHeight: CGFloat = 40
    private static let stripX: CGFloat = 44
    private static let step: CGFloat = 3

    var content = PillContent.status("") {
        didSet {
            guard content != oldValue else { return }
            refreshControls()
            refreshAccessibility()
            needsDisplay = true
        }
    }
    var timeline = PillTimeline()
    var elapsed: TimeInterval = 0
    var spin: CGFloat = 0
    var modelName = "" {
        didSet { needsDisplay = true }
    }
    var hasRecordingControls = false {
        didSet { refreshControls() }
    }
    var canPause = false {
        didSet { refreshControls() }
    }
    private(set) var hovered = false
    var onHoverChange: (() -> Void)?

    let cancelButton = PillButton(title: "Cancel")
    let pauseButton = PillButton(title: "Pause")
    let pasteButton = PillButton(title: "Paste again")
    let copyButton = PillButton(title: "Copy")
    let historyButton = PillButton(title: "History")
    let statusElement = NSView()
    private var trackingArea: NSTrackingArea?

    override init(frame: NSRect) {
        super.init(frame: frame)
        statusElement.setAccessibilityElement(true)
        addSubview(statusElement)
        for button in [cancelButton, pauseButton, pasteButton, copyButton, historyButton] {
            button.isHidden = true
            addSubview(button)
        }
        refreshControls()
        refreshAccessibility()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override var isOpaque: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    var showsActions: Bool {
        switch content {
        case .recording, .transcribing: hasRecordingControls && hovered
        case .done, .notPasted, .media: true
        case .status, .progress: false
        }
    }

    var acceptsMouse: Bool {
        switch content {
        case .recording, .transcribing: hasRecordingControls
        case .done, .notPasted, .media: true
        case .status, .progress: false
        }
    }

    var preferredHeight: CGFloat {
        showsActions ? Self.rowHeight + Self.actionHeight : Self.rowHeight
    }

    var copyTitle: String {
        get { copyButton.title }
        set { copyButton.title = newValue }
    }

    func setHovered(_ hovered: Bool) {
        guard self.hovered != hovered else { return }
        self.hovered = hovered
        refreshControls()
        onHoverChange?()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        refreshControls()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        refreshControls()
        needsDisplay = true
    }

    override func mouseEntered(with event: NSEvent) { setHovered(true) }
    override func mouseExited(with event: NSEvent) { setHovered(false) }

    override func updateTrackingAreas() {
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(
            rect: .zero,
            options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self
        )
        addTrackingArea(area)
        trackingArea = area
        super.updateTrackingAreas()
    }

    private var palette: PillPalette { PillPalette.resolve(effectiveAppearance) }
    private var hasStrip: Bool {
        guard !timeline.isEmpty else { return false }
        switch content {
        case .recording, .done: return true
        case .transcribing: return true
        default: return false
        }
    }

    // MARK: Controls

    private func refreshControls() {
        let actions = showsActions && bounds.height > Self.rowHeight
        let recording = if case .recording = content { true } else { false }
        let paused = if case .recording(true) = content { true } else { false }
        let notPasted = content == .notPasted
        let result = switch content {
        case .done, .notPasted, .media: true
        default: false
        }
        let transcribing = if case .transcribing = content { true } else { false }
        cancelButton.isHidden = !(actions && (recording || transcribing))
        pauseButton.isHidden = !(actions && recording && canPause)
        pauseButton.title = paused ? "Resume" : "Pause"
        pasteButton.isHidden = !(actions && notPasted)
        copyButton.isHidden = !(actions && result)
        historyButton.isHidden = !actions

        let palette = palette
        let y = Self.rowHeight + (Self.actionHeight - PillButton.height) / 2 + 2
        var left: CGFloat = 12
        for button in [cancelButton, pauseButton, pasteButton] where !button.isHidden {
            button.frame = NSRect(x: left, y: y, width: button.fittingWidth, height: PillButton.height)
            left = button.frame.maxX + 6
        }
        var right = bounds.width - 12
        for button in [historyButton, copyButton] where !button.isHidden {
            let width = button.fittingWidth
            button.frame = NSRect(x: right - width, y: y, width: width, height: PillButton.height)
            right = button.frame.minX - 6
        }
        actionLabelRect = NSRect(x: left + 4, y: y, width: max(0, right - left - 8), height: PillButton.height)
        for button in [cancelButton, pauseButton, pasteButton, copyButton, historyButton] {
            button.palette = palette
        }
        statusElement.frame = NSRect(x: 0, y: 0, width: bounds.width, height: Self.rowHeight)
        needsDisplay = true
    }

    private var actionLabelRect = NSRect.zero

    private var actionLabel: (String, NSTextAlignment)? {
        switch content {
        case .recording: (modelName, .right)
        case .done(let duration): ("Done · \(RecordingOverlayFormat.elapsedTime(duration))", .left)
        case .media: ("Media transcript", .left)
        default: nil
        }
    }

    // MARK: Accessibility

    private func refreshAccessibility() {
        let element = statusElement
        element.setAccessibilityMinValue(nil)
        element.setAccessibilityMaxValue(nil)
        element.setAccessibilityValueDescription(nil)
        func staticText(_ value: String) {
            element.setAccessibilityRole(.staticText)
            element.setAccessibilityLabel("Koett status")
            element.setAccessibilityValue(value)
        }
        func progress(_ label: String, _ fraction: Double) {
            let clamped = min(1, max(0, fraction))
            element.setAccessibilityRole(.progressIndicator)
            element.setAccessibilityLabel(label)
            element.setAccessibilityValue(NSNumber(value: clamped))
            element.setAccessibilityMinValue(NSNumber(value: 0))
            element.setAccessibilityMaxValue(NSNumber(value: 1))
            element.setAccessibilityValueDescription("\(Int((clamped * 100).rounded())) percent")
        }
        switch content {
        case .recording(let paused): staticText(paused ? "Paused" : "Recording")
        case .transcribing(let fraction?): progress("Transcribing", fraction)
        case .transcribing(nil): staticText("Transcribing…")
        case .done(let duration): staticText("Done · \(RecordingOverlayFormat.elapsedTime(duration))")
        case .notPasted: staticText("Not pasted · text saved")
        case .media(let preview): staticText(preview)
        case .status(let text): staticText(text)
        case .progress(let text, let fraction):
            progress(text, fraction)
        }
    }

    // MARK: Drawing

    override func draw(_ dirtyRect: NSRect) {
        let palette = palette
        let tall = bounds.height > Self.rowHeight
        let radius: CGFloat = tall ? 22 : 24
        let pill = NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: radius, yRadius: radius)
        palette.background.setFill()
        pill.fill()
        let drawing = timeline.isDrawing && { if case .recording = content { true } else { false } }()
        if drawing {
            palette.accent.setStroke()
            pill.lineWidth = 2
        } else {
            palette.stroke.setStroke()
            pill.lineWidth = 1
        }
        pill.stroke()

        if tall, let (label, alignment) = actionLabel, !label.isEmpty,
           alignment == .left || Self.labelWidth(label) <= actionLabelRect.width {
            drawText(label, in: actionLabelRect, alignment: alignment, size: 12, color: palette.text.withAlphaComponent(0.62), weight: .medium)
        }

        let cy = Self.rowHeight / 2
        drawGlyph(cy: cy, palette: palette, drawing: drawing)
        if hasStrip {
            drawStrip(cy: cy, palette: palette)
        }
        drawRowText(cy: cy, palette: palette)
    }

    private func drawGlyph(cy: CGFloat, palette: PillPalette, drawing: Bool) {
        palette.accent.set()
        let path = NSBezierPath()
        path.lineCapStyle = .round
        path.lineJoinStyle = .round
        switch content {
        case .recording(paused: true):
            path.lineWidth = 2.5
            for x: CGFloat in [20.5, 26.5] {
                path.move(to: NSPoint(x: x, y: cy - 5))
                path.line(to: NSPoint(x: x, y: cy + 5))
            }
            path.stroke()
        case .recording where drawing:
            path.lineWidth = 1.8
            path.move(to: NSPoint(x: 18, y: cy - 6))
            path.line(to: NSPoint(x: 19, y: cy - 2))
            path.line(to: NSPoint(x: 27, y: cy + 6))
            path.line(to: NSPoint(x: 29.5, y: cy + 3.5))
            path.line(to: NSPoint(x: 21.5, y: cy - 4.5))
            path.close()
            path.stroke()
        case .recording, .status, .progress:
            NSBezierPath(ovalIn: NSRect(x: 19, y: cy - 4, width: 8, height: 8)).fill()
        case .transcribing:
            let ring = NSBezierPath(ovalIn: NSRect(x: 17, y: cy - 6, width: 12, height: 12))
            ring.lineWidth = 2
            palette.dim.setStroke()
            ring.stroke()
            let arc = NSBezierPath()
            arc.lineWidth = 2
            arc.lineCapStyle = .round
            let start = -spin * 180 / .pi
            arc.appendArc(withCenter: NSPoint(x: 23, y: cy), radius: 6, startAngle: start, endAngle: start - 92, clockwise: true)
            palette.accent.setStroke()
            arc.stroke()
        case .done:
            path.lineWidth = 2
            path.move(to: NSPoint(x: 17.5, y: cy - 0.5))
            path.line(to: NSPoint(x: 21.5, y: cy - 4.5))
            path.line(to: NSPoint(x: 29, y: cy + 4))
            path.stroke()
        case .notPasted:
            path.lineWidth = 1.8
            path.move(to: NSPoint(x: 23, y: cy + 7))
            path.line(to: NSPoint(x: 30, y: cy - 6))
            path.line(to: NSPoint(x: 16, y: cy - 6))
            path.close()
            path.move(to: NSPoint(x: 23, y: cy + 2))
            path.line(to: NSPoint(x: 23, y: cy - 1.5))
            path.stroke()
            NSBezierPath(ovalIn: NSRect(x: 22, y: cy - 4.8, width: 2, height: 2)).fill()
        case .media:
            for (index, height) in [7.0, 13.0, 19.0, 13.0, 7.0].enumerated() {
                NSBezierPath(
                    roundedRect: NSRect(x: 14 + CGFloat(index) * 4.5, y: cy - height / 2, width: 2.5, height: height),
                    xRadius: 1.25,
                    yRadius: 1.25
                ).fill()
            }
        }
    }

    private func drawStrip(cy: CGFloat, palette: PillPalette) {
        let head: Int? = if case .transcribing(let fraction?) = content {
            timeline.decodeHead(fraction: fraction)
        } else {
            nil
        }
        let transcribing = if case .transcribing = content { true } else { false }
        for sketch in timeline.sketches {
            let span = timeline.span(of: sketch)
            let rect = NSRect(
                x: Self.stripX + CGFloat(span.lowerBound) * Self.step - 1,
                y: cy - 15,
                width: CGFloat(span.count) * Self.step + 1,
                height: 30
            )
            let band = NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3)
            palette.band.setFill()
            band.fill()
            if sketch.live {
                palette.accent.setStroke()
                band.lineWidth = 1.5
                band.stroke()
            }
        }
        for (index, level) in timeline.levels.enumerated() {
            let x = Self.stripX + CGFloat(index) * Self.step
            if level < PillTimeline.silentLevel {
                palette.dim.setFill()
                NSBezierPath(roundedRect: NSRect(x: x, y: cy - 1, width: 2, height: 2), xRadius: 1, yRadius: 1).fill()
                continue
            }
            let bright = head.map { index < $0 } ?? !transcribing
            (bright ? palette.bar : palette.dim).setFill()
            let height = 4 + CGFloat(level) * 22
            NSBezierPath(
                roundedRect: NSRect(x: x, y: cy - height / 2, width: 2, height: height),
                xRadius: 1,
                yRadius: 1
            ).fill()
        }
        if let head {
            palette.accent.setFill()
            NSBezierPath(
                roundedRect: NSRect(x: Self.stripX + CGFloat(head) * Self.step - 1.5, y: cy - 17, width: 2, height: 34),
                xRadius: 1,
                yRadius: 1
            ).fill()
        }
        let font = NSFont.systemFont(ofSize: 9, weight: .bold)
        for sketch in timeline.sketches {
            let x = Self.stripX + CGFloat(timeline.span(of: sketch).lowerBound) * Self.step + 5
            let y = cy + 15
            palette.accent.setFill()
            NSBezierPath(ovalIn: NSRect(x: x - 6.5, y: y - 6.5, width: 13, height: 13)).fill()
            let label = NSAttributedString(string: "\(sketch.index)", attributes: [
                .font: font,
                .foregroundColor: NSColor.white,
            ])
            let size = label.size()
            label.draw(at: NSPoint(x: x - size.width / 2, y: y - size.height / 2))
        }
    }

    private func drawRowText(cy: CGFloat, palette: PillPalette) {
        let row = NSRect(x: 0, y: 0, width: bounds.width, height: Self.rowHeight)
        let clockRect = NSRect(x: 238, y: 0, width: 46, height: row.height)
        let textRect = NSRect(x: 40, y: 0, width: 246, height: row.height)
        switch content {
        case .recording:
            drawText(RecordingOverlayFormat.elapsedTime(elapsed), in: clockRect, alignment: .right, monospaced: true, color: palette.text)
        case .transcribing where !hasStrip:
            drawText("Transcribing…", in: textRect, alignment: .left, color: palette.text)
        case .notPasted:
            drawText("Not pasted · text saved", in: textRect, alignment: .left, color: palette.text)
        case .media(let preview):
            drawText(preview, in: textRect, alignment: .left, color: palette.text)
        case .status(let text):
            drawText(text, in: textRect, alignment: .left, color: palette.text)
        case .progress(let text, let fraction):
            let track = NSRect(x: 40, y: 10, width: 244, height: 3)
            palette.dim.setFill()
            NSBezierPath(roundedRect: track, xRadius: 1.5, yRadius: 1.5).fill()
            let clamped = CGFloat(min(1, max(0, fraction)))
            if clamped > 0 {
                palette.accent.setFill()
                NSBezierPath(
                    roundedRect: NSRect(x: track.minX, y: track.minY, width: track.width * clamped, height: track.height),
                    xRadius: 1.5,
                    yRadius: 1.5
                ).fill()
            }
            drawText(text, in: NSRect(x: 40, y: 12, width: 244, height: 32), alignment: .left, color: palette.text)
        case .transcribing, .done:
            break
        }
    }

    private static func labelWidth(_ text: String) -> CGFloat {
        NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .medium)]).size().width
    }

    private func drawText(
        _ text: String,
        in rect: NSRect,
        alignment: NSTextAlignment,
        size: CGFloat = 13,
        monospaced: Bool = false,
        color: NSColor,
        weight: NSFont.Weight = .medium
    ) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = alignment
        paragraph.lineBreakMode = .byTruncatingTail
        let font = monospaced
            ? NSFont.monospacedDigitSystemFont(ofSize: size, weight: weight)
            : NSFont.systemFont(ofSize: size, weight: weight)
        let string = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: color,
            .paragraphStyle: paragraph,
        ])
        let height = string.size().height
        string.draw(in: NSRect(x: rect.minX, y: rect.midY - height / 2, width: rect.width, height: height))
    }
}

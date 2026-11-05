import AppKit

enum PillContent: Equatable {
    case recording(paused: Bool)
    case transcribing(fraction: Double?)
    case done(preview: String)
    case notPasted
    case media(preview: String)
    case status(String)
    case error(String)
    case progress(text: String, fraction: Double)
}

/// The recording pill. Every state is one 300×48 row; hover actions replace
/// the strip inside the row instead of growing the pill.
@MainActor
final class PillView: NSView {
    static let width: CGFloat = 300
    static let rowHeight: CGFloat = 48
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
    var recordingNotice: String? {
        didSet { refreshAccessibility(); needsDisplay = true }
    }
    var elapsed: TimeInterval = 0
    var spin: CGFloat = 0
    var modelName = ""
    var hasRecordingControls = false {
        didSet { refreshControls() }
    }
    var canPause = false {
        didSet { refreshControls() }
    }
    /// The state being morphed away from, drawn underneath at `1 - morph`.
    var morphFrom: PillContent? {
        didSet { needsDisplay = true }
    }
    /// 0 shows `morphFrom`, 1 shows `content`.
    var morph: CGFloat = 1 {
        didSet {
            copyButton.opacity = morphFrom == nil ? 1 : morph
            needsDisplay = true
        }
    }
    private(set) var hovered = false
    var onHoverChange: (() -> Void)?
    /// A click on the pill body (not a button) of a result or error.
    var onDismiss: (() -> Void)?

    let cancelButton = PillButton(title: "Cancel")
    let pauseButton = PillButton(title: "Pause")
    let pasteButton = PillButton(title: "Paste")
    let copyButton = PillButton(title: "Copy")
    let historyButton = PillButton(title: "History")
    let statusElement = NSView()
    private var trackingArea: NSTrackingArea?
    private var buttons: [PillButton] { [cancelButton, pauseButton, historyButton, pasteButton, copyButton] }

    override init(frame: NSRect) {
        super.init(frame: frame)
        statusElement.setAccessibilityElement(true)
        addSubview(statusElement)
        for button in buttons {
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

    /// True while hover buttons replace the strip.
    var showsActions: Bool {
        switch content {
        case .recording, .transcribing: hasRecordingControls && hovered
        default: false
        }
    }

    var acceptsMouse: Bool {
        switch content {
        case .recording, .transcribing: hasRecordingControls
        case .done, .notPasted, .media, .error: true
        case .status, .progress: false
        }
    }

    var preferredHeight: CGFloat { Self.rowHeight }

    var copyTitle: String {
        get { copyButton.title }
        set {
            copyButton.title = newValue
            refreshControls()
        }
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
    override func mouseDown(with event: NSEvent) {}

    override func mouseUp(with event: NSEvent) {
        switch content {
        case .done, .notPasted, .media, .error: onDismiss?()
        default: break
        }
    }

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

    // MARK: Controls

    private func refreshControls() {
        let actions = showsActions
        let recording = if case .recording = content { true } else { false }
        let paused = content == .recording(paused: true)
        let transcribing = if case .transcribing = content { true } else { false }
        let resultRow = switch content {
        case .done, .media: true
        default: false
        }
        cancelButton.isHidden = !(actions && (recording || transcribing))
        pauseButton.isHidden = !(actions && recording && canPause)
        pauseButton.title = paused ? "Resume" : "Pause"
        historyButton.isHidden = !actions
        pasteButton.isHidden = content != .notPasted
        copyButton.isHidden = !(resultRow || content == .notPasted)
        copyButton.opacity = morphFrom == nil ? 1 : morph

        let y = (Self.rowHeight - PillButton.height) / 2
        var left = Self.stripX - 4
        for button in [cancelButton, pauseButton, historyButton] where !button.isHidden {
            button.frame = NSRect(x: left, y: y, width: button.fittingWidth, height: PillButton.height)
            left = button.frame.maxX + 4
        }
        var right = bounds.width - 11
        for button in [copyButton, pasteButton] where !button.isHidden {
            let width = button.fittingWidth
            button.frame = NSRect(x: right - width, y: y, width: width, height: PillButton.height)
            right = button.frame.minX - 6
        }
        let palette = palette
        for button in buttons { button.palette = palette }
        statusElement.frame = NSRect(x: 0, y: 0, width: bounds.width, height: Self.rowHeight)
        needsDisplay = true
    }

    /// Right edge for row text, left of any right-hand buttons.
    private var textRight: CGFloat {
        let visible = [pasteButton, copyButton].filter { !$0.isHidden }
        return (visible.map(\.frame.minX).min() ?? bounds.width - 14) - 10
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
        case .recording(let paused): staticText(recordingNotice ?? (paused ? "Paused" : "Recording"))
        case .transcribing(let fraction?): progress("Transcribing", fraction)
        case .transcribing(nil): staticText("Transcribing…")
        case .done(let preview), .media(let preview): staticText(preview)
        case .notPasted: staticText("Not pasted · text saved")
        case .status(let text), .error(let text): staticText(text)
        case .progress(let text, let fraction): progress(text, fraction)
        }
    }

    // MARK: Drawing

    override func draw(_ dirtyRect: NSRect) {
        let palette = palette
        let pill = NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 23, yRadius: 23)
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

        if let morphFrom, morph < 1 {
            drawBody(morphFrom, palette: palette, alpha: max(0, 1 - morph * 1.6), shift: -8 * morph, drawing: false)
            drawBody(content, palette: palette, alpha: morph, shift: 8 * (1 - morph), drawing: drawing)
        } else {
            drawBody(content, palette: palette, alpha: 1, shift: 0, drawing: drawing)
        }
    }

    private func drawBody(_ content: PillContent, palette: PillPalette, alpha: CGFloat, shift: CGFloat, drawing: Bool) {
        guard alpha > 0, let context = NSGraphicsContext.current?.cgContext else { return }
        context.saveGState()
        context.setAlpha(alpha)
        context.beginTransparencyLayer(auxiliaryInfo: nil)
        let cy = Self.rowHeight / 2
        drawGlyph(content, cy: cy, palette: palette, drawing: drawing)
        context.translateBy(x: shift, y: 0)
        let stripContent = switch content {
        case .recording: !timeline.isEmpty && recordingNotice == nil
        case .transcribing: !timeline.isEmpty
        default: false
        }
        if stripContent && !(showsActions && content == self.content) {
            drawStrip(content, cy: cy, palette: palette)
        }
        drawRowText(content, palette: palette, hasStrip: stripContent)
        context.endTransparencyLayer()
        context.restoreGState()
    }

    private func drawGlyph(_ content: PillContent, cy: CGFloat, palette: PillPalette, drawing: Bool) {
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
        case .notPasted, .error:
            path.lineWidth = 1.8
            path.move(to: NSPoint(x: 23, y: cy + 7))
            path.line(to: NSPoint(x: 30, y: cy - 6))
            path.line(to: NSPoint(x: 16, y: cy - 6))
            path.close()
            path.move(to: NSPoint(x: 23, y: cy + 2))
            path.line(to: NSPoint(x: 23, y: cy - 1.5))
            path.stroke()
            NSBezierPath(ovalIn: NSRect(x: 22, y: cy - 4.8, width: 2, height: 2)).fill()
        case .done, .media:
            for (index, height) in [7.0, 13.0, 19.0, 13.0, 7.0].enumerated() {
                NSBezierPath(
                    roundedRect: NSRect(x: 14 + CGFloat(index) * 4.5, y: cy - height / 2, width: 2.5, height: height),
                    xRadius: 1.25,
                    yRadius: 1.25
                ).fill()
            }
        }
    }

    private func drawStrip(_ content: PillContent, cy: CGFloat, palette: PillPalette) {
        let transcribing = if case .transcribing = content { true } else { false }
        func x(_ unit: Double) -> CGFloat { Self.stripX + CGFloat(unit) * Self.step }
        let bands = timeline.sketches.compactMap { sketch in timeline.band(of: sketch).map { (sketch, $0) } }
        for (sketch, band) in bands {
            let rect = NSRect(x: x(band.lowerBound) - 1, y: cy - 15, width: CGFloat(band.upperBound - band.lowerBound) * Self.step + 1, height: 30)
            let shape = NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3)
            palette.band.setFill()
            shape.fill()
            if sketch.live {
                palette.accent.setStroke()
                shape.lineWidth = 1.5
                shape.stroke()
            }
        }
        let fade = 6.0
        for bar in timeline.bars() {
            let alpha = CGFloat(min(1, max(0, (bar.x + 1) / fade)))
            let left = x(bar.x)
            guard let level = bar.level, level >= PillTimeline.silentLevel else {
                palette.dim.withAlphaComponent(palette.dim.alphaComponent * alpha).setFill()
                NSBezierPath(roundedRect: NSRect(x: left, y: cy - 1, width: 2, height: 2), xRadius: 1, yRadius: 1).fill()
                continue
            }
            let color = transcribing ? palette.dim : palette.bar
            color.withAlphaComponent(color.alphaComponent * alpha).setFill()
            let height = 4 + CGFloat(level) * 22
            NSBezierPath(
                roundedRect: NSRect(x: left, y: cy - height / 2, width: 2, height: height),
                xRadius: 1,
                yRadius: 1
            ).fill()
        }
        let font = NSFont.systemFont(ofSize: 9, weight: .bold)
        for (sketch, band) in bands {
            let center = max(x(band.lowerBound) + 5, Self.stripX + 6.5)
            let y = cy + 15
            palette.accent.setFill()
            NSBezierPath(ovalIn: NSRect(x: center - 6.5, y: y - 6.5, width: 13, height: 13)).fill()
            let label = NSAttributedString(string: "\(sketch.index)", attributes: [
                .font: font,
                .foregroundColor: NSColor.white,
            ])
            let size = label.size()
            label.draw(at: NSPoint(x: center - size.width / 2, y: y - size.height / 2))
        }
    }

    private func drawRowText(_ content: PillContent, palette: PillPalette, hasStrip: Bool) {
        let height = Self.rowHeight
        let clockRect = NSRect(x: 238, y: 0, width: 46, height: height)
        let textRect = NSRect(x: 40, y: 0, width: 246, height: height)
        let resultRect = NSRect(x: 44, y: 0, width: max(0, textRight - 44), height: height)
        switch content {
        case .recording:
            if let recordingNotice, !showsActions {
                drawText(recordingNotice, in: NSRect(x: 44, y: 0, width: 180, height: height), alignment: .left, color: palette.text)
            }
            drawText(RecordingOverlayFormat.elapsedTime(elapsed), in: clockRect, alignment: .right, monospaced: true, color: palette.text)
        case .transcribing(let fraction?) where hasStrip:
            drawText("\(Int((fraction * 100).rounded()))%", in: clockRect, alignment: .right, monospaced: true, color: palette.text.withAlphaComponent(0.62))
        case .transcribing where !hasStrip:
            drawText("Transcribing…", in: textRect, alignment: .left, color: palette.text)
        case .transcribing:
            break
        case .notPasted:
            drawText("Not pasted · saved", in: resultRect, alignment: .left, color: palette.text)
        case .done(let preview), .media(let preview):
            drawText(preview, in: resultRect, alignment: .left, color: palette.text)
        case .status(let text), .error(let text):
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
        }
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

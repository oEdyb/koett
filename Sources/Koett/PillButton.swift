import AppKit

/// Small drawn button for the pill's action row. It acts on the first click
/// and never takes keyboard focus, so the app under the pill stays active.
@MainActor
final class PillButton: NSView {
    var title: String {
        didSet {
            setAccessibilityLabel(title)
            needsDisplay = true
        }
    }
    var palette = PillPalette.dark {
        didSet { needsDisplay = true }
    }
    var action: (() -> Void)?
    private var hovered = false
    private var trackingArea: NSTrackingArea?

    static let height: CGFloat = 26
    private static let font = NSFont.systemFont(ofSize: 12, weight: .semibold)

    init(title: String) {
        self.title = title
        super.init(frame: .zero)
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(title)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override var isOpaque: Bool { false }
    override var acceptsFirstResponder: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    var fittingWidth: CGFloat {
        ceil(NSAttributedString(string: title, attributes: [.font: Self.font]).size().width) + 20
    }

    override func draw(_ dirtyRect: NSRect) {
        let shape = NSBezierPath(roundedRect: bounds, xRadius: bounds.height / 2, yRadius: bounds.height / 2)
        (hovered ? palette.buttonHover : palette.button).setFill()
        shape.fill()
        let text = NSAttributedString(string: title, attributes: [
            .font: Self.font,
            .foregroundColor: palette.text,
        ])
        let size = text.size()
        text.draw(at: NSPoint(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2))
    }

    override func mouseDown(with event: NSEvent) {}

    override func mouseUp(with event: NSEvent) {
        guard bounds.contains(convert(event.locationInWindow, from: nil)) else { return }
        action?()
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
        needsDisplay = true
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

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .pointingHand)
    }

    override func accessibilityPerformPress() -> Bool {
        guard let action else { return false }
        action()
        return true
    }
}

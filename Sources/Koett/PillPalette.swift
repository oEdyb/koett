import AppKit

/// Pill colors for the system appearance. Matches the Linux `DARK`/`LIGHT` palettes.
struct PillPalette {
    let background: NSColor
    let stroke: NSColor
    let text: NSColor
    let bar: NSColor
    let dim: NSColor
    let accent: NSColor
    let band: NSColor
    let button: NSColor
    let buttonHover: NSColor

    static let dark = PillPalette(
        background: NSColor(srgbRed: 0.075, green: 0.075, blue: 0.08, alpha: 0.97),
        stroke: .white.withAlphaComponent(0.10),
        text: .white.withAlphaComponent(0.90),
        bar: .white.withAlphaComponent(0.82),
        dim: .white.withAlphaComponent(0.28),
        accent: NSColor(srgbRed: 1.0, green: 0.31, blue: 0.26, alpha: 1),
        band: NSColor(srgbRed: 1.0, green: 0.31, blue: 0.26, alpha: 0.24),
        button: .white.withAlphaComponent(0.10),
        buttonHover: .white.withAlphaComponent(0.20)
    )

    static let light = PillPalette(
        background: NSColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.97),
        stroke: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.12),
        text: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.90),
        bar: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.72),
        dim: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.22),
        accent: NSColor(srgbRed: 0.85, green: 0.267, blue: 0.227, alpha: 1),
        band: NSColor(srgbRed: 0.85, green: 0.267, blue: 0.227, alpha: 0.16),
        button: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.07),
        buttonHover: NSColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.14)
    )

    static func resolve(_ appearance: NSAppearance) -> PillPalette {
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .aqua ? light : dark
    }
}

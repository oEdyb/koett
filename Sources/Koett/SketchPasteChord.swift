import AppKit

/// Which chord pastes the contact sheet into an app. Terminal agents such as
/// Claude Code read a clipboard image on Control-V, while Command-V in a
/// terminal pastes only text. The text after the sheet always uses Command-V.
enum SketchPasteChord: Equatable, Sendable {
    case command
    case control

    static let overridesKey = "sketchImagePasteControlV"

    /// Terminals detected by bundle ID. VS Code and Cursor are left out: their
    /// integrated terminal shares one app with the editor and its focus is not
    /// readable without forcing Electron accessibility on, so they use the
    /// per-app override instead.
    static let terminals: Set<String> = [
        "com.apple.Terminal",
        "com.googlecode.iterm2",
        "com.mitchellh.ghostty",
        "net.kovidgoyal.kitty",
        "com.github.wez.wezterm",
        "org.alacritty",
        "dev.warp.Warp-Stable",
    ]

    var flags: CGEventFlags {
        switch self {
        case .command: .maskCommand
        case .control: .maskControl
        }
    }

    var displayName: String {
        switch self {
        case .command: "Command-V"
        case .control: "Control-V"
        }
    }

    static func isTerminal(_ bundleID: String?) -> Bool {
        bundleID.map(terminals.contains) ?? false
    }

    /// The per-app override wins; otherwise terminals get Control-V.
    static func forApp(_ bundleID: String?, overrides: [String: Bool]) -> SketchPasteChord {
        guard let bundleID else { return .command }
        let control = overrides[bundleID] ?? isTerminal(bundleID)
        return control ? .control : .command
    }

    static func overrides(in defaults: UserDefaults) -> [String: Bool] {
        defaults.dictionary(forKey: overridesKey) as? [String: Bool] ?? [:]
    }

    /// Flips the chord for one app, dropping the override when it matches detection.
    static func toggle(_ bundleID: String, in defaults: UserDefaults) {
        var overrides = overrides(in: defaults)
        let control = forApp(bundleID, overrides: overrides) == .command
        overrides[bundleID] = control == isTerminal(bundleID) ? nil : control
        defaults.set(overrides, forKey: overridesKey)
    }
}

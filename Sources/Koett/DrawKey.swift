import AppKit

/// The modifier held to draw while dictating. A hold draws only when it is the
/// only modifier down, so chords like Command-Shift-4 or Control-Shift-T never draw.
enum DrawKey: String, CaseIterable, Sendable {
    case shift
    case control
    case option

    static let defaultsKey = "drawKey"
    static let enabledDefaultsKey = "drawWhileDictating"
    static let pinSelectedTextDefaultsKey = "pinSelectedText"

    var flag: NSEvent.ModifierFlags {
        switch self {
        case .shift: .shift
        case .control: .control
        case .option: .option
        }
    }

    var displayName: String {
        switch self {
        case .shift: "Shift"
        case .control: "Control"
        case .option: "Option"
        }
    }

    func draws(_ flags: NSEvent.ModifierFlags) -> Bool {
        flags.intersection([.shift, .control, .option, .command]) == flag
    }

    /// True when a modifier-only dictation shortcut already uses this key.
    func conflicts(with dictation: ShortcutBinding) -> Bool {
        dictation.modifierOnly && dictation.modifiers.contains(flag)
    }

    /// The requested key, or the first free one when the dictation shortcut uses it.
    static func effective(_ requested: DrawKey, dictation: ShortcutBinding) -> DrawKey {
        guard requested.conflicts(with: dictation) else { return requested }
        return allCases.first { !$0.conflicts(with: dictation) } ?? requested
    }
}

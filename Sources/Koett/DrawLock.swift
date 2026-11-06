import AppKit

enum DrawLock {
    static let shortcuts: [ShortcutBinding] = [
        ShortcutBinding.custom(keyCode: 2, modifiers: [.control, .option], keyName: "D", modifierOnly: false),
        ShortcutBinding.custom(keyCode: 2, modifiers: [.control, .shift], keyName: "D", modifierOnly: false),
        ShortcutBinding.custom(keyCode: 2, modifiers: [.command, .option], keyName: "D", modifierOnly: false),
    ]

    static func shortcut(dictation: ShortcutBinding, media: ShortcutBinding, mode: KoettController.RecordingMode,
                         voiceOverEnabled: Bool = false) -> ShortcutBinding? {
        shortcuts.first {
            (!voiceOverEnabled || !$0.modifiers.contains([.control, .option]))
                && !ShortcutBinding.shortcutsConflict(dictation: dictation, media: $0, dictationUsesToggle: mode == .toggle)
                && !$0.conflicts(with: media)
        }
    }
}

struct DrawLockUndoKey {
    enum Action: Equatable { case undo, consume }
    private var down = false

    mutating func action(type: NSEvent.EventType, keyCode: UInt16, modifierFlags: UInt,
                         isRepeat: Bool, locked: Bool) -> Action? {
        guard keyCode == 6 else { return nil }
        if type == .keyUp, down { down = false; return .consume }
        guard type == .keyDown else { return nil }
        if down { return .consume }
        let modifiers = NSEvent.ModifierFlags(rawValue: modifierFlags).intersection(ShortcutBinding.relevantModifiers)
        guard locked, [.control, .command].contains(modifiers) else { return nil }
        down = true
        return isRepeat ? .consume : .undo
    }
}

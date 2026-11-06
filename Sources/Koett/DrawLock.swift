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

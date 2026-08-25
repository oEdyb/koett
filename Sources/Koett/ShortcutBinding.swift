import AppKit
import Foundation
import IOKit.hidsystem

struct ShortcutBinding: Codable, Equatable {
    static let relevantModifiers: NSEvent.ModifierFlags = [
        .command,
        .option,
        .control,
        .shift,
        .function,
    ]

    static let defaultDictation = ShortcutBinding(
        keyCodes: [58, 61],
        modifiersRawValue: NSEvent.ModifierFlags.option.rawValue,
        keyName: "Either Option",
        modifierOnly: true
    )

    static let defaultMedia = ShortcutBinding(
        keyCodes: [17],
        modifiersRawValue: NSEvent.ModifierFlags([.control, .shift]).rawValue,
        keyName: "T",
        modifierOnly: false
    )

    static let defaultAssistant = ShortcutBinding(
        keyCodes: [49],
        modifiersRawValue: NSEvent.ModifierFlags([.control, .shift]).rawValue,
        keyName: "Space",
        modifierOnly: false
    )

    let keyCodes: [UInt16]
    let modifiersRawValue: UInt
    let keyName: String
    let modifierOnly: Bool

    var modifiers: NSEvent.ModifierFlags {
        NSEvent.ModifierFlags(rawValue: modifiersRawValue)
    }

    var displayName: String {
        if keyName == "Either Option" {
            return keyName
        }
        if modifierOnly {
            if let keyCode = keyCodes.first,
               modifiers == Self.modifier(for: keyCode) {
                return keyName
            }
            return Self.modifierSymbols(modifiers)
        }
        return Self.modifierSymbols(modifiers) + keyName
    }

    var isValid: Bool {
        !keyCodes.isEmpty && (!modifierOnly || singleModifier != nil)
    }

    func matchesDown(
        type: NSEvent.EventType,
        keyCode: UInt16,
        modifierFlags: UInt,
        isRepeat: Bool
    ) -> Bool {
        guard !modifierOnly, keyCodes.contains(keyCode), !isRepeat else { return false }
        let flags = NSEvent.ModifierFlags(rawValue: modifierFlags)
            .intersection(.deviceIndependentFlagsMask)
            .intersection(Self.relevantModifiers)
        return type == .keyDown && flags == modifiers
    }

    func matchesUp(
        type: NSEvent.EventType,
        keyCode: UInt16,
        modifierFlags: UInt
    ) -> Bool {
        guard !modifierOnly, keyCodes.contains(keyCode) else { return false }
        return type == .keyUp
    }

    func conflicts(with other: ShortcutBinding) -> Bool {
        if modifierOnly,
           let modifier = singleModifier,
           other.modifiers.contains(modifier) {
            return true
        }
        if other.modifierOnly,
           let modifier = other.singleModifier,
           modifiers.contains(modifier) {
            return true
        }
        return modifierOnly == other.modifierOnly
            && modifiers == other.modifiers
            && !Set(keyCodes).isDisjoint(with: other.keyCodes)
    }

    static func shortcutsConflict(
        dictation: ShortcutBinding,
        media: ShortcutBinding,
        dictationUsesToggle: Bool
    ) -> Bool {
        guard dictation.conflicts(with: media) else { return false }
        if dictationUsesToggle,
           dictation.modifierOnly,
           !media.modifierOnly {
            return false
        }
        return true
    }

    static func custom(
        keyCode: UInt16,
        modifiers: NSEvent.ModifierFlags,
        keyName: String,
        modifierOnly: Bool
    ) -> ShortcutBinding {
        ShortcutBinding(
            keyCodes: [keyCode],
            modifiersRawValue: modifiers.intersection(relevantModifiers).rawValue,
            keyName: keyName,
            modifierOnly: modifierOnly
        )
    }

    static func migratedDictation(_ value: String?) -> ShortcutBinding {
        switch value {
        case "rightOption":
            return custom(
                keyCode: 61,
                modifiers: .option,
                keyName: "Right Option",
                modifierOnly: true
            )
        case "rightCommand":
            return custom(
                keyCode: 54,
                modifiers: .command,
                keyName: "Right Command",
                modifierOnly: true
            )
        default:
            return .defaultDictation
        }
    }

    static func modifier(for keyCode: UInt16) -> NSEvent.ModifierFlags? {
        switch keyCode {
        case 54, 55: .command
        case 56, 60: .shift
        case 58, 61: .option
        case 59, 62: .control
        case 63: .function
        default: nil
        }
    }

    static func keyName(keyCode: UInt16, characters: String?) -> String {
        let names: [UInt16: String] = [
            36: "Return",
            48: "Tab",
            49: "Space",
            51: "Delete",
            53: "Escape",
            76: "Enter",
            96: "F5",
            97: "F6",
            98: "F7",
            99: "F3",
            100: "F8",
            101: "F9",
            103: "F11",
            109: "F10",
            111: "F12",
            118: "F4",
            120: "F2",
            122: "F1",
            123: "←",
            124: "→",
            125: "↓",
            126: "↑",
        ]
        if let name = names[keyCode] {
            return name
        }
        let value = characters?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .uppercased()
        if let value, !value.isEmpty {
            return value
        }
        return "Key \(keyCode)"
    }

    static func modifierName(for keyCode: UInt16) -> String {
        switch keyCode {
        case 54: "Right Command"
        case 55: "Left Command"
        case 56: "Left Shift"
        case 60: "Right Shift"
        case 58: "Left Option"
        case 61: "Right Option"
        case 59: "Left Control"
        case 62: "Right Control"
        case 63: "Fn"
        default: "Modifier"
        }
    }

    private static func modifierSymbols(_ flags: NSEvent.ModifierFlags) -> String {
        var value = ""
        if flags.contains(.control) { value += "⌃" }
        if flags.contains(.option) { value += "⌥" }
        if flags.contains(.shift) { value += "⇧" }
        if flags.contains(.command) { value += "⌘" }
        if flags.contains(.function) { value += "fn" }
        return value
    }

    private var singleModifier: NSEvent.ModifierFlags? {
        let values: [NSEvent.ModifierFlags] = [
            .command,
            .option,
            .control,
            .shift,
            .function,
        ]
        let selected = values.filter { modifiers.contains($0) }
        return selected.count == 1 ? selected[0] : nil
    }
}

struct ModifierShortcutState {
    private var pressedKeyCodes = Set<UInt16>()
    private var chordUsed = false

    mutating func noteKeyDown() {
        if !pressedKeyCodes.isEmpty {
            chordUsed = true
        }
    }

    mutating func update(
        binding: ShortcutBinding,
        type: NSEvent.EventType,
        keyCode: UInt16,
        modifierFlags: UInt = 0,
        keyIsDown: Bool? = nil
    ) -> (down: Bool, up: Bool, releasedWithoutChord: Bool) {
        guard binding.modifierOnly, type == .flagsChanged else {
            return (false, false, false)
        }
        guard binding.keyCodes.contains(keyCode) else {
            if !pressedKeyCodes.isEmpty {
                chordUsed = true
            }
            return (false, false, false)
        }

        let wasActive = !pressedKeyCodes.isEmpty
        let eventIsDown = Self.eventReportsKeyDown(
            keyCode: keyCode,
            modifierFlags: modifierFlags
        ) ?? keyIsDown ?? !pressedKeyCodes.contains(keyCode)
        if eventIsDown {
            guard !pressedKeyCodes.contains(keyCode) else {
                return (false, false, false)
            }
            if !wasActive {
                let flags = NSEvent.ModifierFlags(rawValue: modifierFlags)
                    .intersection(.deviceIndependentFlagsMask)
                chordUsed = !flags.subtracting(binding.modifiers).isEmpty
            } else {
                chordUsed = true
            }
            pressedKeyCodes.insert(keyCode)
        } else {
            pressedKeyCodes.remove(keyCode)
        }
        let isActive = !pressedKeyCodes.isEmpty
        let releasedWithoutChord = wasActive && !isActive && !chordUsed
        if !isActive {
            chordUsed = false
        }
        return (
            !wasActive && isActive,
            wasActive && !isActive,
            releasedWithoutChord
        )
    }

    mutating func reset() {
        pressedKeyCodes.removeAll()
        chordUsed = false
    }

    private static func eventReportsKeyDown(
        keyCode: UInt16,
        modifierFlags: UInt
    ) -> Bool? {
        guard modifierFlags != 0 else { return nil }

        let deviceMask: UInt
        switch keyCode {
        case 54: deviceMask = UInt(NX_DEVICERCMDKEYMASK)
        case 55: deviceMask = UInt(NX_DEVICELCMDKEYMASK)
        case 56: deviceMask = UInt(NX_DEVICELSHIFTKEYMASK)
        case 60: deviceMask = UInt(NX_DEVICERSHIFTKEYMASK)
        case 58: deviceMask = UInt(NX_DEVICELALTKEYMASK)
        case 61: deviceMask = UInt(NX_DEVICERALTKEYMASK)
        case 59: deviceMask = UInt(NX_DEVICELCTLKEYMASK)
        case 62: deviceMask = UInt(NX_DEVICERCTLKEYMASK)
        case 63:
            deviceMask = NSEvent.ModifierFlags.function.rawValue
        default:
            return nil
        }
        return modifierFlags & deviceMask != 0
    }

    @discardableResult
    mutating func resync(
        binding: ShortcutBinding,
        isKeyDown: (UInt16) -> Bool
    ) -> Bool {
        guard binding.modifierOnly else {
            reset()
            return false
        }

        pressedKeyCodes = Set(binding.keyCodes.filter(isKeyDown))
        chordUsed = !pressedKeyCodes.isEmpty
        return !pressedKeyCodes.isEmpty
    }
}

@MainActor
enum ShortcutCapture {
    static func run(title: String) -> ShortcutBinding? {
        let previousApplication = NSWorkspace.shared.frontmostApplication
        defer {
            previousApplication?.activate(options: [])
        }
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = "Press a shortcut. Press Escape to cancel."
        alert.addButton(withTitle: "Cancel")

        let preview = NSTextField(labelWithString: "Waiting…")
        preview.alignment = .center
        preview.font = .systemFont(ofSize: 18, weight: .medium)
        preview.frame = NSRect(x: 0, y: 0, width: 240, height: 28)
        alert.accessoryView = preview

        var result: ShortcutBinding?
        var pendingModifier: ShortcutBinding?
        var waitingForRelease = false
        let monitor = NSEvent.addLocalMonitorForEvents(
            matching: [.keyDown, .flagsChanged]
        ) { event in
            let flags = event.modifierFlags
                .intersection(.deviceIndependentFlagsMask)
                .intersection(ShortcutBinding.relevantModifiers)

            if waitingForRelease {
                if event.type == .flagsChanged, flags.isEmpty {
                    NSApp.abortModal()
                }
                return nil
            }

            if event.type == .keyDown {
                if event.keyCode == 53 && flags.isEmpty {
                    NSApp.abortModal()
                    return nil
                }
                let binding = ShortcutBinding.custom(
                    keyCode: event.keyCode,
                    modifiers: flags,
                    keyName: ShortcutBinding.keyName(
                        keyCode: event.keyCode,
                        characters: event.charactersIgnoringModifiers
                    ),
                    modifierOnly: false
                )
                result = binding
                preview.stringValue = binding.displayName
                if flags.isEmpty {
                    NSApp.abortModal()
                } else {
                    waitingForRelease = true
                }
                return nil
            }

            if let modifier = ShortcutBinding.modifier(for: event.keyCode),
               flags.contains(modifier),
               flags.rawValue.nonzeroBitCount == 1 {
                let binding = ShortcutBinding.custom(
                    keyCode: event.keyCode,
                    modifiers: flags,
                    keyName: ShortcutBinding.modifierName(for: event.keyCode),
                    modifierOnly: true
                )
                pendingModifier = binding
                preview.stringValue = binding.displayName
            } else if !flags.isEmpty {
                pendingModifier = nil
                preview.stringValue = "Add a key"
            } else if flags.isEmpty, let pendingModifier {
                result = pendingModifier
                NSApp.abortModal()
            }
            return nil
        }
        defer {
            if let monitor {
                NSEvent.removeMonitor(monitor)
            }
        }

        _ = NSRunningApplication.current.activate(options: [.activateAllWindows])
        NSApp.activate(ignoringOtherApps: true)
        alert.window.makeKeyAndOrderFront(nil)
        let response = alert.runModal()
        return response == .abort ? result : nil
    }
}

import CoreGraphics

struct PasteEvents {
    let down: CGEvent
    let up: CGEvent

    static func make(flags: CGEventFlags) -> Self? {
        guard let source = CGEventSource(stateID: .privateState),
              let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else { return nil }
        down.flags = flags
        up.flags = flags
        return Self(down: down, up: up)
    }
}

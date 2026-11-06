import CoreGraphics

struct PasteEvents {
    let down: CGEvent
    let up: CGEvent

    static func make(flags: CGEventFlags) -> Self? {
        guard let source = CGEventSource(stateID: .privateState),
              let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else { return nil }
        down.flags = flags
        // Posting through the HID tap updates macOS modifier state even with a
        // private source. End the synthetic chord on key-up so the next paste
        // does not mistake our Command/Control flag for a held user key.
        up.flags = []
        return Self(down: down, up: up)
    }
}

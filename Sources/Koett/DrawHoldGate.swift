import AppKit

struct DrawHoldGate {
    private var seenUp = false
    private var spoiled = false
    private var otherKeys: Set<UInt16> = []

    mutating func noteKey(type: NSEvent.EventType, keyCode: UInt16) {
        if type == .keyDown { otherKeys.insert(keyCode) }
        if type == .keyUp { otherKeys.remove(keyCode) }
    }

    mutating func draws(key: DrawKey, flags: NSEvent.ModifierFlags) -> Bool {
        guard flags.contains(key.flag) else {
            seenUp = true
            spoiled = false
            return false
        }
        if !seenUp || !key.draws(flags) || !otherKeys.isEmpty {
            spoiled = true
        }
        return !spoiled
    }
}

import AppKit
import CoreGraphics

extension KoettController {
    static let drawingKeyEventMask: CGEventMask = [CGEventType.keyDown, .keyUp]
        .reduce(0) { $0 | (CGEventMask(1) << $1.rawValue) }

    var drawingKeyTapIsNeeded: Bool {
        state == .recording && drawWhileDictating && sketchOverlay.acceptsInput
    }

    @discardableResult
    func refreshDrawingKeyTap() -> Bool {
        guard drawingKeyTapIsNeeded else { removeDrawingKeyTap(); return true }
        guard modifierTapAvailable else { removeDrawingKeyTap(); return false }
        guard drawingEventTap == nil else { return true }
        guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap,
            options: .defaultTap, eventsOfInterest: Self.drawingKeyEventMask,
            callback: { _, type, event, info in
                guard let info else { return Unmanaged.passUnretained(event) }
                let controller = Unmanaged<KoettController>.fromOpaque(info).takeUnretainedValue()
                let disabled = type == .tapDisabledByTimeout || type == .tapDisabledByUserInput
                let eventType = NSEvent.EventType(rawValue: UInt(type.rawValue))
                let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
                let flags = UInt(event.flags.rawValue)
                let repeating = event.getIntegerValueField(.keyboardEventAutorepeat) != 0
                let consumed = MainActor.assumeIsolated {
                    guard controller.drawingKeyTapIsNeeded else { return false }
                    if disabled {
                        if let tap = controller.drawingEventTap { CGEvent.tapEnable(tap: tap, enable: true) }
                        return false
                    }
                    guard let eventType else { return false }
                    return controller.consumeDrawLockKey(type: eventType, keyCode: keyCode,
                        modifierFlags: flags, isRepeat: repeating)
                }
                return consumed ? nil : Unmanaged.passUnretained(event)
            }, userInfo: Unmanaged.passUnretained(self).toOpaque()) else { return false }
        guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) else {
            CFMachPortInvalidate(tap)
            return false
        }
        drawingEventTap = tap
        drawingEventSource = source
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        return true
    }

    private func removeDrawingKeyTap() {
        if let tap = drawingEventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            CFMachPortInvalidate(tap)
        }
        if let source = drawingEventSource { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        drawingEventTap = nil
        drawingEventSource = nil
    }
}

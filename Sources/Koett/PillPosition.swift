import AppKit

struct PillDisplay {
    let id: Int
    let visibleFrame: NSRect

    @MainActor static var mainID: Int? {
        (NSScreen.main?.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.intValue
    }

    @MainActor static var current: [Self] {
        NSScreen.screens.map {
            Self(id: ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.intValue ?? 0,
                 visibleFrame: $0.visibleFrame)
        }
    }
}

struct PillPosition {
    static let key = "pillPosition"
    let defaults: UserDefaults

    func origin(size: NSSize, displays: [PillDisplay], preferredDisplayID: Int? = nil) -> NSPoint? {
        guard let fallback = displays.first(where: { $0.id == preferredDisplayID }) ?? displays.first else { return nil }
        if let saved = defaults.dictionary(forKey: Self.key),
           let id = saved["display"] as? Int,
           let x = saved["x"] as? Double, let y = saved["y"] as? Double,
           x.isFinite, y.isFinite,
           let display = displays.first(where: { $0.id == id }) {
            let frame = display.visibleFrame
            return clamp(NSPoint(x: frame.minX + x * max(0, frame.width - size.width),
                                 y: frame.minY + y * max(0, frame.height - size.height)),
                         size: size, to: frame)
        }
        return clamp(NSPoint(x: fallback.visibleFrame.midX - size.width / 2,
                             y: fallback.visibleFrame.minY + 72), size: size, to: fallback.visibleFrame)
    }

    func save(origin: NSPoint, size: NSSize, displays: [PillDisplay]) -> NSPoint {
        let center = NSPoint(x: origin.x + size.width / 2, y: origin.y + size.height / 2)
        guard origin.x.isFinite, origin.y.isFinite,
              let display = displays.min(by: { distance(center, to: $0.visibleFrame) < distance(center, to: $1.visibleFrame) }) else { return origin }
        let frame = display.visibleFrame
        let point = clamp(origin, size: size, to: frame)
        defaults.set(["display": display.id,
                      "x": (point.x - frame.minX) / max(1, frame.width - size.width),
                      "y": (point.y - frame.minY) / max(1, frame.height - size.height)], forKey: Self.key)
        return point
    }

    private func clamp(_ point: NSPoint, size: NSSize, to frame: NSRect) -> NSPoint {
        NSPoint(x: min(max(point.x, frame.minX), max(frame.minX, frame.maxX - size.width)),
                y: min(max(point.y, frame.minY), max(frame.minY, frame.maxY - size.height)))
    }

    private func distance(_ point: NSPoint, to frame: NSRect) -> CGFloat {
        let dx = max(frame.minX - point.x, 0, point.x - frame.maxX)
        let dy = max(frame.minY - point.y, 0, point.y - frame.maxY)
        return dx * dx + dy * dy
    }
}

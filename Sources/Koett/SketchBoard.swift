import CoreGraphics
import Foundation

/// A change for the pill: an upserted sketch, or a freed number.
enum SketchUpdate: Equatable, Sendable {
    case upsert(SketchEvent)
    case removed(index: UInt32)
}

/// Sketch and pin state for one dictation, without windows or capture: holds,
/// strokes, clicks, numbering and capture phases. Mirrors the Linux `Session`
/// rules: one draw-key hold is one sketch per screen, numbers follow creation
/// order, and a failed capture renumbers the rest. A click while the key is held
/// drops a pin instead (rules in `docs/screen-context-proposal.md`).
struct SketchBoard {
    static let maxSketches = 256
    /// Points the pointer must travel before a hold starts a sketch, so Shift
    /// for a capital letter never leaves a dot.
    static let startDistance: CGFloat = 3
    static let fadeAfter: TimeInterval = 1.5
    static let fadeFor: TimeInterval = 0.4
    /// A press that travels this far becomes ink instead of a pin.
    static let pinSlop: CGFloat = 12
    /// A press held longer than this drops no pin.
    static let pinPressLimit: TimeInterval = 1
    /// Ink shorter than this before a click in the same hold is aiming jitter
    /// and is dropped for the pin; longer ink makes the click part of the drawing.
    static let pinJitterInk: CGFloat = 12
    static let pinPulse: TimeInterval = 0.45

    enum Phase: Equatable { case drawing, capturing, captured }

    struct Group: Equatable {
        let id: UInt64
        let screen: Int
        /// Points in screen-local, top-left logical coordinates.
        var strokes: [[CGPoint]]
        let start: TimeInterval
        var end: TimeInterval
        /// Start on the captured-audio clock, used to link speech.
        let audioStart: Float
        var lifted: TimeInterval?
        var closed = false
        var phase = Phase.drawing
        /// Set for a pin: the click point, in the same coordinates as strokes.
        var pin: CGPoint?

        func alpha(at now: TimeInterval) -> Double {
            guard closed, let lifted else { return 1 }
            if pin != nil { return 1 - min(max((now - lifted) / SketchBoard.pinPulse, 0), 1) }
            let fading = now - (lifted + SketchBoard.fadeAfter)
            return 1 - min(max(fading / SketchBoard.fadeFor, 0), 1)
        }
    }

    /// Wall-clock origin shared with the pill (`systemUptime`).
    let startedAt: TimeInterval
    private(set) var groups: [Group] = []
    private(set) var held = false
    private(set) var stroking: UInt64?
    private(set) var finishing = false
    private var nextID: UInt64 = 0
    private var holdOrigin: (screen: Int, point: CGPoint, time: TimeInterval, audio: Float)?
    /// Ink travelled in the current hold.
    private var holdInk: CGFloat = 0
    private var pressing: Press?

    private struct Press {
        let screen: Int
        let point: CGPoint
        let time: TimeInterval
        let audio: Float
        /// False for the second click of a double-click.
        let pins: Bool
    }

    init(startedAt: TimeInterval) {
        self.startedAt = startedAt
    }

    mutating func press() {
        guard !finishing else { return }
        held = true
        holdOrigin = nil
        holdInk = 0
        pressing = nil
    }

    /// Ends the hold: its sketches close and become ready for capture. A click
    /// still pressed when the key lifts counts as released now.
    mutating func release(now: TimeInterval) -> [SketchUpdate] {
        var updates = finishPress(now: now)
        held = false
        holdOrigin = nil
        updates += lift(now: now)
        for index in groups.indices { groups[index].closed = true }
        return updates
    }

    /// Recording stopped: no new ink; every open sketch closes.
    mutating func finish(now: TimeInterval) -> [SketchUpdate] {
        let updates = release(now: now)
        finishing = true
        return updates
    }

    /// A pointer sample while the key is held. `audio` is the captured-audio clock.
    mutating func point(screen: Int, _ point: CGPoint, now: TimeInterval, audio: Float) -> [SketchUpdate] {
        guard held, !finishing else { return [] }
        if let press = pressing {
            guard press.screen == screen,
                  hypot(point.x - press.point.x, point.y - press.point.y) >= Self.pinSlop else { return [] }
            pressing = nil
            holdOrigin = (press.screen, press.point, press.time, press.audio)
        }
        if let current = stroking.flatMap(position(of:)), groups[current].screen == screen {
            guard let last = groups[current].strokes.last?.last, last != point else { return [] }
            holdInk += hypot(point.x - last.x, point.y - last.y)
            groups[current].end = now
            groups[current].strokes[groups[current].strokes.count - 1].append(point)
            return events()
        }
        guard let origin = holdOrigin, origin.screen == screen else {
            holdOrigin = (screen, point, now, audio)
            return []
        }
        let travel = hypot(point.x - origin.point.x, point.y - origin.point.y)
        guard travel >= Self.startDistance else { return [] }
        holdInk += travel
        var updates = lift(now: now)
        let open = groups.firstIndex { $0.screen == screen && $0.phase == .drawing && !$0.closed }
        let index: Int
        if let open {
            index = open
        } else {
            guard groups.count < Self.maxSketches else { return updates }
            nextID += 1
            groups.append(Group(
                id: nextID, screen: screen, strokes: [], start: origin.time, end: now, audioStart: origin.audio
            ))
            index = groups.count - 1
        }
        groups[index].strokes.append([origin.point, point])
        groups[index].lifted = nil
        groups[index].end = now
        stroking = groups[index].id
        holdOrigin = (screen, point, now, audio)
        updates = events()
        return updates
    }

    /// A mouse button went down while the key is held. Ink so far in this hold
    /// shorter than `pinJitterInk` is dropped; longer ink keeps drawing and
    /// the click drops no pin.
    mutating func mouseDown(
        screen: Int, _ point: CGPoint, now: TimeInterval, audio: Float, clickCount: Int
    ) -> [SketchUpdate] {
        guard held, !finishing else { return [] }
        guard holdInk < Self.pinJitterInk else {
            pressing = nil
            return []
        }
        var updates: [SketchUpdate] = []
        let before = groups.count
        groups.removeAll { !$0.closed && $0.pin == nil }
        if groups.count < before {
            stroking = nil
            let freed = (groups.count + 1)...before
            updates = events() + freed.map { .removed(index: UInt32($0)) }
        }
        holdInk = 0
        holdOrigin = nil
        pressing = Press(screen: screen, point: point, time: now, audio: audio, pins: clickCount <= 1)
        return updates
    }

    /// The button came up: a short, still press becomes a pin.
    mutating func mouseUp(now: TimeInterval) -> [SketchUpdate] {
        finishPress(now: now)
    }

    private mutating func finishPress(now: TimeInterval) -> [SketchUpdate] {
        guard let press = pressing else { return [] }
        pressing = nil
        holdOrigin = nil
        guard press.pins, held, !finishing, now - press.time <= Self.pinPressLimit,
              groups.count < Self.maxSketches else { return [] }
        nextID += 1
        var group = Group(
            id: nextID, screen: press.screen, strokes: [], start: press.time, end: press.time, audioStart: press.audio
        )
        group.pin = press.point
        group.closed = true
        group.lifted = now
        groups.append(group)
        return events()
    }

    /// Closed sketches that should be captured now; they move to `.capturing`.
    mutating func takeReadyForCapture() -> [Group] {
        var ready: [Group] = []
        for index in groups.indices where groups[index].phase == .drawing
            && groups[index].closed && stroking != groups[index].id {
            groups[index].phase = .capturing
            ready.append(groups[index])
        }
        return ready
    }

    /// Records a capture; nil when the sketch no longer exists.
    mutating func captured(_ id: UInt64) -> Group? {
        guard let index = position(of: id) else { return nil }
        groups[index].phase = .captured
        return groups[index]
    }

    /// A failed capture removes its sketch; later numbers shift down and the
    /// highest number is freed.
    mutating func failed(_ id: UInt64) -> [SketchUpdate] {
        guard let index = position(of: id) else { return [] }
        let freed = UInt32(groups.count)
        groups.remove(at: index)
        return events() + [.removed(index: freed)]
    }

    /// Delivery stopped waiting: sketches without a capture are removed so the
    /// pill numbers the same sketches the sheet shows.
    mutating func abandonPending() -> [SketchUpdate] {
        let before = groups.count
        groups.removeAll { $0.phase != .captured }
        guard groups.count < before else { return [] }
        let freed = (groups.count + 1)...before
        return events() + freed.map { .removed(index: UInt32($0)) }
    }

    var isCapturing: Bool { groups.contains { $0.phase != .captured } }

    /// True while released ink is still visible, including the pause before it fades.
    func hasFadingInk(at now: TimeInterval) -> Bool {
        groups.contains { $0.closed && $0.lifted != nil && $0.alpha(at: now) > 0 }
    }

    /// Every sketch numbered by creation order, on the pill's wall clock.
    func events() -> [SketchUpdate] {
        groups.sorted { $0.id < $1.id }.enumerated().map { offset, group in
            .upsert(SketchEvent(
                index: UInt32(offset + 1),
                startMs: Self.milliseconds(group.start - startedAt),
                endMs: Self.milliseconds(group.end - startedAt),
                live: stroking == group.id
            ))
        }
    }

    private mutating func lift(now: TimeInterval) -> [SketchUpdate] {
        guard let id = stroking, let index = position(of: id) else {
            stroking = nil
            return []
        }
        stroking = nil
        groups[index].lifted = now
        return events()
    }

    private func position(of id: UInt64) -> Int? {
        groups.firstIndex { $0.id == id }
    }

    private static func milliseconds(_ seconds: TimeInterval) -> UInt64 {
        guard seconds.isFinite, seconds > 0 else { return 0 }
        return UInt64(min(seconds * 1_000, Double(UInt64.max / 2)))
    }
}

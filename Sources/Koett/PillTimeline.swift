import Foundation

/// One sketch drawn during a dictation, as shown on the pill's timeline.
/// Mirrors `koett_engine::sketch::SketchEvent` on Linux.
struct SketchEvent: Equatable, Sendable {
    /// 1-based number; the same number that is pasted with the sketch.
    var index: UInt32
    /// Milliseconds since the dictation started when drawing began.
    var startMs: UInt64
    /// Latest drawing time. While `live`, the emitter keeps raising this.
    var endMs: UInt64
    /// True while the sketch is still being drawn.
    var live: Bool

    var isValid: Bool { index > 0 && endMs >= startMs }
}

/// Live level strip with a fixed width: the newest audio enters at the right
/// edge and older columns slide left and out. The width never changes and no
/// column is ever re-binned, so the strip looks the same at 5 s and at 20 min.
/// Every column comes from measured microphone input.
struct PillTimeline {
    static let columnCount = 64
    static let columnMs: UInt64 = 150
    static let silentLevel = 0.08
    /// Distinct sketches kept per dictation. Updates to a kept index always apply.
    static let sketchLimit = 256

    /// One drawn column. `x` is in column units from the strip's left edge
    /// (0..<columnCount, fractional while sliding); `level` is nil before the
    /// dictation started.
    struct Bar: Equatable {
        let x: Double
        let level: Double?
    }

    private(set) var levels: [Double] = []
    private(set) var firstColumn = 0
    private(set) var latestMs: UInt64 = 0
    private(set) var sketches: [SketchEvent] = []
    private var smoothed = 0.0
    private var started = false

    var isEmpty: Bool { !started }
    var isDrawing: Bool { sketches.contains(where: \.live) }

    mutating func reset() {
        self = PillTimeline()
    }

    /// Adds one meter sample. `level` is 0...1 from the dB mapping.
    mutating func push(elapsedMs: UInt64, level: Double) {
        let level = level.isFinite ? min(1, max(0, level)) : 0
        smoothed = smoothed * 0.55 + level * 0.45
        started = true
        latestMs = max(latestMs, elapsedMs)
        let column = Int(elapsedMs / Self.columnMs)
        let lastColumn = firstColumn + levels.count - 1
        if column > lastColumn {
            levels.append(contentsOf: repeatElement(0, count: column - lastColumn))
        }
        let position = column - firstColumn
        if levels.indices.contains(position) {
            levels[position] = max(levels[position], smoothed)
        }
        let overflow = levels.count - (Self.columnCount + 2)
        if overflow > 0 {
            levels.removeFirst(overflow)
            firstColumn += overflow
        }
    }

    /// The strip's right edge in column units since the dictation started.
    private var head: Double { Double(latestMs) / Double(Self.columnMs) }

    private func unit(ms: UInt64) -> Double {
        Double(ms) / Double(Self.columnMs) - head + Double(Self.columnCount - 1)
    }

    /// Columns to draw, oldest first, at their sliding positions.
    func bars() -> [Bar] {
        let newest = Int(latestMs / Self.columnMs)
        return (newest - Self.columnCount...newest).compactMap { column in
            let x = Double(column) - head + Double(Self.columnCount - 1)
            guard x > -1 else { return nil }
            guard column >= 0 else { return Bar(x: x, level: nil) }
            let position = column - firstColumn
            return Bar(x: x, level: levels.indices.contains(position) ? levels[position] : 0)
        }
    }

    /// Inserts or replaces the sketch with the same index. Returns false when a
    /// new index arrives after `sketchLimit` sketches.
    @discardableResult
    mutating func apply(_ event: SketchEvent) -> Bool {
        if let position = sketches.firstIndex(where: { $0.index == event.index }) {
            sketches[position] = event
            return true
        }
        guard sketches.count < Self.sketchLimit else { return false }
        let position = sketches.firstIndex(where: { $0.index > event.index }) ?? sketches.endIndex
        sketches.insert(event, at: position)
        return true
    }

    /// Removes the band with this number (undone or failed sketch).
    @discardableResult
    mutating func remove(index: UInt32) -> Bool {
        guard let position = sketches.firstIndex(where: { $0.index == index }) else { return false }
        sketches.remove(at: position)
        return true
    }

    /// A sketch's band in column units, clipped to the strip; nil once it has
    /// slid out on the left.
    func band(of sketch: SketchEvent) -> ClosedRange<Double>? {
        let start = max(0, unit(ms: sketch.startMs))
        let end = min(Double(Self.columnCount), unit(ms: sketch.endMs) + 1)
        guard end > 0, start < Double(Self.columnCount), end >= start else { return nil }
        return start...end
    }
}

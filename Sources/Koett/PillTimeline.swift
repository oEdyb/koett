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

/// Whole-dictation level history. Every column comes from measured
/// microphone input; nothing moves without input.
struct PillTimeline {
    static let columnCount = 64
    static let baseMs: UInt64 = 250
    static let silentLevel = 0.08
    /// Distinct sketches kept per dictation. Updates to a kept index always apply.
    static let sketchLimit = 256

    private(set) var levels: [Double] = []
    private(set) var msPerColumn = baseMs
    private(set) var sketches: [SketchEvent] = []
    private var smoothed = 0.0

    var isEmpty: Bool { levels.isEmpty }
    var isDrawing: Bool { sketches.contains(where: \.live) }

    mutating func reset() {
        self = PillTimeline()
    }

    /// Adds one meter sample. `level` is 0...1 from the dB mapping.
    mutating func push(elapsedMs: UInt64, level: Double) {
        let level = level.isFinite ? min(1, max(0, level)) : 0
        smoothed = smoothed * 0.55 + level * 0.45
        var index = elapsedMs / msPerColumn
        while index >= UInt64(Self.columnCount) {
            compress()
            index = elapsedMs / msPerColumn
        }
        let column = Int(index)
        if levels.count <= column {
            levels.append(contentsOf: repeatElement(0, count: column + 1 - levels.count))
        }
        levels[column] = max(levels[column], smoothed)
    }

    private mutating func compress() {
        levels = stride(from: 0, to: levels.count, by: 2).map { start in
            levels[start..<min(start + 2, levels.count)].max() ?? 0
        }
        msPerColumn = msPerColumn.multipliedReportingOverflow(by: 2).overflow
            ? UInt64.max
            : msPerColumn * 2
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

    /// Inclusive column range covered by a sketch, compressed with the strip.
    func span(of sketch: SketchEvent) -> ClosedRange<Int> {
        let last = UInt64(Self.columnCount - 1)
        let start = min(sketch.startMs / msPerColumn, last)
        let end = min(max(sketch.endMs / msPerColumn, start), last)
        return Int(start)...Int(end)
    }

    /// Column reached by decoding, for a 0...1 fraction of the recording.
    func decodeHead(fraction: Double) -> Int {
        let fraction = fraction.isFinite ? min(1, max(0, fraction)) : 0
        return Int((Double(levels.count) * fraction).rounded())
    }
}

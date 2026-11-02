import CoreGraphics
import Foundation

/// One captured sketch: creation order, start on the captured-audio clock,
/// and its cropped screen image with the ink.
struct SketchFrame {
    let order: UInt64
    let start: Float
    let image: CGImage
}

/// Captured sketches scoped to one dictation, as in Linux `sketch/frames.rs`.
/// Every recording start begins a new scope, so frames from a cancelled,
/// failed or earlier dictation, and captures finishing late, never reach a
/// later paste.
@MainActor
final class SketchFrames {
    private var session: UInt64?
    private var open = false
    private var frames: [SketchFrame] = []

    func begin(_ session: UInt64) {
        self.session = session
        open = false
        frames.removeAll()
    }

    /// Drops a cancelled or failed dictation's frames and rejects its late captures.
    func discard(_ session: UInt64) {
        guard self.session == session else { return }
        self.session = nil
        open = false
        frames.removeAll()
    }

    /// Marks the dictation's overlay as still capturing.
    func setOpen(_ session: UInt64, _ open: Bool) {
        guard self.session == session else { return }
        self.open = open
    }

    @discardableResult
    func push(_ session: UInt64, _ frame: SketchFrame) -> Bool {
        guard self.session == session else { return false }
        frames.append(frame)
        return true
    }

    /// Waits (bounded) for the overlay to finish capturing. Returns false when
    /// captures are still open at the deadline.
    func wait(_ session: UInt64, timeout: Duration) async -> Bool {
        let deadline = ContinuousClock.now + timeout
        while self.session == session, open, ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(20))
        }
        return !(self.session == session && open)
    }

    /// Takes the dictation's frames in creation order. Empty when nothing was drawn.
    func take(_ session: UInt64) -> [SketchFrame] {
        guard self.session == session else { return [] }
        let taken = frames.sorted { $0.order < $1.order }
        frames.removeAll()
        return taken
    }
}

import AppKit
@testable import Koett
import XCTest

/// Renders every pill state in dark and light to PNG files.
/// Runs only when KOETT_PILL_SNAPSHOT_DIR is set.
@MainActor
final class PillSnapshotTests: XCTestCase {
    func testRenderEveryStateInDarkAndLight() throws {
        guard let directory = ProcessInfo.processInfo.environment["KOETT_PILL_SNAPSHOT_DIR"] else {
            throw XCTSkip("Set KOETT_PILL_SNAPSHOT_DIR to render pill snapshots.")
        }
        let folder = URL(fileURLWithPath: directory, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        func speech(seconds total: Double) -> PillTimeline {
            var timeline = PillTimeline()
            for step in 0...Int(total * 30) {
                let seconds = Double(step) / 30
                let word = abs(sin(seconds * 2.7)) * (seconds.truncatingRemainder(dividingBy: 3.4) < 2.6 ? 1 : 0)
                timeline.push(elapsedMs: UInt64(seconds * 1_000), level: word == 0 ? 0.01 : 0.25 + 0.6 * word * abs(sin(seconds * 9)))
            }
            return timeline
        }
        let short = speech(seconds: 11)
        var sketched = short
        sketched.apply(SketchEvent(index: 1, startMs: 1_500, endMs: 3_250, live: false))
        sketched.apply(SketchEvent(index: 2, startMs: 7_500, endMs: 10_750, live: true))
        let done = PillContent.done(preview: "Please buy milk and bread, and call the dentist on Friday about the appointment.")

        let morphFrames: [CGFloat] = [0, 0.25, 0.5, 0.75, 1]
        let plain: [(String, PillContent, PillTimeline, Bool, TimeInterval)] = [
            ("recording-0m05", .recording(paused: false), speech(seconds: 5), false, 5),
            ("recording-1m00", .recording(paused: false), speech(seconds: 60), false, 60),
            ("recording-5m00", .recording(paused: false), speech(seconds: 300), false, 300),
            ("recording-20m00", .recording(paused: false), speech(seconds: 1_200), false, 1_200),
            ("recording-hover", .recording(paused: false), short, true, 11),
            ("drawing", .recording(paused: false), sketched, false, 11),
            ("paused-hover", .recording(paused: true), short, true, 11),
            ("transcribing", .transcribing(fraction: nil), short, false, 11),
            ("transcribing-progress", .transcribing(fraction: 0.55), short, false, 11),
            ("done", done, PillTimeline(), false, 11),
            ("not-pasted", .notPasted, PillTimeline(), false, 11),
            ("media", .media(preview: "Today we are looking at what local models can do"), PillTimeline(), false, 11),
            ("status", .status("Transcribing locally…"), PillTimeline(), false, 11),
            ("error", .error("Dictation failed · Audio saved"), PillTimeline(), false, 11),
        ]
        let morphStates: [(String, PillContent, PillTimeline, Bool, TimeInterval, CGFloat?)] = morphFrames.enumerated().map { index, morph in
            ("morph-\(index)", done, short, false, 11, morph)
        }
        let states = morphStates + plain.map { ($0.0, $0.1, $0.2, $0.3, $0.4, CGFloat?.none) }
        for (appearanceName, appearance) in [("dark", NSAppearance.Name.darkAqua), ("light", .aqua)] {
            for (name, content, timeline, hovered, elapsed, morph) in states {
                let view = PillView(frame: NSRect(x: 0, y: 0, width: PillView.width, height: PillView.rowHeight))
                view.appearance = NSAppearance(named: appearance)
                view.hasRecordingControls = true
                view.canPause = true
                view.modelName = "Parakeet v2"
                view.timeline = timeline
                view.elapsed = elapsed
                view.spin = 1
                view.content = content
                if let morph {
                    view.morphFrom = .transcribing(fraction: nil)
                    view.morph = morph
                }
                view.setHovered(hovered)
                view.frame.size.height = view.preferredHeight
                let canvas = NSView(frame: view.frame.insetBy(dx: -12, dy: -12))
                canvas.wantsLayer = true
                canvas.layer?.backgroundColor = (appearanceName == "dark"
                    ? NSColor(white: 0.18, alpha: 1)
                    : NSColor(white: 0.86, alpha: 1)).cgColor
                view.frame.origin = NSPoint(x: 12, y: 12)
                canvas.addSubview(view)
                let scale: CGFloat = 2
                let rep = try XCTUnwrap(NSBitmapImageRep(
                    bitmapDataPlanes: nil,
                    pixelsWide: Int(canvas.bounds.width * scale),
                    pixelsHigh: Int(canvas.bounds.height * scale),
                    bitsPerSample: 8,
                    samplesPerPixel: 4,
                    hasAlpha: true,
                    isPlanar: false,
                    colorSpaceName: .deviceRGB,
                    bytesPerRow: 0,
                    bitsPerPixel: 0
                ))
                rep.size = canvas.bounds.size
                let context = try XCTUnwrap(NSGraphicsContext(bitmapImageRep: rep))
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = context
                (appearanceName == "dark" ? NSColor(white: 0.18, alpha: 1) : NSColor(white: 0.86, alpha: 1)).setFill()
                canvas.bounds.fill()
                NSGraphicsContext.restoreGraphicsState()
                NSAppearance(named: appearance)?.performAsCurrentDrawingAppearance {
                    canvas.cacheDisplay(in: canvas.bounds, to: rep)
                }
                let data = try XCTUnwrap(rep.representation(using: .png, properties: [:]))
                try data.write(to: folder.appendingPathComponent("\(name)-\(appearanceName).png"))
            }
        }
    }
}

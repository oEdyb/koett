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
        var speech = PillTimeline()
        for step in 0..<(11 * 30) {
            let seconds = Double(step) / 30
            let word = abs(sin(seconds * 2.7)) * (seconds.truncatingRemainder(dividingBy: 3.4) < 2.6 ? 1 : 0)
            speech.push(elapsedMs: UInt64(seconds * 1_000), level: word == 0 ? 0.01 : 0.25 + 0.6 * word * abs(sin(seconds * 9)))
        }
        var sketched = speech
        sketched.apply(SketchEvent(index: 1, startMs: 1_500, endMs: 3_250, live: false))
        sketched.apply(SketchEvent(index: 2, startMs: 7_500, endMs: 10_750, live: true))

        let states: [(String, PillContent, PillTimeline, Bool)] = [
            ("recording", .recording(paused: false), speech, false),
            ("recording-hover", .recording(paused: false), speech, true),
            ("drawing", .recording(paused: false), sketched, false),
            ("paused-hover", .recording(paused: true), speech, true),
            ("transcribing", .transcribing(fraction: nil), speech, false),
            ("transcribing-progress", .transcribing(fraction: 0.55), speech, false),
            ("done", .done(duration: 11), speech, false),
            ("not-pasted", .notPasted, PillTimeline(), false),
            ("media", .media(preview: "Today we are looking at what local models can do"), PillTimeline(), false),
            ("status", .status("Transcribing locally…"), PillTimeline(), false),
        ]
        for (appearanceName, appearance) in [("dark", NSAppearance.Name.darkAqua), ("light", .aqua)] {
            for (name, content, timeline, hovered) in states {
                let view = PillView(frame: NSRect(x: 0, y: 0, width: PillView.width, height: PillView.rowHeight))
                view.appearance = NSAppearance(named: appearance)
                view.hasRecordingControls = true
                view.canPause = true
                view.modelName = "Parakeet v2"
                view.timeline = timeline
                view.elapsed = 11
                view.spin = 1
                view.content = content
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

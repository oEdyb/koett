import AppKit
import AVFoundation
@testable import Koett
import XCTest

@MainActor
final class SketchDrawTests: XCTestCase {
    // MARK: Session scope

    private func frame(_ order: UInt64, _ start: Float = 1) -> SketchFrame {
        SketchFrame(order: order, start: start, image: solid(4, 4, 0.5))
    }

    func testCancelledImagesNeverReachALaterDictation() async {
        let frames = SketchFrames()
        frames.begin(1)
        XCTAssertTrue(frames.push(1, frame(1)))
        frames.discard(1)
        XCTAssertFalse(frames.push(1, frame(2)), "late capture of a cancelled dictation")
        frames.begin(2)
        let taken = await frames.take(2, timeout: .zero)
        XCTAssertTrue(taken.isEmpty)
    }

    func testFailedDictationFramesAreDroppedByTheNextStart() async {
        let frames = SketchFrames()
        frames.begin(1)
        frames.push(1, frame(1))
        frames.begin(2)
        XCTAssertFalse(frames.push(1, frame(2)))
        let stale = await frames.take(1, timeout: .zero)
        XCTAssertTrue(stale.isEmpty)
        let current = await frames.take(2, timeout: .zero)
        XCTAssertTrue(current.isEmpty)
    }

    func testTakeWaitsForOpenCapturesAndKeepsCreationOrder() async {
        let frames = SketchFrames()
        frames.begin(7)
        frames.setOpen(7, true)
        frames.push(7, frame(2, 5))
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(80))
            frames.push(7, frame(1, 5))
            frames.setOpen(7, false)
        }
        let started = ContinuousClock.now
        let taken = await frames.take(7, timeout: .seconds(5))
        XCTAssertEqual(taken.map(\.order), [1, 2])
        XCTAssertLessThan(ContinuousClock.now - started, .seconds(2))
        let again = await frames.take(7, timeout: .zero)
        XCTAssertTrue(again.isEmpty)
    }

    func testTakeIsBoundedWhenACaptureNeverFinishes() async {
        let frames = SketchFrames()
        frames.begin(3)
        frames.setOpen(3, true)
        let started = ContinuousClock.now
        _ = await frames.take(3, timeout: .milliseconds(100))
        XCTAssertLessThan(ContinuousClock.now - started, .seconds(1))
    }

    // MARK: Holds and numbering

    private func draw(_ board: inout SketchBoard, screen: Int, from x: CGFloat, at time: TimeInterval, audio: Float) {
        board.press()
        _ = board.point(screen: screen, CGPoint(x: x, y: 10), now: time, audio: audio)
        _ = board.point(screen: screen, CGPoint(x: x + 20, y: 10), now: time + 0.1, audio: audio)
        _ = board.release(now: time + 0.2)
    }

    func testEachHoldIsASeparateSketchOnAnyScreen() {
        var board = SketchBoard(startedAt: 100)
        draw(&board, screen: 0, from: 0, at: 101, audio: 1)
        draw(&board, screen: 0, from: 50, at: 102, audio: 2)
        draw(&board, screen: 1, from: 50, at: 103, audio: 3)
        XCTAssertEqual(board.groups.map(\.screen), [0, 0, 1])
        XCTAssertEqual(board.groups.map(\.audioStart), [1, 2, 3])
        XCTAssertTrue(board.groups.allSatisfy(\.closed))
        XCTAssertEqual(board.takeReadyForCapture().count, 3)
    }

    func testShiftForACapitalLetterLeavesNoSketch() {
        var board = SketchBoard(startedAt: 0)
        board.press()
        _ = board.point(screen: 0, CGPoint(x: 10, y: 10), now: 1, audio: 1)
        _ = board.point(screen: 0, CGPoint(x: 11, y: 11), now: 1.1, audio: 1)
        _ = board.release(now: 1.2)
        XCTAssertTrue(board.groups.isEmpty)
    }

    func testMovingToAnotherScreenInOneHoldStartsASketchThere() {
        var board = SketchBoard(startedAt: 0)
        board.press()
        _ = board.point(screen: 0, CGPoint(x: 0, y: 0), now: 1, audio: 1)
        _ = board.point(screen: 0, CGPoint(x: 10, y: 0), now: 1.1, audio: 1)
        _ = board.point(screen: 1, CGPoint(x: 0, y: 0), now: 1.2, audio: 1.2)
        _ = board.point(screen: 1, CGPoint(x: 10, y: 0), now: 1.3, audio: 1.3)
        _ = board.point(screen: 0, CGPoint(x: 50, y: 0), now: 1.4, audio: 1.4)
        _ = board.point(screen: 0, CGPoint(x: 60, y: 0), now: 1.5, audio: 1.5)
        XCTAssertEqual(board.groups.count, 2, "returning in the same hold continues the first sketch")
        XCTAssertEqual(board.groups[0].strokes.count, 2)
        XCTAssertTrue(board.takeReadyForCapture().isEmpty, "nothing is captured until the key is released")
        _ = board.release(now: 2)
        XCTAssertEqual(board.takeReadyForCapture().count, 2)
    }

    func testAtMost256SketchesPerDictation() {
        var board = SketchBoard(startedAt: 0)
        for index in 0..<300 {
            draw(&board, screen: 0, from: CGFloat(index), at: Double(index), audio: Float(index))
        }
        XCTAssertEqual(board.groups.count, 256)
        XCTAssertEqual(SketchBoard.maxSketches, PillTimeline.sketchLimit)
    }

    func testPillNumbersMatchPastedNumbersWhenCapturesFinishOutOfOrder() async {
        var board = SketchBoard(startedAt: 0)
        draw(&board, screen: 0, from: 0, at: 1, audio: 1)
        draw(&board, screen: 1, from: 0, at: 2, audio: 1)
        draw(&board, screen: 0, from: 0, at: 3, audio: 1)
        let frames = SketchFrames()
        frames.begin(1)
        for group in board.takeReadyForCapture().reversed() {
            let captured = board.captured(group.id)!
            frames.push(1, SketchFrame(order: captured.id, start: captured.audioStart, image: solid(2, 2, 0)))
        }
        let pasted = await frames.take(1, timeout: .zero).map(\.order)
        let pill = board.events().compactMap { update -> UInt64? in
            guard case .upsert(let event) = update else { return nil }
            return board.groups.sorted { $0.id < $1.id }[Int(event.index) - 1].id
        }
        XCTAssertEqual(pasted, pill)
        XCTAssertEqual(pasted, [1, 2, 3])
    }

    func testCaptureFailingAfterStopRenumbersThePill() {
        var board = SketchBoard(startedAt: 0)
        draw(&board, screen: 0, from: 0, at: 1, audio: 1)
        draw(&board, screen: 0, from: 0, at: 5, audio: 5)
        _ = board.finish(now: 6)
        let ready = board.takeReadyForCapture()
        var timeline = PillTimeline()
        for case .upsert(let event) in board.events() { timeline.apply(event) }
        XCTAssertEqual(timeline.sketches.map(\.startMs), [1_000, 5_000])
        for update in board.failed(ready[0].id) {
            switch update {
            case .upsert(let event): timeline.apply(event)
            case .removed(let index): timeline.remove(index: index)
            }
        }
        XCTAssertEqual(timeline.sketches.map(\.index), [1])
        XCTAssertEqual(timeline.sketches.map(\.startMs), [5_000], "#1 is now the second sketch")
        XCTAssertTrue(board.isCapturing)
        _ = board.captured(ready[1].id)
        XCTAssertFalse(board.isCapturing)
    }

    func testPillAcceptsLateChangesOnlyForItsOwnDictation() throws {
        let overlay = RecordingOverlayController()
        let recorder = try makeRecorder()
        overlay.start(recorder: recorder, sketchSession: 4)
        let event = SketchEvent(index: 1, startMs: 0, endMs: 10, live: false)
        recorder.stop()
        overlay.stop()
        XCTAssertTrue(overlay.applySketch(.upsert(event), session: 4), "captures finish after Stop")
        XCTAssertFalse(overlay.applySketch(.upsert(event), session: 3))
        XCTAssertTrue(overlay.applySketch(.removed(index: 1), session: 4))
        XCTAssertTrue(overlay.pillView.timeline.sketches.isEmpty)
        overlay.showStatus("Ready")
        XCTAssertFalse(overlay.applySketch(.upsert(event), session: 4))
    }

    // MARK: Clocks

    func testPausedAudioDoesNotAdvanceTheLinkingClock() throws {
        let capture = SketchFakeCapture()
        let recorder = try makeRecorder(capture)
        capture.feed(seconds: 2)
        recorder.isPaused = true
        capture.feed(seconds: 5)
        recorder.isPaused = false
        capture.feed(seconds: 1)
        XCTAssertEqual(recorder.currentTime, 3, accuracy: 0.01)
        let words = [
            TimedWord(text: "fix", start: 0.5, end: 0.8),
            TimedWord(text: "this.", start: 0.9, end: 1.2),
            TimedWord(text: "and", start: 2.3, end: 2.5),
            TimedWord(text: "that.", start: 2.6, end: 2.9),
        ]
        XCTAssertEqual(
            SketchLinking.keyframeSummary(
                finalText: "Fix this. And that.", words: words, anchors: [0.6, Float(recorder.currentTime) - 0.8]
            ),
            "- #1 (0:00) “Fix this.”\n- #2 (0:02) “And that.”"
        )
        recorder.stop()
    }

    func testBoardKeepsWallTimeForThePillAndAudioTimeForLinking() {
        var board = SketchBoard(startedAt: 10)
        draw(&board, screen: 0, from: 0, at: 18, audio: 3)
        guard case .upsert(let event) = board.events()[0] else { return XCTFail("no event") }
        XCTAssertEqual(event.startMs, 8_000)
        XCTAssertEqual(board.groups[0].audioStart, 3)
    }

    // MARK: Draw key

    func testDrawKeyOnlyDrawsAlone() {
        XCTAssertTrue(DrawKey.shift.draws([.shift]))
        XCTAssertTrue(DrawKey.shift.draws([.shift, .capsLock]))
        XCTAssertFalse(DrawKey.shift.draws([.shift, .command]))
        XCTAssertFalse(DrawKey.shift.draws([.shift, .control]))
        XCTAssertFalse(DrawKey.shift.draws([]))
    }

    func testDrawKeyNeverSharesTheDictationModifier() {
        XCTAssertTrue(DrawKey.option.conflicts(with: .defaultDictation))
        XCTAssertEqual(DrawKey.effective(.option, dictation: .defaultDictation), .shift)
        XCTAssertEqual(DrawKey.effective(.control, dictation: .defaultDictation), .control)
    }

    // MARK: Images

    func testInkCropMatchesLinux() {
        let screen = CGSize(width: 1000, height: 800)
        XCTAssertEqual(SketchSheet.inkCrop(screen: screen, strokes: [[.zero]]), CGRect(x: 0, y: 0, width: 450, height: 360))
        XCTAssertEqual(
            SketchSheet.inkCrop(screen: screen, strokes: [[CGPoint(x: 100, y: 100), CGPoint(x: 900, y: 700)]]),
            CGRect(x: 60, y: 60, width: 880, height: 680)
        )
        XCTAssertEqual(SketchSheet.inkCrop(screen: screen, strokes: []), CGRect(x: 0, y: 0, width: 1000, height: 800))
    }

    func testKeyframeMapsFractionalScaleAndDrawsOnlyItsInk() throws {
        let still = solid(1250, 1000, 1)
        let frame = try XCTUnwrap(SketchSheet.keyframe(
            screen: still,
            logical: CGSize(width: 1000, height: 800),
            strokes: [[CGPoint(x: 500, y: 400)]]
        ))
        XCTAssertEqual(frame.width, 564)
        XCTAssertEqual(frame.height, 450)
        let center = try pixel(frame, x: frame.width / 2, y: frame.height / 2)
        XCTAssertGreaterThan(center.0, 200)
        XCTAssertLessThan(center.1, 120, "red ink at the stroke")
        let corner = try pixel(frame, x: 2, y: 2)
        XCTAssertEqual(corner.1, 255, "no ink away from the stroke")
    }

    func testContactSheetRespectsTheCaps() throws {
        for count in [1, 2, 3, 5, 12] {
            let aspects = Array(repeating: 16.0 / 9.0, count: count)
            let (width, rows) = SketchSheet.fit(aspects)
            let height = SketchSheet.canvasHeight(rows)
            XCTAssertLessThanOrEqual(width, SketchSheet.longEdge)
            XCTAssertLessThanOrEqual(height, SketchSheet.longEdge + 0.5)
            XCTAssertLessThanOrEqual(width * height, SketchSheet.maxPixels * 1.01)
            XCTAssertEqual(rows.reduce(0) { $0 + $1.indices.count }, count)
        }
        let sheet = try XCTUnwrap(SketchSheet.contactSheet([solid(320, 180, 0.8), solid(180, 320, 0.2), solid(320, 180, 0.8)]))
        XCTAssertLessThanOrEqual(max(sheet.width, sheet.height), 1600)
        XCTAssertNotNil(SketchSheet.png(sheet))
    }

    /// Writes a sample sheet when KOETT_SKETCH_SNAPSHOT_DIR is set.
    func testRenderSampleContactSheet() throws {
        guard let directory = ProcessInfo.processInfo.environment["KOETT_SKETCH_SNAPSHOT_DIR"] else {
            throw XCTSkip("Set KOETT_SKETCH_SNAPSHOT_DIR to render the sample sheet.")
        }
        let screens = [(2880, 1800, 0.85), (2880, 1800, 0.35), (3840, 2160, 0.6)]
        let strokes: [[[CGPoint]]] = [
            [[CGPoint(x: 300, y: 300), CGPoint(x: 600, y: 320), CGPoint(x: 620, y: 500)]],
            [[CGPoint(x: 900, y: 200), CGPoint(x: 1100, y: 420)], [CGPoint(x: 1100, y: 200), CGPoint(x: 900, y: 420)]],
            [[CGPoint(x: 1600, y: 900)]],
        ]
        let frames = try zip(screens, strokes).map { screen, ink in
            try XCTUnwrap(SketchSheet.keyframe(
                screen: gradient(screen.0, screen.1, screen.2),
                logical: CGSize(width: screen.0 / 2, height: screen.1 / 2),
                strokes: ink
            ))
        }
        let sheet = try XCTUnwrap(SketchSheet.contactSheet(frames).flatMap(SketchSheet.png))
        let folder = URL(fileURLWithPath: directory, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try sheet.write(to: folder.appendingPathComponent("contact-sheet.png"))
    }

    // MARK: Helpers

    private func solid(_ width: Int, _ height: Int, _ shade: CGFloat) -> CGImage {
        let context = SketchSheet.canvas(width: width, height: height)!
        context.setFillColor(CGColor(gray: shade, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }

    private func gradient(_ width: Int, _ height: Int, _ hue: CGFloat) -> CGImage {
        let context = SketchSheet.canvas(width: width, height: height)!
        let colors = [NSColor(hue: hue, saturation: 0.3, brightness: 0.95, alpha: 1).cgColor,
                      NSColor(hue: hue, saturation: 0.6, brightness: 0.55, alpha: 1).cgColor]
        let gradient = CGGradient(colorsSpace: nil, colors: colors as CFArray, locations: nil)!
        context.drawLinearGradient(gradient, start: .zero, end: CGPoint(x: width, y: height), options: [])
        return context.makeImage()!
    }

    private func pixel(_ image: CGImage, x: Int, y: Int) throws -> (Int, Int, Int) {
        let context = try XCTUnwrap(SketchSheet.canvas(width: image.width, height: image.height))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        let data = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
        let offset = y * context.bytesPerRow + x * 4
        return (Int(data[offset]), Int(data[offset + 1]), Int(data[offset + 2]))
    }

    private func makeRecorder(_ capture: SketchFakeCapture = SketchFakeCapture()) throws -> MicrophoneRecorder {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("koett-sketch-\(UUID()).wav")
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        return try MicrophoneRecorder(url: url, device: MicrophoneDevice(id: 1, uid: "fake", name: "Fake"), input: capture)
    }
}

@MainActor
private final class SketchFakeCapture: MicrophoneCapturing {
    let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
    var error: Error?
    private var onAudio: (@Sendable (AVAudioPCMBuffer) -> Void)?

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        self.onAudio = onAudio
    }

    func stop() {}

    func feed(seconds: Double) {
        let frames = AVAudioFrameCount(seconds * 16_000)
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames)!
        buffer.frameLength = frames
        onAudio?(buffer)
    }
}

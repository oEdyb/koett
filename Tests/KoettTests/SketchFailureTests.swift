import AppKit
@testable import Koett
import XCTest

@MainActor
final class SketchFailureTests: XCTestCase {
    func testFailedCaptureKeepsTextAndReportsAnIncompleteDrawing() async throws {
        let controller = try makeController()
        controller.sketchFrames.begin(1)
        controller.sketchFrames.markCaptureFailed(1)
        let result = await controller.sketchedDictation(text: "Synthetic text", rawText: nil, words: [], session: 1)
        XCTAssertEqual(result.text, "Synthetic text")
        XCTAssertNil(result.sheet)
        XCTAssertTrue(result.drawingFailed)

        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        overlay.showDone(text: result.text, drawingFailed: result.drawingFailed)
        XCTAssertEqual(overlay.latestOutcome, .failure("Text pasted · Drawing failed"))
        XCTAssertEqual(overlay.pillView.preferredHeight, 48)
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        overlay.copyResult(to: board)
        XCTAssertEqual(board.string(forType: .string), result.text)
    }

    func testPartialCapturePreservesTheSurvivingSheetAndWarns() async throws {
        let controller = try makeController()
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 8, pixelsHigh: 8,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        controller.sketchFrames.begin(2)
        controller.sketchFrames.push(2, SketchFrame(order: 1, start: 0, image: try XCTUnwrap(bitmap.cgImage)))
        controller.sketchFrames.markCaptureFailed(2)
        let result = await controller.sketchedDictation(text: "Synthetic text", rawText: nil, words: [], session: 2)
        XCTAssertNotNil(result.sheet)
        XCTAssertEqual(result.frameCount, 1)
        XCTAssertTrue(result.drawingFailed)
    }

    func testTimeoutWarnsAndLateFailuresCannotAffectTheNextRecording() async throws {
        let controller = try makeController()
        controller.sketchFrames.begin(3)
        controller.sketchFrames.setOpen(3, true)
        let result = await controller.sketchedDictation(text: "Synthetic text", rawText: nil, words: [], session: 3)
        XCTAssertTrue(result.drawingFailed)
        controller.sketchFrames.begin(4)
        controller.sketchFrames.markCaptureFailed(3)
        XCTAssertFalse(controller.sketchFrames.hasFailedCaptures(4))
        controller.sketchFrames.markCaptureFailed(4)
        controller.sketchFrames.discard(4)
        XCTAssertFalse(controller.sketchFrames.hasFailedCaptures(4))
    }

    func testDrawingWarningCannotReplaceAPasteFailure() {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        XCTAssertTrue(overlay.showNotPasted(text: "Synthetic text", reason: "Focus changed", pasteboard: board))
        overlay.showDone(text: "Synthetic text", drawingFailed: true)
        XCTAssertEqual(overlay.latestOutcome, .failure("Not pasted · Focus changed · Text saved"))
    }

    private func makeController() throws -> KoettController {
        let name = "koett-sketch-failure-" + UUID().uuidString
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        return try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))))
    }
}

import AppKit
@testable import Koett
import XCTest

@MainActor
final class ClipboardRecoveryTests: XCTestCase {
    func testSkippedPasteSavesOnceAndOnlyConfirmsVerifiedClipboard() {
        for clipboardWorks in [true, false] {
            let board = NSPasteboard.withUniqueName()
            defer { board.releaseGlobally() }
            let overlay = RecordingOverlayController()
            defer { overlay.stop() }
            overlay.showTranscribing()
            var saves = 0
            XCTAssertThrowsError(try DictationDelivery.deliver("Recovery fixture",
                writeToClipboard: { text in
                    clipboardWorks && SketchClipboard(text: text, png: nil).write(to: board)
                }, postPaste: { false }, persist: {
                    saves += 1
                    return .init(historySaved: true, recoveryURL: nil, lastTranscriptSaved: true, warning: nil)
                })) { _ in
                XCTAssertTrue(overlay.showNotPasted(text: "Recovery fixture", pasteboard: board))
            }
            XCTAssertEqual(saves, 1)
            XCTAssertEqual(overlay.pillView.content, clipboardWorks ? .copied : .notPasted)
        }
    }

    func testFailedPasteVerifiesDeliveryClipboardThenFadesLikeDone() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        XCTAssertTrue(SketchClipboard(text: "Recovery fixture", png: nil).write(to: board))
        XCTAssertTrue(overlay.showNotPasted(text: "Recovery fixture", pasteboard: board))
        XCTAssertEqual(board.string(forType: .string), "Recovery fixture")
        XCTAssertEqual(overlay.pillView.content, .copied)
        XCTAssertEqual(overlay.pillView.preferredHeight, 48)
        XCTAssertTrue(overlay.pillView.copyButton.isHidden)
        XCTAssertTrue(overlay.pillView.pasteButton.isHidden)
        if !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            XCTAssertEqual(overlay.pillView.morphFrom, .transcribing(fraction: nil))
        }
        try await Task.sleep(for: .seconds(RecordingOverlayController.doneDisplayDuration + 0.4))
        XCTAssertFalse(overlay.isPresented)
    }

    func testVerifiedClipboardIsNotRewrittenAndHoverKeepsConfirmation() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        XCTAssertTrue(SketchClipboard(text: "Recovery fixture", png: nil).write(to: board))
        let generation = board.changeCount
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        overlay.showNotPasted(text: "Recovery fixture", pasteboard: board)
        XCTAssertEqual(board.changeCount, generation)
        overlay.pillView.setHovered(true)
        try await Task.sleep(for: .seconds(RecordingOverlayController.doneDisplayDuration + 0.4))
        XCTAssertTrue(overlay.isPresented)
        overlay.pillView.setHovered(false)
        try await Task.sleep(for: .seconds(1.2))
        XCTAssertFalse(overlay.isPresented)
    }

    func testMissingClipboardReadbackKeepsCopySticky() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        overlay.showNotPasted(text: "Recovery fixture", pasteboard: board)
        XCTAssertEqual(overlay.pillView.content, .notPasted)
        try await Task.sleep(for: .seconds(RecordingOverlayController.doneDisplayDuration + 0.4))
        XCTAssertTrue(overlay.isPresented)
        XCTAssertFalse(overlay.pillView.copyButton.isHidden)
        overlay.copyResult(to: board)
        XCTAssertEqual(overlay.pillView.content, .copied)
        XCTAssertEqual(board.string(forType: .string), "Recovery fixture")
        try await Task.sleep(for: .seconds(RecordingOverlayController.doneDisplayDuration + 0.4))
        XCTAssertFalse(overlay.isPresented)
    }

    func testNewerCopyIsPreservedWhenOwnershipWasLost() {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        board.setString("Newer user copy", forType: .string)
        let generation = board.changeCount
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        overlay.showNotPasted(text: "Recovery fixture", pasteboard: board)
        XCTAssertEqual(board.changeCount, generation)
        XCTAssertEqual(board.string(forType: .string), "Newer user copy")
        XCTAssertEqual(overlay.pillView.content, .notPasted)
    }

    func testDrawingRecoveryVerifiesOneItemWithPNGHTMLAndText() throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let png = try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 3, height: 3)?.makeImage())))
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        XCTAssertTrue(SketchClipboard(text: "Recovery fixture", png: png).write(to: board))
        overlay.showNotPasted(text: "Recovery fixture", png: png, pasteboard: board)
        XCTAssertEqual(overlay.pillView.content, .copied)
        XCTAssertEqual(board.pasteboardItems?.count, 1)
        XCTAssertTrue(SketchClipboard(text: "Recovery fixture", png: png).matches(board))
        board.clearContents()
        board.setString("Recovery fixture", forType: .string)
        XCTAssertFalse(SketchClipboard(text: "Recovery fixture", png: png).matches(board))
    }
}

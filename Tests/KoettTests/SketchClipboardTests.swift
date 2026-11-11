import AppKit
@testable import Koett
import XCTest

@MainActor
final class SketchClipboardTests: XCTestCase {
    private func png() throws -> Data {
        try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 3, height: 3)?.makeImage())))
    }

    private var noStorage: TranscriptPersistence.Outcome {
        .init(historySaved: false, recoveryURL: nil, lastTranscriptSaved: false, warning: nil)
    }

    func testAutomaticPasteLeavesOneItemWithExactImageAndTextForRepeatPaste() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let image = try png()
        let text = "- #1 (0:02) “Koett — svenska 👋”"
        var events: [String] = []
        _ = try await SketchPaste.deliver(png: image, text: text, pasteboard: board, postImage: {
            XCTAssertEqual(board.data(forType: .png), image)
            XCTAssertNil(board.string(forType: .string))
            events.append("image")
            return true
        }, postText: {
            XCTAssertNil(board.data(forType: .png), "The second automatic paste must consume text, not another image")
            XCTAssertEqual(board.string(forType: .string), text)
            events.append("text")
            return true
        }, persist: {
            events.append("save")
            return self.noStorage
        })
        XCTAssertEqual(events, ["image", "text", "save"])
        XCTAssertEqual(board.pasteboardItems?.count, 1)
        for _ in 0..<3 {
            XCTAssertEqual(board.data(forType: .png), image)
            XCTAssertEqual(board.string(forType: .string), text)
            XCTAssertNotNil(board.data(forType: .tiff))
            XCTAssertEqual(board.string(forType: .html), SketchClipboard.html(text: text, png: image))
        }
        XCTAssertNotNil(NSImage(pasteboard: board), "A native image consumer must still receive the sketch")
        let editor = NSTextView(frame: .zero)
        editor.isRichText = false
        XCTAssertTrue(editor.readSelection(from: board, type: .string))
        XCTAssertEqual(editor.string, text)
    }

    func testTheTextWaitsForTheDrawingToSettleSoOneEnterSendsBoth() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let image = try png()
        var imageReadAt: ContinuousClock.Instant?
        var textPostedAt: ContinuousClock.Instant?
        _ = try await SketchPaste.deliver(png: image, text: "Settled text", pasteboard: board, postImage: {
            _ = board.data(forType: .png)
            imageReadAt = .now
            return true
        }, postText: {
            textPostedAt = .now
            _ = board.string(forType: .string)
            return true
        }, persist: { self.noStorage })
        let read = try XCTUnwrap(imageReadAt), posted = try XCTUnwrap(textPostedAt)
        XCTAssertGreaterThanOrEqual(posted - read, SketchPaste.drawingSettle,
            "The text must land after the target has had time to attach the drawing")
        XCTAssertEqual(board.data(forType: .png), image, "The drawing stays on the clipboard")
        XCTAssertEqual(board.string(forType: .string), "Settled text")
    }

    func testRestorationWaitsForTheAsynchronousTextRead() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let image = try png()
        var consumer: Task<Void, Never>?
        _ = try await SketchPaste.deliver(png: image, text: "Delayed text", pasteboard: board, postImage: {
            _ = board.data(forType: .png)
            return true
        }, postText: {
            consumer = Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(50))
                XCTAssertNil(board.data(forType: .png))
                XCTAssertEqual(board.string(forType: .string), "Delayed text")
            }
            return true
        }, persist: { self.noStorage })
        await consumer?.value
        XCTAssertEqual(board.data(forType: .png), image)
        XCTAssertEqual(board.string(forType: .string), "Delayed text")
    }

    func testFailedImageOrTextPostStillOffersBothFormatsAndSavesOnce() async throws {
        for failImage in [true, false] {
            let board = NSPasteboard.withUniqueName()
            defer { board.releaseGlobally() }
            let image = try png()
            var saves = 0
            do {
                _ = try await SketchPaste.deliver(png: image, text: "Recover both", pasteboard: board, postImage: {
                    _ = board.data(forType: .png)
                    return !failImage
                }, postText: { false }, persist: {
                    saves += 1
                    return self.noStorage
                })
                XCTFail("The failed paste must reach Not pasted recovery")
            } catch {}
            XCTAssertEqual(saves, 1)
            XCTAssertEqual(board.data(forType: .png), image)
            XCTAssertEqual(board.string(forType: .string), "Recover both")
        }
    }

    func testRestorationDoesNotOverwriteANewerClipboardOwner() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        _ = try await SketchPaste.deliver(png: png(), text: "Old dictation", pasteboard: board, postImage: {
            _ = board.data(forType: .png)
            return true
        }, postText: {
            _ = board.string(forType: .string)
            board.clearContents()
            return board.setString("Copied elsewhere", forType: .string)
        }, persist: { self.noStorage })
        XCTAssertEqual(board.string(forType: .string), "Copied elsewhere")
        XCTAssertNil(board.data(forType: .png))
    }

    func testHTMLContainsEmbeddedImageAndEscapedExactText() throws {
        let image = try png()
        let html = SketchClipboard.html(text: "<literal> & “Unicode”\nsecond line", png: image)
        XCTAssertEqual(html, "<p><img alt=\"Koett sketch\" src=\"data:image/png;base64,\(image.base64EncodedString())\"></p><pre>&lt;literal&gt; &amp; “Unicode”\nsecond line</pre>")
    }

    func testNewCopyDuringImageStageSurvivesAndSavesOnce() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        var saves = 0
        var textPosts = 0
        do {
            _ = try await SketchPaste.deliver(png: png(), text: "Old dictation", pasteboard: board, postImage: {
                _ = board.data(forType: .png)
                board.clearContents()
                return board.setString("New user copy", forType: .string)
            }, postText: {
                textPosts += 1
                return true
            }, persist: {
                saves += 1
                return self.noStorage
            })
            XCTFail("Interrupted clipboard ownership must offer recovery")
        } catch {}
        XCTAssertEqual(saves, 1)
        XCTAssertEqual(textPosts, 0)
        XCTAssertEqual(board.string(forType: .string), "New user copy")
        XCTAssertNil(board.data(forType: .png))
    }

    func testDoneAndNotPastedCopyKeepBothFormatsAndClearAnOlderImage() throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        let image = try png()
        overlay.showTranscribing()
        overlay.showDone(text: "Done with ink", png: image)
        overlay.copyResult(to: board)
        XCTAssertEqual(board.pasteboardItems?.count, 1)
        XCTAssertEqual(board.data(forType: .png), image)
        XCTAssertEqual(board.string(forType: .string), "Done with ink")
        overlay.showTranscribing()
        XCTAssertTrue(overlay.showNotPasted(text: "Recover ink", png: image, pasteboard: board))
        overlay.copyResult(to: board)
        XCTAssertEqual(board.data(forType: .png), image)
        XCTAssertEqual(board.string(forType: .string), "Recover ink")
        overlay.showTranscribing()
        overlay.showDone(text: "Plain next result")
        overlay.copyResult(to: board)
        XCTAssertNil(board.data(forType: .png))
        XCTAssertEqual(board.string(forType: .string), "Plain next result")
    }

    func testNewPlainTranscriptClearsTheImageEvenWhenItsTextIsIdentical() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("koett-sketch-clipboard-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let defaults = try XCTUnwrap(UserDefaults(suiteName: "koett-sketch-clipboard-" + UUID().uuidString))
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))))
        controller.persistTranscript("Same words", model: "test", keepAsLast: true, image: try png())
        XCTAssertNotNil(controller.lastTranscriptImage)
        controller.persistTranscript("Same words", model: "test", keepAsLast: true)
        XCTAssertNil(controller.lastTranscriptImage)
    }
}

import AppKit
@testable import Koett
import XCTest

final class LastTranscriptDeliveryTests: XCTestCase {
    @MainActor
    private func makeController() throws -> KoettController {
        let name = "koett-last-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        addTeardownBlock { UserDefaults(suiteName: name)?.removePersistentDomain(forName: name) }
        return try KoettController(
            defaults: defaults,
            speechEngine: .parakeet,
            availableMicrophones: { [] },
            defaultMicrophoneID: { 0 }
        )
    }

    @MainActor
    func testCopyLastWorksDuringSetupWhenProgressReplacedTheOpenMenu() throws {
        let controller = try makeController()
        controller.lastTranscript = "Restored text."
        XCTAssertEqual(controller.state, .loading)
        XCTAssertEqual(controller.makeMenu().item(withTitle: "Copy Last Transcript")?.isEnabled, true)

        let copied = expectation(description: "copied")
        controller.writeLastTranscriptToClipboard = { text in
            XCTAssertEqual(text, "Restored text.")
            copied.fulfill()
            return true
        }
        controller.copyLastTranscript()
        controller.menuDidClose(NSMenu())
        wait(for: [copied], timeout: 2)
        controller.recordingOverlay.stop()
    }

    @MainActor
    func testCopyLastStaysOffWithoutATranscriptAndDuringASession() throws {
        let controller = try makeController()
        controller.lastTranscript = nil
        XCTAssertEqual(controller.makeMenu().item(withTitle: "Copy Last Transcript")?.isEnabled, false)

        controller.lastTranscript = "Text."
        controller.state = .recording
        XCTAssertEqual(controller.makeMenu().item(withTitle: "Copy Last Transcript")?.isEnabled, false)
        controller.state = .loading
    }

    func testMenuActionsRunOnceOnlyAfterTheMenuCloses() throws {
        var queue = LastTranscriptMenuActionQueue()
        var delivered: [String] = []

        queue.queue(.copy("Copy me."))
        XCTAssertTrue(delivered.isEmpty)
        if case .copy(let text) = try XCTUnwrap(queue.takeAfterMenuDidClose()) {
            try LastTranscriptDelivery.copy(text) {
                delivered.append("copy:\($0)")
                return true
            }
        } else {
            XCTFail("Expected a queued Copy Last action.")
        }
        XCTAssertNil(queue.takeAfterMenuDidClose())

        queue.queue(.paste("Paste me."))
        XCTAssertEqual(delivered, ["copy:Copy me."])
        if case .paste(let text) = try XCTUnwrap(queue.takeAfterMenuDidClose()) {
            try LastTranscriptDelivery.paste(
                text,
                writeToClipboard: {
                    delivered.append("clipboard:\($0)")
                    return true
                },
                postPaste: {
                    delivered.append("paste")
                    return true
                }
            )
        } else {
            XCTFail("Expected a queued Paste Last action.")
        }
        XCTAssertNil(queue.takeAfterMenuDidClose())
        XCTAssertEqual(
            delivered,
            ["copy:Copy me.", "clipboard:Paste me.", "paste"]
        )
    }

    func testCopyDoesNotModifyTheTranscript() throws {
        var clipboardText: String?

        try LastTranscriptDelivery.copy("Keep\nthis exactly.") {
            clipboardText = $0
            return true
        }

        XCTAssertEqual(clipboardText, "Keep\nthis exactly.")
    }

    func testCopyReportsClipboardFailure() {
        XCTAssertThrowsError(
            try LastTranscriptDelivery.copy("Keep me.") { _ in false }
        ) { error in
            XCTAssertEqual(
                error.localizedDescription,
                "The last transcript could not be copied to the clipboard."
            )
        }
    }

    func testPasteStopsWhenClipboardWriteFails() {
        var postedPaste = false

        XCTAssertThrowsError(
            try LastTranscriptDelivery.paste(
                "Keep me.",
                writeToClipboard: { _ in false },
                postPaste: {
                    postedPaste = true
                    return true
                }
            )
        )
        XCTAssertFalse(postedPaste)
    }

    func testPasteLeavesTextOnClipboardWhenCommandVFails() {
        var clipboardText: String?

        XCTAssertThrowsError(
            try LastTranscriptDelivery.paste(
                "Keep me.",
                writeToClipboard: {
                    clipboardText = $0
                    return true
                },
                postPaste: { false }
            )
        ) { error in
            XCTAssertEqual(
                error.localizedDescription,
                "The last transcript is on the clipboard, "
                    + "but Command-V could not be sent."
            )
        }
        XCTAssertEqual(clipboardText, "Keep me.")
    }

    func testPasteWritesBeforeItPostsCommandV() throws {
        var events: [String] = []

        try LastTranscriptDelivery.paste(
            "Keep me.",
            writeToClipboard: {
                events.append("clipboard:\($0)")
                return true
            },
            postPaste: {
                events.append("paste")
                return true
            }
        )

        XCTAssertEqual(events, ["clipboard:Keep me.", "paste"])
    }
}

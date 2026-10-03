@testable import Koett
import Foundation
import XCTest

@MainActor
final class DictationDeliveryTests: XCTestCase {
    func testPasteIsPostedBeforeSavingAndReturnsTheEarlierTimestamp() throws {
        var events: [String] = []
        let timestamp = try DictationDelivery.deliver(
            "A final transcript.",
            writeToClipboard: { _ in events.append("clipboard"); return true },
            postPaste: { events.append("paste"); return true },
            persist: { events.append("save"); return savedOutcome() },
            clock: { events.append("clock"); return 123 }
        )

        XCTAssertEqual(events, ["clipboard", "paste", "clock", "save"])
        XCTAssertEqual(timestamp, 123)
    }

    func testClipboardFailureStillPersistsOnceAndDoesNotPostPaste() {
        var saves = 0
        var pastePosted = false
        XCTAssertThrowsError(try DictationDelivery.deliver(
            "Recover me.",
            writeToClipboard: { _ in false },
            postPaste: { pastePosted = true; return true },
            persist: { saves += 1; return savedOutcome() }
        ))
        XCTAssertEqual(saves, 1)
        XCTAssertFalse(pastePosted)
    }

    func testPasteFailureStillPersistsOnce() {
        var saves = 0
        XCTAssertThrowsError(try DictationDelivery.deliver(
            "Recover me.",
            writeToClipboard: { _ in true },
            postPaste: { false },
            persist: { saves += 1; return savedOutcome() }
        )) { error in
            XCTAssertTrue(error.localizedDescription.contains("on the clipboard"))
        }
        XCTAssertEqual(saves, 1)
    }

    func testDeliveryFailureKeepsTotalStorageFailureVisible() {
        XCTAssertThrowsError(try DictationDelivery.deliver(
            "Recover me.",
            writeToClipboard: { _ in false },
            postPaste: { true },
            persist: {
                TranscriptPersistence.Outcome(
                    historySaved: false,
                    recoveryURL: nil,
                    lastTranscriptSaved: false,
                    warning: "Text in memory · Save failed · Copy Unsaved Transcripts"
                )
            }
        )) { error in
            XCTAssertEqual(
                DictationFailurePresentation.message(
                    error: error.localizedDescription,
                    transcriptionCompleted: true,
                    audioSaved: false
                ),
                "Text in memory · Save failed · Copy Unsaved Transcripts"
            )
        }
    }

    private func savedOutcome() -> TranscriptPersistence.Outcome {
        TranscriptPersistence.Outcome(
            historySaved: true,
            recoveryURL: nil,
            lastTranscriptSaved: true,
            warning: nil
        )
    }
}

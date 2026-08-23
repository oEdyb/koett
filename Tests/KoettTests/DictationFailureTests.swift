@testable import Koett
import XCTest

final class DictationFailureTests: XCTestCase {
    func testExplainsSavedAudioAndPasteRecovery() {
        XCTAssertEqual(
            DictationFailurePresentation.message(
                error: "The model stopped.",
                transcriptionCompleted: false,
                audioSaved: true
            ),
            "Dictation failed · Audio saved"
        )
        XCTAssertEqual(
            DictationFailurePresentation.message(
                error: "The transcript is on the clipboard, but Command-V could not be sent.",
                transcriptionCompleted: true,
                audioSaved: false
            ),
            "Text copied · Paste failed"
        )
    }

    func testExplainsMicrophoneStartFailure() {
        XCTAssertEqual(
            DictationFailurePresentation.message(
                error: "The microphone recorder could not start.",
                transcriptionCompleted: false,
                audioSaved: false
            ),
            "Microphone couldn't start"
        )
    }
}

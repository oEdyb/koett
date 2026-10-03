@testable import Koett
import XCTest

@MainActor
final class AssistantPanelStateTests: XCTestCase {
    func testStreamFailureKeepsPartialAnswerAndQuestion() {
        let model = AssistantPanelModel()
        model.startListening()
        model.beginAnswer(question: "Question")
        model.answer = "The partial answer"

        model.failAnswer("The response ended before it finished.")

        XCTAssertEqual(model.answer, "The partial answer")
        XCTAssertEqual(model.question, "Question")
        XCTAssertEqual(model.phase, .error)
        XCTAssertEqual(model.responseError, "The response ended before it finished.")
        XCTAssertTrue(model.canCopyAnswer)
        XCTAssertTrue(model.screenIncluded)
    }

    func testHistoryWarningSurvivesAnswerAndStreamFailure() {
        let model = AssistantPanelModel()
        model.startListening()
        model.storageWarning = "History unavailable · Transcript saved for recovery"
        model.beginAnswer(question: "Question")
        model.failAnswer("Provider failed")

        XCTAssertEqual(model.storageWarning, "History unavailable · Transcript saved for recovery")
        XCTAssertEqual(model.responseError, "Provider failed")
    }

    func testNewQuestionClearsPreviousErrorsAndPartialAnswer() {
        let model = AssistantPanelModel()
        model.beginAnswer(question: "Old question")
        model.answer = "Old partial answer"
        model.storageWarning = "History unavailable"
        model.failAnswer("Provider failed")

        model.startListening()

        XCTAssertEqual(model.phase, .listening)
        XCTAssertEqual(model.question, "")
        XCTAssertEqual(model.answer, "")
        XCTAssertNil(model.responseError)
        XCTAssertNil(model.storageWarning)
    }

    func testCaptureFailureDoesNotClaimScreenWasIncluded() {
        let model = AssistantPanelModel()
        model.startListening()
        model.phase = .transcribing

        model.failAnswer("Screen capture failed")

        XCTAssertFalse(model.screenIncluded)
        XCTAssertFalse(model.canCopyAnswer)
        XCTAssertEqual(model.responseError, "Screen capture failed")
    }
}

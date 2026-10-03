@testable import Koett
import XCTest

@MainActor
final class AssistantSpeechConnectionTests: XCTestCase {
    func testStoppedConnectionRejectsLateAudioAndErrors() {
        let connection = AssistantSpeechConnection()
        let id = connection.begin()
        XCTAssertTrue(connection.accepts(id))

        connection.stop()

        XCTAssertFalse(connection.accepts(id))
        XCTAssertFalse(connection.bufferScheduled(for: id))
    }

    func testNewConnectionRejectsOldReceiveAndPlaybackCallbacks() {
        let connection = AssistantSpeechConnection()
        let old = connection.begin()
        XCTAssertTrue(connection.bufferScheduled(for: old))
        let current = connection.begin()
        XCTAssertTrue(connection.bufferScheduled(for: current))

        XCTAssertFalse(connection.accepts(old))
        XCTAssertFalse(connection.bufferPlayed(for: old))
        XCTAssertTrue(connection.accepts(current))
        XCTAssertFalse(connection.finishGeneration(for: current))
        XCTAssertTrue(connection.bufferPlayed(for: current))
    }

    func testDoneWaitsForLastAudioBufferBeforeStoppingPlayback() {
        let connection = AssistantSpeechConnection()
        let id = connection.begin()
        XCTAssertTrue(connection.bufferScheduled(for: id))
        XCTAssertTrue(connection.bufferScheduled(for: id))

        XCTAssertFalse(connection.finishGeneration(for: id))
        XCTAssertFalse(connection.accepts(id))
        XCTAssertFalse(connection.bufferPlayed(for: id))
        XCTAssertTrue(connection.bufferPlayed(for: id))
        XCTAssertFalse(connection.bufferPlayed(for: id))
    }

    func testDoneStopsImmediatelyWhenNoAudioRemains() {
        let connection = AssistantSpeechConnection()
        let id = connection.begin()
        XCTAssertTrue(connection.bufferScheduled(for: id))
        XCTAssertFalse(connection.bufferPlayed(for: id))

        XCTAssertTrue(connection.finishGeneration(for: id))
        XCTAssertFalse(connection.accepts(id))
    }

    func testOldSendFailureCannotStopNewConnection() async {
        let connection = AssistantSpeechConnection()
        let old = connection.begin()
        let gate = AssistantTestGate()
        var failures = 0
        let send = Task {
            await connection.send(
                for: old,
                operation: {
                    await gate.wait()
                    throw AssistantTestError.failed
                },
                sent: { XCTFail("Old send unexpectedly succeeded") },
                failed: { _ in failures += 1 }
            )
        }
        await gate.waitUntilEntered()
        let current = connection.begin()
        gate.open()
        await send.value

        XCTAssertEqual(failures, 0)
        XCTAssertTrue(connection.accepts(current))
    }

    func testStoppedSendCannotMarkNextQuestionAsSent() async {
        let connection = AssistantSpeechConnection()
        let old = connection.begin()
        let gate = AssistantTestGate()
        var sends = 0
        let send = Task {
            await connection.send(
                for: old,
                operation: { await gate.wait() },
                sent: { sends += 1 },
                failed: { _ in XCTFail("Unexpected send failure") }
            )
        }
        await gate.waitUntilEntered()
        connection.stop()
        gate.open()
        await send.value

        XCTAssertEqual(sends, 0)
    }

    func testCancelledSendCannotMutateConnection() async {
        let connection = AssistantSpeechConnection()
        let id = connection.begin()
        let gate = AssistantTestGate()
        var sends = 0
        let send = Task {
            await connection.send(
                for: id,
                operation: { await gate.wait() },
                sent: { sends += 1 },
                failed: { _ in XCTFail("Unexpected send failure") }
            )
        }
        await gate.waitUntilEntered()
        send.cancel()
        gate.open()
        await send.value

        XCTAssertEqual(sends, 0)
        XCTAssertTrue(connection.accepts(id))
    }

    func testCurrentSendFailureStillReportsError() async {
        let connection = AssistantSpeechConnection()
        let id = connection.begin()
        var failures = 0
        await connection.send(
            for: id,
            operation: { throw AssistantTestError.failed },
            sent: { XCTFail("Send unexpectedly succeeded") },
            failed: { _ in failures += 1 }
        )
        XCTAssertEqual(failures, 1)
    }
}

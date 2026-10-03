@testable import Koett
import Foundation
import XCTest

@MainActor
final class AssistantLifecycleTests: XCTestCase {
    func testInterruptCancelsScreenshotAndCoalescesRestartRequests() async {
        let workGate = AssistantTestGate()
        let captureGate = AssistantTestGate()
        let lifecycle = AssistantWorkLifecycle()
        let work = Task { await workGate.wait() }
        let capture = Task<Data, Error> {
            await captureGate.wait()
            return Data()
        }
        lifecycle.workTask = work
        lifecycle.captureTask = capture
        await workGate.waitUntilEntered()
        await captureGate.waitUntilEntered()

        XCTAssertNotNil(lifecycle.interrupt())
        XCTAssertNil(lifecycle.interrupt())
        XCTAssertTrue(work.isCancelled)
        XCTAssertTrue(capture.isCancelled)
        workGate.open()
        captureGate.open()
        await work.value
        _ = try? await capture.value
        lifecycle.finish()
        XCTAssertNil(lifecycle.workTask)
        XCTAssertNil(lifecycle.captureTask)
    }

    func testFinishingFailedQuestionCancelsOutstandingScreenshot() async {
        let gate = AssistantTestGate()
        let lifecycle = AssistantWorkLifecycle()
        let capture = Task<Data, Error> {
            await gate.wait()
            return Data()
        }
        lifecycle.captureTask = capture
        await gate.waitUntilEntered()
        var speechStops = 0

        lifecycle.finish { speechStops += 1 }

        XCTAssertTrue(capture.isCancelled)
        XCTAssertNil(lifecycle.captureTask)
        XCTAssertEqual(speechStops, 1)
        gate.open()
        _ = try? await capture.value
    }

    func testLifecycleDeallocationCancelsOwnedTasks() async {
        let gate = AssistantTestGate()
        let task = Task { await gate.wait() }
        var lifecycle: AssistantWorkLifecycle? = AssistantWorkLifecycle()
        lifecycle?.workTask = task
        weak var weakLifecycle = lifecycle
        await gate.waitUntilEntered()

        lifecycle = nil

        XCTAssertNil(weakLifecycle)
        XCTAssertTrue(task.isCancelled)
        gate.open()
        await task.value
    }

    func testCancelledCaptureStopsBeforeUsingScreenCaptureKit() async {
        let gate = AssistantTestGate()
        let task = Task {
            await gate.wait()
            return try await ScreenContextCapture.captureJPEG(displayID: 0)
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()

        do {
            _ = try await task.value
            XCTFail("Expected cancellation before capture")
        } catch is CancellationError {} catch {
            XCTFail("Cancelled capture attempted screen access: \(error)")
        }
    }

    func testCancelledASRKeepsFinalQuestionWithoutSendingItToCloud() async {
        let gate = AssistantTestGate()
        let probe = AssistantPipelineProbe()
        let task = Task {
            try await probe.run(transcribe: {
                await gate.wait()
                return "  The completed question.  "
            })
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()

        await assertCancelled(task)
        XCTAssertEqual(probe.savedQuestions, ["The completed question."])
        XCTAssertEqual(probe.events, ["saved"])
        XCTAssertEqual(probe.failedAudioCount, 0)
    }

    func testCancelledScreenshotNeverBeginsCloudAnswer() async {
        let gate = AssistantTestGate()
        let probe = AssistantPipelineProbe()
        let task = Task {
            try await probe.run(screenshot: {
                await gate.wait()
                return Data([1])
            })
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()

        await assertCancelled(task)
        XCTAssertEqual(probe.events, ["saved"])
    }

    func testCancelledStreamDoesNotFinishSpeechOrAnswer() async {
        let gate = AssistantTestGate()
        let probe = AssistantPipelineProbe()
        let task = Task {
            try await probe.run(stream: {
                await gate.wait()
            })
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()

        await assertCancelled(task)
        XCTAssertEqual(probe.events, ["saved", "began", "streamed"])
    }

    func testCancelledSpeechDoesNotFinishAnswer() async {
        let gate = AssistantTestGate()
        let probe = AssistantPipelineProbe()
        let task = Task {
            try await probe.run(finishSpeech: {
                await gate.wait()
            })
        }
        await gate.waitUntilEntered()
        task.cancel()
        gate.open()

        await assertCancelled(task)
        XCTAssertEqual(probe.events, ["saved", "began", "streamed", "speech"])
    }

    func testASRFailurePreservesAudioWithoutCloudWork() async {
        let probe = AssistantPipelineProbe()
        do {
            try await probe.run(transcribe: { throw AssistantTestError.failed })
            XCTFail("Expected ASR failure")
        } catch {}

        XCTAssertEqual(probe.failedAudioCount, 1)
        XCTAssertTrue(probe.savedQuestions.isEmpty)
        XCTAssertTrue(probe.events.isEmpty)
    }

    func testEmptyQuestionPreservesAudio() async {
        let probe = AssistantPipelineProbe()
        do {
            try await probe.run(transcribe: { " \n" })
            XCTFail("Expected empty-question failure")
        } catch {}

        XCTAssertEqual(probe.failedAudioCount, 1)
        XCTAssertTrue(probe.savedQuestions.isEmpty)
    }

    func testCloudFailureKeepsQuestionWithoutPreservingSuccessfulAudio() async {
        let probe = AssistantPipelineProbe()
        do {
            try await probe.run(stream: { throw AssistantTestError.failed })
            XCTFail("Expected cloud failure")
        } catch {}

        XCTAssertEqual(probe.savedQuestions, ["Question"])
        XCTAssertEqual(probe.failedAudioCount, 0)
        XCTAssertEqual(probe.events, ["saved", "began", "streamed"])
    }

    func testSuccessSavesBeforeCloudAndFinishesOnce() async throws {
        let probe = AssistantPipelineProbe()
        try await probe.run()

        XCTAssertEqual(probe.events, ["saved", "began", "streamed", "speech", "finished"])
        XCTAssertEqual(probe.savedQuestions, ["Question"])
        XCTAssertEqual(probe.failedAudioCount, 0)
    }

    private func assertCancelled(_ task: Task<Void, Error>) async {
        do {
            try await task.value
            XCTFail("Expected cancellation")
        } catch is CancellationError {} catch {
            XCTFail("Unexpected error: \(error)")
        }
    }
}

@MainActor
private final class AssistantPipelineProbe {
    var savedQuestions: [String] = []
    var events: [String] = []
    var failedAudioCount = 0

    func run(
        transcribe: () async throws -> String = { "Question" },
        screenshot: () async throws -> Data = { Data([1]) },
        stream: () async throws -> Void = {},
        finishSpeech: () async -> Void = {}
    ) async throws {
        try await AssistantQuestionPipeline.run(
            transcribe: transcribe,
            transcriptionFailed: { self.failedAudioCount += 1 },
            preserveQuestion: {
                self.savedQuestions.append($0)
                self.events.append("saved")
            },
            screenshot: screenshot,
            beginAnswer: { _ in self.events.append("began") },
            streamAnswer: { _, _ in
                self.events.append("streamed")
                try await stream()
            },
            finishSpeech: {
                self.events.append("speech")
                await finishSpeech()
            },
            finishAnswer: { self.events.append("finished") }
        )
    }
}

@MainActor
final class AssistantTestGate {
    private var entered = false
    private var suspended: CheckedContinuation<Void, Never>?
    private var entryWaiter: CheckedContinuation<Void, Never>?

    func wait() async {
        await withCheckedContinuation { continuation in
            suspended = continuation
            entered = true
            entryWaiter?.resume()
            entryWaiter = nil
        }
    }

    func waitUntilEntered() async {
        guard !entered else { return }
        await withCheckedContinuation { entryWaiter = $0 }
    }

    func open() {
        suspended?.resume()
        suspended = nil
    }
}

enum AssistantTestError: Error {
    case failed
}

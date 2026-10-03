import Foundation

@MainActor
final class AssistantWorkLifecycle {
    var captureTask: Task<Data, Error>?
    var workTask: Task<Void, Never>?
    private var restartRequested = false

    func interrupt() -> Task<Void, Never>? {
        guard let workTask, !restartRequested else { return nil }
        restartRequested = true
        captureTask?.cancel()
        workTask.cancel()
        return workTask
    }

    func finish(stopSpeech: () -> Void = {}) {
        captureTask?.cancel()
        captureTask = nil
        workTask = nil
        restartRequested = false
        stopSpeech()
    }

    deinit {
        captureTask?.cancel()
        workTask?.cancel()
    }
}

@MainActor
enum AssistantQuestionPipeline {
    static func run(
        transcribe: () async throws -> String,
        transcriptionFailed: () -> Void,
        preserveQuestion: (String) -> Void,
        screenshot: () async throws -> Data,
        beginAnswer: (String) -> Void,
        streamAnswer: (String, Data) async throws -> Void,
        finishSpeech: () async -> Void,
        finishAnswer: () -> Void
    ) async throws {
        try Task.checkCancellation()
        let question: String
        do {
            question = try await transcribe()
                .trimmingCharacters(in: .whitespacesAndNewlines)
            guard !question.isEmpty else {
                throw NSError(
                    domain: "Koett",
                    code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Koett did not hear a question."]
                )
            }
        } catch {
            if !Task.isCancelled {
                transcriptionFailed()
            }
            throw error
        }

        preserveQuestion(question)
        try Task.checkCancellation()
        let image = try await screenshot()
        try Task.checkCancellation()
        beginAnswer(question)
        try await streamAnswer(question, image)
        try Task.checkCancellation()
        await finishSpeech()
        try Task.checkCancellation()
        finishAnswer()
    }
}

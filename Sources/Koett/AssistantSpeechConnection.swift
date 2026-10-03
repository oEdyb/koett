import Foundation

@MainActor
final class AssistantSpeechConnection {
    private var id: UUID?
    private var pendingBuffers = 0
    private var generationFinished = false

    func begin() -> UUID {
        let newID = UUID()
        id = newID
        pendingBuffers = 0
        generationFinished = false
        return newID
    }

    func stop() {
        id = nil
        pendingBuffers = 0
        generationFinished = false
    }

    func accepts(_ candidate: UUID) -> Bool {
        id == candidate && !generationFinished && !Task.isCancelled
    }

    func bufferScheduled(for candidate: UUID) -> Bool {
        guard accepts(candidate) else { return false }
        pendingBuffers += 1
        return true
    }

    func bufferPlayed(for candidate: UUID) -> Bool {
        guard id == candidate, pendingBuffers > 0 else { return false }
        pendingBuffers -= 1
        return generationFinished && pendingBuffers == 0
    }

    func finishGeneration(for candidate: UUID) -> Bool {
        guard accepts(candidate) else { return false }
        generationFinished = true
        return pendingBuffers == 0
    }

    func send(
        for candidate: UUID,
        operation: () async throws -> Void,
        sent: () -> Void,
        failed: (Error) -> Void
    ) async {
        guard accepts(candidate) else { return }
        do {
            try await operation()
            guard accepts(candidate) else { return }
            sent()
        } catch {
            guard accepts(candidate) else { return }
            failed(error)
        }
    }
}

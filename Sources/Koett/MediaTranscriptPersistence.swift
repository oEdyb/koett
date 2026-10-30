import Foundation

@MainActor
enum MediaTranscriptPersistence {
    struct Outcome {
        let fileURL: URL?
        let recoveryRecord: TranscriptRecord?
        let recoveryOutcome: TranscriptPersistence.Outcome?

        var message: String? {
            guard let recoveryOutcome else { return nil }
            return recoveryOutcome.warning ?? "Transcript recovered to history"
        }
    }

    static func save(
        _ text: String,
        title: String,
        source: BrowserMediaSource,
        mediaStore: MediaTranscriptStore,
        recovery: TranscriptPersistence
    ) async -> Outcome {
        do {
            let fileURL = try await mediaStore.save(
                text,
                title: title,
                source: source,
                model: "Parakeet v2"
            )
            return Outcome(fileURL: fileURL, recoveryRecord: nil, recoveryOutcome: nil)
        } catch {
            fputs("Warning: media history failed: \(error.localizedDescription)\n", stderr)
            let record = TranscriptRecord(
                text: text,
                model: "Parakeet v2 (Media: \(source.platformName); "
                    + "\(MediaTranscriptStore.cleanTitle(title)); \(source.url.absoluteString))"
            )
            return Outcome(
                fileURL: nil,
                recoveryRecord: record,
                recoveryOutcome: await recovery.persistAsync(record)
            )
        }
    }
}

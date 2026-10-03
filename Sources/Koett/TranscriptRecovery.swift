import Foundation

extension KoettController {
    @discardableResult
    func persistTranscript(
        _ text: String,
        model: String,
        rawText: String? = nil,
        keepAsLast: Bool = false
    ) -> TranscriptPersistence.Outcome {
        let record = TranscriptRecord(text: text, model: model, rawText: rawText)
        if keepAsLast && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            lastTranscript = text
        }
        let outcome = transcriptPersistence.persist(record, keepAsLast: keepAsLast)
        presentTranscriptRecovery(record, outcome: outcome)
        return outcome
    }

    func presentTranscriptRecovery(
        _ record: TranscriptRecord,
        outcome: TranscriptPersistence.Outcome
    ) {
        if !outcome.hasDurableCopy && !record.text.isEmpty {
            lastTranscript = record.text
        }
        if let warning = outcome.warning {
            recordingOverlay.showError(warning)
        }
        rebuildMenu()
    }

    func recoverTranscripts() {
        let outcome = transcriptPersistence.recover()
        if let warning = outcome.warning {
            recordingOverlay.showError(warning)
            fputs("Warning: \(warning)\n", stderr)
        }
    }

    @objc func copyUnsavedTranscripts() {
        guard state == .ready, let text = transcriptPersistence.unsavedTranscriptText else { return }
        lastTranscriptMenuActions.queue(.copy(text))
    }
}

import Foundation

extension KoettController {
    @discardableResult
    func persistTranscript(
        _ text: String,
        model: String,
        rawText: String? = nil,
        keepAsLast: Bool = false,
        image: Data? = nil
    ) -> TranscriptPersistence.Outcome {
        let record = TranscriptRecord(text: text, model: model, rawText: rawText)
        if keepAsLast && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            lastTranscript = text
            lastTranscriptImage = image
        }
        let outcome = transcriptPersistence.enqueue(record, keepAsLast: keepAsLast) { [weak self] outcome in
            self?.presentTranscriptSaveOutcome(outcome)
            if AssistantAvailability.isEnabled { self?.assistantPanel.showStorageWarning(outcome.warning) }
        }
        presentTranscriptRecovery(record, outcome: outcome, image: image)
        return outcome
    }

    func presentTranscriptRecovery(
        _ record: TranscriptRecord,
        outcome: TranscriptPersistence.Outcome,
        image: Data? = nil
    ) {
        if !outcome.hasDurableCopy && !record.text.isEmpty {
            lastTranscript = record.text
            lastTranscriptImage = image
        }
        presentTranscriptSaveOutcome(outcome)
    }

    private func presentTranscriptSaveOutcome(_ outcome: TranscriptPersistence.Outcome) {
        if let warning = outcome.warning {
            recordingOverlay.showError(warning, keepUntilClicked: true)
        }
        rebuildMenu()
    }

    func recoverTranscripts() {
        transcriptPersistence.importHistoryInBackground()
        transcriptPersistence.recoverInBackground { [weak self] outcome in
            guard let self else { return }
            if let warning = outcome.warning {
                recordingOverlay.showError(warning, keepUntilClicked: true)
                fputs("Warning: \(warning)\n", stderr)
            }
            rebuildMenu()
        }
    }

    @objc func retrySaveTranscripts() {
        guard state == .ready else { return }
        transcriptPersistence.recoverInBackground { [weak self] outcome in
            if let warning = outcome.warning {
                self?.recordingOverlay.showError(warning, keepUntilClicked: true)
            } else {
                self?.recordingOverlay.showTransientStatus("Transcripts saved")
            }
            self?.rebuildMenu()
        }
    }

    @objc func copyUnsavedTranscripts() {
        guard state == .ready, let text = transcriptPersistence.unsavedTranscriptText else { return }
        lastTranscriptMenuActions.queue(.copy(text))
    }
}

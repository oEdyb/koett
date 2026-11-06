import AppKit

extension KoettController {
    func prepareTermination() -> NSApplication.TerminateReply {
        switch state {
        case .ready, .loading:
            guard appUpdater?.prepareForTermination() != false else {
                recordingOverlay.showError("Update paused · Could not save rollback state", keepUntilClicked: true)
                return .terminateCancel
            }
            if transcriptPersistence.hasVolatileTranscripts {
                recoverTranscripts()
                guard !transcriptPersistence.hasVolatileTranscripts else {
                    recordingOverlay.showError("Text in memory · Save failed · Copy Unsaved Transcripts", keepUntilClicked: true)
                    return .terminateCancel
                }
            }
            return .terminateNow
        case .recording, .transcribing, .media, .assistantRecording, .assistantWorking:
            if recordingOverlay.isTrackingRecording {
                recordingOverlay.recordFailure("Koett is recording · Stop before quitting")
            } else {
                recordingOverlay.showError("Koett is working · Finish before quitting")
            }
            return .terminateCancel
        }
    }
}

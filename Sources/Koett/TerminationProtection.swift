import AppKit

extension KoettController {
    func prepareTermination() -> NSApplication.TerminateReply {
        switch state {
        case .ready, .loading:
            if transcriptPersistence.hasVolatileTranscripts {
                recoverTranscripts()
                guard !transcriptPersistence.hasVolatileTranscripts else {
                    recordingOverlay.showError("Text in memory · Save failed · Copy Unsaved Transcripts")
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

import Foundation

@MainActor
enum DictationDelivery {
    private struct Failure: LocalizedError {
        let message: String
        let storageWarning: String?

        var errorDescription: String? {
            [message, storageWarning].compactMap { $0 }.joined(separator: " ")
        }
    }

    static func deliver(
        _ text: String,
        writeToClipboard: (String) -> Bool,
        postPaste: () -> Bool,
        persist: () -> TranscriptPersistence.Outcome,
        clock: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    ) throws -> TimeInterval {
        let deliveryFailure: String?
        if !writeToClipboard(text) {
            deliveryFailure = "The transcript could not be copied to the clipboard."
        } else if !postPaste() {
            deliveryFailure = "The transcript is on the clipboard, but Command-V could not be sent."
        } else {
            deliveryFailure = nil
        }
        let pastePostedAt = clock()
        let saved = persist()
        if let deliveryFailure {
            throw Failure(message: deliveryFailure, storageWarning: saved.warning)
        }
        return pastePostedAt
    }
}

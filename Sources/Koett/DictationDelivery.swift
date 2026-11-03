import AppKit

/// `CGEvent.post` reports nothing, so a dropped Command-V looks like a paste.
/// This covers the two drops Koett can see: Accessibility is gone, or Koett is
/// the active app with no text field of its own to receive the keystroke.
/// It must not read focus in other apps; Electron focus is unreadable.
@MainActor
struct PasteTargetGate {
    var isTrusted: @MainActor () -> Bool = { AXIsProcessTrusted() }
    var koettIsActive: @MainActor () -> Bool = { NSApp.isActive }
    var koettTextFieldHasFocus: @MainActor () -> Bool = { NSApp.keyWindow?.firstResponder is NSText }

    func post(_ sendKey: () -> Bool) -> Bool {
        guard isTrusted(), !koettIsActive() || koettTextFieldHasFocus() else { return false }
        return sendKey()
    }
}

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

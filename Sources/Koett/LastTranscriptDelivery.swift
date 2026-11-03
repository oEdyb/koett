import AppKit
import Foundation

struct LastTranscriptDelivery {
    private enum DeliveryError: LocalizedError {
        case clipboard
        case paste

        var errorDescription: String? {
            switch self {
            case .clipboard:
                "The last transcript could not be copied to the clipboard."
            case .paste:
                "The last transcript is on the clipboard, "
                    + "but Command-V could not be sent."
            }
        }
    }

    static func copy(
        _ text: String,
        writeToClipboard: (String) -> Bool
    ) throws {
        guard writeToClipboard(text) else {
            throw DeliveryError.clipboard
        }
    }

    static func paste(
        _ text: String,
        writeToClipboard: (String) -> Bool,
        postPaste: () -> Bool
    ) throws {
        try copy(text, writeToClipboard: writeToClipboard)
        guard postPaste() else {
            throw DeliveryError.paste
        }
    }
}

enum PendingLastTranscriptAction {
    case copy(String)
    case paste(String)
}

struct LastTranscriptMenuActionQueue {
    private var pendingAction: PendingLastTranscriptAction?

    mutating func queue(_ action: PendingLastTranscriptAction) {
        pendingAction = action
    }

    mutating func takeAfterMenuDidClose() -> PendingLastTranscriptAction? {
        defer { pendingAction = nil }
        return pendingAction
    }
}

extension KoettController: NSMenuDelegate {
    /// Works while setup downloads or after it failed. A running session keeps
    /// its pill, which a copy confirmation would replace.
    var canCopyLastTranscript: Bool {
        lastTranscript != nil && (state == .ready || state == .loading)
    }

    @objc func copyLastTranscript() {
        guard canCopyLastTranscript, let lastTranscript else { return }
        lastTranscriptMenuActions.queue(.copy(lastTranscript))
    }

    @objc func pasteLastTranscript() {
        guard state == .ready, let lastTranscript else { return }
        lastTranscriptMenuActions.queue(.paste(lastTranscript))
    }

    /// Pastes the last transcript into the app under the pill.
    func pasteLastTranscriptNow() {
        guard state == .ready, let lastTranscript else { return }
        performLastTranscriptAction(.paste(lastTranscript))
    }

    /// Download progress rebuilds the menu while it is open, so the closing menu
    /// may no longer be the status item's current one.
    func menuDidClose(_ menu: NSMenu) {
        guard let action = lastTranscriptMenuActions.takeAfterMenuDidClose() else { return }

        DispatchQueue.main.async { [weak self] in
            self?.performLastTranscriptAction(action)
        }
    }

    private func performLastTranscriptAction(_ action: PendingLastTranscriptAction) {
        do {
            switch action {
            case .copy(let text):
                try LastTranscriptDelivery.copy(
                    text,
                    writeToClipboard: writeLastTranscriptToClipboard
                )
                recordingOverlay.showTransientStatus("Last transcript copied")
            case .paste(let text):
                try LastTranscriptDelivery.paste(
                    text,
                    writeToClipboard: writeLastTranscriptToClipboard,
                    postPaste: pasteAtCursor
                )
                recordingOverlay.showTransientStatus("Last transcript pasted")
            }
        } catch {
            recordingOverlay.showError(error.localizedDescription)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }
}

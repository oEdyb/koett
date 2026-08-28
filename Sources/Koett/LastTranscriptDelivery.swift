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
    @objc func copyLastTranscript() {
        guard state == .ready, let lastTranscript else { return }
        lastTranscriptMenuActions.queue(.copy(lastTranscript))
    }

    @objc func pasteLastTranscript() {
        guard state == .ready, let lastTranscript else { return }
        lastTranscriptMenuActions.queue(.paste(lastTranscript))
    }

    func menuDidClose(_ menu: NSMenu) {
        guard menu === statusItem?.menu,
              let action = lastTranscriptMenuActions.takeAfterMenuDidClose()
        else { return }

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
                    writeToClipboard: writeToGeneralPasteboard
                )
                recordingOverlay.showTransientStatus("Last transcript copied")
            case .paste(let text):
                try LastTranscriptDelivery.paste(
                    text,
                    writeToClipboard: writeToGeneralPasteboard,
                    postPaste: pasteAtCursor
                )
                recordingOverlay.showTransientStatus("Last transcript pasted")
            }
        } catch {
            recordingOverlay.showError(error.localizedDescription)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func writeToGeneralPasteboard(_ text: String) -> Bool {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        return pasteboard.setString(text, forType: .string)
    }
}

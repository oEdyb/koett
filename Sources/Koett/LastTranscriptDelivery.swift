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

    /// Pastes the last transcript into the app under the pill.
    func pasteLastTranscriptNow() {
        guard state == .ready, let lastTranscript else { return }
        performLastTranscriptAction(.paste(lastTranscript))
    }

    func menuDidClose(_ menu: NSMenu) {
        guard menu === statusItem?.menu,
              let action = lastTranscriptMenuActions.takeAfterMenuDidClose()
        else { return }

        DispatchQueue.main.async { [weak self] in
            self?.performLastTranscriptAction(action)
        }
    }

    func performLastTranscriptAction(_ action: PendingLastTranscriptAction) {
        guard state == .ready else { return }
        if case .paste(let text) = action {
            let png = text == lastTranscript ? lastTranscriptImage : nil
            automaticPasteTarget = PasteTarget.capture()
            state = .transcribing
            recordingOverlay.showTranscribing()
            Task { @MainActor [weak self] in
                guard let self else { return }
                defer { self.state = .ready }
                do {
                    if let png {
                        let delivery = try await self.deliverSketch(text: text, png: png)
                        if !delivery.drawingLanded {
                            self.recordingOverlay.showPartial(textLanded: true, text: text, png: png)
                            return
                        }
                    } else {
                        let ready = await self.pasteCoordinator.prepare(stage: .manual)
                        try LastTranscriptDelivery.paste(text,
                            writeToClipboard: self.writeToGeneralPasteboard,
                            postPaste: { ready && self.pasteAtCursor() })
                    }
                    self.recordingOverlay.showDone(text: text, png: png)
                } catch is SketchPartialPaste {
                    if let png { self.recordingOverlay.showPartial(textLanded: false, text: text, png: png) }
                } catch {
                    let reason = self.pasteCoordinator.lastFailure?.message
                    if !self.recordingOverlay.showNotPasted(text: text, png: png, reason: reason) {
                        self.recordingOverlay.showError(reason ?? error.localizedDescription, keepUntilClicked: true)
                    }
                }
            }
            return
        }
        guard case .copy(let text) = action else { return }
        do {
            try LastTranscriptDelivery.copy(text, writeToClipboard: writeToGeneralPasteboard)
            recordingOverlay.showTransientStatus("Last transcript copied")
        } catch {
            recordingOverlay.showError(error.localizedDescription)
        }
    }

    private func writeToGeneralPasteboard(_ text: String) -> Bool {
        SketchClipboard(text: text, png: text == lastTranscript ? lastTranscriptImage : nil).write()
    }
}

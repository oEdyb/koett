import AppKit
import FluidAudio
import Foundation

extension KoettController {
    func startMediaTranscription() {
        guard canStartSession else { return }
        if AssistantAvailability.isEnabled { assistantSpeech.stop() }
        guard let application = NSWorkspace.shared.frontmostApplication,
              let target = BrowserTabTarget.target(
                  bundleIdentifier: application.bundleIdentifier,
                  localizedName: application.localizedName
              ) else {
            recordingOverlay.showError("Focus a browser tab")
            return
        }

        beginMediaTranscription {
            let browserURL = try await BrowserMedia.currentURL(for: target)
            guard let source = BrowserMediaSource(url: browserURL) else {
                throw Self.failure("The focused tab does not have a web URL.")
            }
            return try await BrowserMedia.fetchAudio(for: source)
        }
    }

    func beginMediaTranscription(load: @escaping @MainActor @Sendable () async throws -> DownloadedMediaAudio) {
        guard canStartSession else { return }
        if AssistantAvailability.isEnabled { assistantSpeech.stop() }
        state = .media
        recordingOverlay.showStatus("Fetching audio…")
        Task { @MainActor [weak self] in await self?.transcribeMedia(load: load) }
    }

    private func transcribeMedia(load: @MainActor @Sendable () async throws -> DownloadedMediaAudio) async {
        defer { state = .ready }
        do {
            let download = try await load()
            do {
                recordingOverlay.showStatus("Transcribing locally…")
                let decoderLayers = await manager.decoderLayerCount
                var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
                let result = try await manager.transcribe(
                    download.audioURL,
                    decoderState: &decoderState
                )
                let text = result.text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !text.isEmpty else {
                    throw Self.failure("No speech was found in the media.")
                }

                let saved = await MediaTranscriptPersistence.save(
                    text,
                    title: download.title,
                    source: download.source,
                    model: speechEngine == .nemotron ? SpeechEngine.parakeet.displayName : speechEngine.displayName,
                    mediaStore: mediaTranscriptStore,
                    recovery: transcriptPersistence
                )
                if let record = saved.recoveryRecord,
                   let outcome = saved.recoveryOutcome {
                    presentTranscriptRecovery(record, outcome: outcome)
                }
                await BrowserMedia.removeTemporaryAudio(
                    at: download.temporaryDirectoryURL
                )
                recordingOverlay.showResult(
                    text,
                    message: saved.message ?? RecordingOverlayFormat.transcriptPreview(text)
                )
                if let fileURL = saved.fileURL {
                    print("MEDIA TRANSCRIPT SAVED: \(fileURL.path)")
                }
            } catch {
                await BrowserMedia.removeTemporaryAudio(
                    at: download.temporaryDirectoryURL
                )
                throw error
            }
        } catch {
            recordingOverlay.showError(Self.shortMediaError(error))
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc func transcribeCurrentMedia() {
        startMediaTranscription()
    }

    @objc func transcribeMediaFile() {
        guard canStartSession else { return }
        let panel = NSOpenPanel()
        panel.title = "Transcribe Media File"
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Transcribe"
        guard panel.runModal() == .OK, let url = panel.url, canStartSession else { return }
        beginMediaTranscription { try await BrowserMedia.localAudio(at: url) }
    }

    @objc func transcribeMediaURL() {
        guard canStartSession else { return }
        let alert = NSAlert()
        alert.messageText = "Transcribe Media URL"
        alert.addButton(withTitle: "Transcribe")
        alert.addButton(withTitle: "Cancel")
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 400, height: 24))
        field.placeholderString = "https://"
        field.setAccessibilityLabel("Media URL")
        alert.accessoryView = field
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn, canStartSession else { return }
        guard let url = URL(string: field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)),
              let source = BrowserMediaSource(url: url) else {
            recordingOverlay.showError("Enter an HTTP or HTTPS URL")
            return
        }
        beginMediaTranscription { try await BrowserMedia.fetchAudio(for: source) }
    }

    @objc func openMediaTranscripts() {
        do {
            try mediaTranscriptStore.prepare()
            guard NSWorkspace.shared.open(mediaTranscriptStore.directoryURL) else {
                throw Self.failure("The media transcript folder could not open.")
            }
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    nonisolated static func shortMediaError(_ error: Error) -> String {
        let message = error.localizedDescription
        let lowercased = message.lowercased()
        if lowercased.contains("does not have a web url") {
            return "Focus a media page"
        }
        if lowercased.contains("needs yt-dlp") || lowercased.contains("needs ffmpeg") {
            return "Media tools are missing"
        }
        if lowercased.contains("unsupported url")
            || lowercased.contains("no suitable extractor") {
            return "Page not supported"
        }
        if lowercased.contains("login")
            || lowercased.contains("cookies")
            || lowercased.contains("private video")
            || lowercased.contains("authentication") {
            return "Media needs login"
        }
        if lowercased.contains("ip address is blocked")
            || lowercased.contains("http error 403")
            || lowercased.contains("forbidden")
            || lowercased.contains("geo-restricted") {
            return "Site blocked the download"
        }
        if lowercased.contains("audio codec")
            || lowercased.contains("does not contain any audio") {
            return "Media has no audio"
        }
        if lowercased.contains("does not pass filter")
            || lowercased.contains("larger than max-filesize") {
            return "Live or long media isn't supported"
        }
        if lowercased.contains("fetch timed out") {
            return "Media fetch timed out"
        }
        if lowercased.contains("no speech was found") {
            return "No speech found"
        }
        return "Media transcription failed"
    }
}

import AppKit
import FluidAudio
import Foundation

extension KoettController {
    func startMediaTranscription() {
        guard state == .ready else { return }
        assistantSpeech.stop()
        guard let application = NSWorkspace.shared.frontmostApplication,
              let target = BrowserTabTarget.target(
                  bundleIdentifier: application.bundleIdentifier,
                  localizedName: application.localizedName
              ) else {
            recordingOverlay.showError("Focus a browser tab")
            return
        }

        state = .media
        rebuildMenu()
        recordingOverlay.showStatus("Finding media…")
        Task { @MainActor [weak self] in
            await self?.transcribeMedia(target: target)
        }
    }

    private func transcribeMedia(target: BrowserTabTarget) async {
        defer {
            state = .ready
            rebuildMenu()
        }

        do {
            let browserURL = try await BrowserMedia.currentURL(for: target)
            guard let source = BrowserMediaSource(url: browserURL) else {
                throw Self.failure("The focused tab does not have a web URL.")
            }

            recordingOverlay.showStatus("Fetching audio…")
            let download = try await BrowserMedia.fetchAudio(for: source)
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

                let fileURL: URL?
                do {
                    fileURL = try await mediaTranscriptStore.save(
                        text,
                        title: download.title,
                        source: download.source,
                        model: "Parakeet v2"
                    )
                } catch {
                    fileURL = nil
                    fputs(
                        "Warning: media transcript was not saved: \(error.localizedDescription)\n",
                        stderr
                    )
                }
                await BrowserMedia.removeTemporaryAudio(
                    at: download.temporaryDirectoryURL
                )
                recordingOverlay.showResult(
                    text,
                    message: fileURL == nil
                        ? "Save failed"
                        : RecordingOverlayFormat.transcriptPreview(text)
                )
                if let fileURL {
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

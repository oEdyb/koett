import AppKit
import AVFoundation
import CoreGraphics
import Darwin
import FluidAudio
import Foundation

private struct PreparedDictation {
    let text: String
    let model: String
    let rawText: String?
}

extension KoettController {
    func toggleRecording(eventTimestamp: TimeInterval) {
        switch state {
        case .ready:
            startRecordingIfReady()
        case .recording:
            stopRecording(releaseEventTimestamp: eventTimestamp)
        case .loading, .transcribing, .media, .assistantRecording, .assistantWorking:
            return
        }
    }

    func startRecordingIfReady() {
        guard canStartSession else { return }
        assistantSpeech.stop()
        do {
            try startRecording()
        } catch {
            state = .ready
            recordingOverlay.showError(
                (error as NSError).domain == "Koett.Microphone"
                    ? "Microphone unavailable · See menu"
                    : DictationFailurePresentation.message(
                        error: error.localizedDescription,
                        transcriptionCompleted: false,
                        audioSaved: false
                    )
            )
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func startRecording() throws {
        switch speechEngine {
        case .parakeet:
            try startParakeetRecording()
        case .nemotron:
            try startNemotronRecording()
        }

        state = .recording
        if let recorder {
            recordingOverlay.start(recorder: recorder)
        }
        play(startSound)
        print("RECORDING")
    }

    private func startParakeetRecording() throws {
        let (newRecorder, url) = try startFileRecording()
        recorder = newRecorder
        recordingURL = url
    }

    private func startNemotronRecording() throws {
        guard let nemotronAdapter else {
            throw Self.failure("Nemotron is not ready.")
        }

        let device = try selectedMicrophone()
        let nemotronRecorder = try NemotronLiveRecorder(adapter: nemotronAdapter, device: device)
        try startNemotronRecording(live: nemotronRecorder) { try startFileRecording(device: device) }
    }

    func startNemotronRecording(
        live nemotronRecorder: NemotronLiveRecorder,
        startRecovery: () throws -> (MicrophoneRecorder, URL)
    ) throws {
        let (recoveryRecorder, recoveryURL) = try startRecovery()
        do {
            try nemotronRecorder.start()
        } catch {
            recoveryRecorder.stop()
            cleanup(recoveryURL)
            throw error
        }

        self.nemotronRecorder = nemotronRecorder
        recorder = recoveryRecorder
        recordingURL = recoveryURL
    }

    func startFileRecording(device: MicrophoneDevice? = nil) throws -> (MicrophoneRecorder, URL) {
        let device = try device ?? selectedMicrophone()
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-\(UUID().uuidString).wav")
        do {
            let recorder = try MicrophoneRecorder(url: url, device: device)
            print("MICROPHONE \(device.name) [\(device.uid)]")
            return (recorder, url)
        } catch {
            cleanup(url)
            throw error
        }
    }

    func handleMicrophoneFailure(_ recorder: MicrophoneRecorder, url: URL) -> Bool {
        guard let error = recorder.recordingError else { return false }
        preserveFailedRecording(url)
        recordingOverlay.showError(error.localizedDescription)
        state = .ready
        fputs("Error: \(error.localizedDescription)\n", stderr)
        return true
    }

    func stopRecording(releaseEventTimestamp: TimeInterval) {
        recordingOverlay.stop()
        switch speechEngine {
        case .parakeet:
            stopParakeetRecording(releaseEventTimestamp: releaseEventTimestamp)
        case .nemotron:
            stopNemotronRecording(releaseEventTimestamp: releaseEventTimestamp)
        }
    }

    private func stopParakeetRecording(releaseEventTimestamp: TimeInterval) {
        guard let recorder, let url = recordingURL else { return }
        let duration = recorder.currentTime
        recorder.stop()
        self.recorder = nil
        recordingURL = nil
        guard !handleMicrophoneFailure(recorder, url: url) else { return }
        state = .transcribing
        play(stopSound)
        print("TRANSCRIBING")

        guard duration >= 0.25 else {
            cleanup(url)
            state = .ready
            print("Ignored: recording was too short.")
            return
        }

        recordingOverlay.showTranscribing()
        Task { @MainActor [weak self] in
            await self?.transcribe(url, releaseEventTimestamp: releaseEventTimestamp)
        }
    }

    private func stopNemotronRecording(releaseEventTimestamp: TimeInterval) {
        guard let nemotronRecorder,
              let recoveryRecorder = recorder,
              let recoveryURL = recordingURL else { return }

        self.nemotronRecorder = nil
        let duration = recoveryRecorder.currentTime
        recoveryRecorder.stop()
        recorder = nil
        recordingURL = nil

        do {
            let pendingCapture = try nemotronRecorder.stop()
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")
            recordingOverlay.showTranscribing()

            Task { @MainActor [weak self] in
                await self?.finishNemotron(
                    pendingCapture,
                    recoveryURL: recoveryURL,
                    duration: max(duration, pendingCapture.store.duration),
                    recoveryError: recoveryRecorder.recordingError,
                    releaseEventTimestamp: releaseEventTimestamp
                )
            }
        } catch {
            guard !handleMicrophoneFailure(recoveryRecorder, url: recoveryURL) else { return }
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")
            recordingOverlay.showTranscribing()
            fputs(
                "Warning: Nemotron stop failed; using Parakeet recovery: \(error.localizedDescription)\n",
                stderr
            )
            Task { @MainActor [weak self] in
                guard let self else { return }
                let transcriptionCompleted = await self.transcribeRecoveryFile(
                    recoveryURL,
                    releaseEventTimestamp: releaseEventTimestamp
                )
                if transcriptionCompleted {
                    self.cleanup(recoveryURL)
                } else {
                    self.preserveFailedRecording(recoveryURL)
                }
                self.recordingOverlay.endTranscribing()
                self.state = .ready
            }
        }
    }

    private func transcribe(_ url: URL, releaseEventTimestamp: TimeInterval) async {
        var transcriptionCompleted = false
        defer {
            if transcriptionCompleted {
                cleanup(url)
            }
            recordingOverlay.endTranscribing()
            state = .ready
        }

        do {
            let transcriptionStartedAt = ProcessInfo.processInfo.systemUptime
            let result = try await DictationTranscriber.transcribe(url, using: manager)
            transcriptionCompleted = true
            let asrReadyAt = ProcessInfo.processInfo.systemUptime
            let rawText = result.text.trimmingCharacters(in: .whitespacesAndNewlines)

            guard !rawText.isEmpty else {
                print("No speech detected. Clipboard unchanged.")
                return
            }

            let prepared = await prepareDictation(
                rawText,
                model: speechEngine.displayName
            )
            let textReadyAt = ProcessInfo.processInfo.systemUptime
            guard !prepared.text.isEmpty else {
                try saveTranscript(prepared)
                print("S1-mini removed filler-only text. Clipboard unchanged.")
                return
            }

            let pastePostedAt = try deliver(prepared)
            print("PASTE POSTED (\(prepared.text.count) characters)")
            print(String(
                format: "LATENCY release-to-ASR-start %.1fms | ASR %.1fms | cleanup %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (transcriptionStartedAt - releaseEventTimestamp) * 1_000,
                (asrReadyAt - transcriptionStartedAt) * 1_000,
                (textReadyAt - asrReadyAt) * 1_000,
                (pastePostedAt - textReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
        } catch {
            if transcriptionCompleted {
                recordingOverlay.showError(
                    DictationFailurePresentation.message(
                        error: error.localizedDescription,
                        transcriptionCompleted: true,
                        audioSaved: false
                    )
                )
            } else {
                preserveFailedRecording(url)
            }
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func finishNemotron(
        _ pendingCapture: NemotronLiveRecorder.PendingCapture,
        recoveryURL: URL,
        duration: TimeInterval,
        recoveryError: Error?,
        releaseEventTimestamp: TimeInterval
    ) async {
        defer {
            recordingOverlay.endTranscribing()
            state = .ready
        }
        guard let nemotronAdapter else {
            preserveFailedRecording(recoveryURL)
            return
        }

        if duration < 0.3 {
            do {
                try await pendingCapture.processingTask.value
            } catch {
                fputs("Warning: Nemotron stopped with: \(error.localizedDescription)\n", stderr)
            }
            await nemotronAdapter.cancel()
            cleanup(recoveryURL)
            print("Ignored: recording was too short.")
            return
        }

        let finalizationStartedAt = ProcessInfo.processInfo.systemUptime
        let rawText: String
        do {
            try await pendingCapture.processingTask.value
            if let error = pendingCapture.error { throw error }
            guard pendingCapture.store.droppedFrames == 0 else {
                throw Self.failure(
                    "Nemotron dropped \(pendingCapture.store.droppedFrames) microphone frames."
                )
            }

            rawText = try await nemotronAdapter.finish()
                .trimmingCharacters(in: .whitespacesAndNewlines)
        } catch {
            fputs(
                "Warning: Nemotron failed; using Parakeet recovery: \(error.localizedDescription)\n",
                stderr
            )
            await nemotronAdapter.cancel()
            if let recoveryError {
                fputs("Warning: Backup recording failed: \(recoveryError.localizedDescription)\n", stderr)
                preserveFailedRecording(recoveryURL)
                return
            }
            let transcriptionCompleted = await transcribeRecoveryFile(
                recoveryURL,
                releaseEventTimestamp: releaseEventTimestamp
            )
            if transcriptionCompleted {
                cleanup(recoveryURL)
            } else {
                preserveFailedRecording(recoveryURL)
            }
            return
        }

        let asrReadyAt = ProcessInfo.processInfo.systemUptime
        cleanup(recoveryURL)
        guard !rawText.isEmpty else {
            print("No speech detected. Clipboard unchanged.")
            return
        }

        do {
            let prepared = await prepareDictation(
                rawText,
                model: speechEngine.displayName
            )
            let textReadyAt = ProcessInfo.processInfo.systemUptime
            guard !prepared.text.isEmpty else {
                try saveTranscript(prepared)
                print("S1-mini removed filler-only text. Clipboard unchanged.")
                return
            }
            let pastePostedAt = try deliver(prepared)
            print("PASTE POSTED (\(prepared.text.count) characters)")
            print(String(
                format: "LATENCY Nemotron release-to-finish-start %.1fms | finish %.1fms | cleanup %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (finalizationStartedAt - releaseEventTimestamp) * 1_000,
                (asrReadyAt - finalizationStartedAt) * 1_000,
                (textReadyAt - asrReadyAt) * 1_000,
                (pastePostedAt - textReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
        } catch {
            recordingOverlay.showError(
                DictationFailurePresentation.message(
                    error: error.localizedDescription,
                    transcriptionCompleted: true,
                    audioSaved: false
                )
            )
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func transcribeRecoveryFile(
        _ url: URL,
        releaseEventTimestamp: TimeInterval
    ) async -> Bool {
        var transcriptionCompleted = false
        do {
            let fallbackStartedAt = ProcessInfo.processInfo.systemUptime
            let result = try await DictationTranscriber.transcribe(url, using: manager)
            transcriptionCompleted = true
            let asrReadyAt = ProcessInfo.processInfo.systemUptime
            let rawText = result.text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !rawText.isEmpty else {
                print("No speech detected. Clipboard unchanged.")
                return true
            }

            let prepared = await prepareDictation(
                rawText,
                model: "Parakeet v2 (Nemotron recovery)"
            )
            let textReadyAt = ProcessInfo.processInfo.systemUptime
            guard !prepared.text.isEmpty else {
                try saveTranscript(prepared)
                print("S1-mini removed filler-only text. Clipboard unchanged.")
                return true
            }
            let pastePostedAt = try deliver(prepared)
            print("PASTE POSTED by Parakeet recovery (\(prepared.text.count) characters)")
            print(String(
                format: "LATENCY recovery-start %.1fms | recovery-ASR %.1fms | cleanup %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (fallbackStartedAt - releaseEventTimestamp) * 1_000,
                (asrReadyAt - fallbackStartedAt) * 1_000,
                (textReadyAt - asrReadyAt) * 1_000,
                (pastePostedAt - textReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
            return true
        } catch {
            if transcriptionCompleted {
                recordingOverlay.showError(
                    DictationFailurePresentation.message(
                        error: error.localizedDescription,
                        transcriptionCompleted: true,
                        audioSaved: false
                    )
                )
            }
            fputs("Error: Parakeet recovery failed: \(error.localizedDescription)\n", stderr)
            return transcriptionCompleted
        }
    }

    private func prepareDictation(
        _ rawText: String,
        model: String
    ) async -> PreparedDictation {
        guard s1MiniEnabled,
              let cleaner = s1MiniCleaner else {
            return PreparedDictation(text: rawText, model: model, rawText: nil)
        }

        do {
            let cleaned = try await cleaner.clean(
                rawText,
                style: s1MiniStyle,
                structure: s1MiniStructure,
                context: s1MiniContext
            )
            return PreparedDictation(
                text: cleaned,
                model: "\(model) + S1-mini by Superwhisper "
                    + "(\(s1MiniStyle.displayName), "
                    + "\(s1MiniStructure.displayName), "
                    + "\(s1MiniContext.displayName))",
                rawText: rawText
            )
        } catch {
            fputs(
                "Warning: S1-mini cleanup failed; pasting raw text: \(error.localizedDescription)\n",
                stderr
            )
            return PreparedDictation(text: rawText, model: model, rawText: nil)
        }
    }

    private func deliver(_ transcript: PreparedDictation) throws -> TimeInterval {
        try DictationDelivery.deliver(
            transcript.text,
            writeToClipboard: { text in
                let pasteboard = NSPasteboard.general
                pasteboard.clearContents()
                return pasteboard.setString(text, forType: .string)
            },
            postPaste: pasteAtCursor,
            persist: {
                persistTranscript(
                    transcript.text,
                    model: transcript.model,
                    rawText: transcript.rawText,
                    keepAsLast: true
                )
            }
        )
    }

    private func saveTranscript(_ transcript: PreparedDictation) throws {
        persistTranscript(
            transcript.text,
            model: transcript.model,
            rawText: transcript.rawText,
            keepAsLast: true
        )
    }

    func play(_ player: AVAudioPlayer) {
        player.stop()
        player.currentTime = 0
        _ = player.play()
    }

    func pasteAtCursor() -> Bool {
        guard let source = CGEventSource(stateID: .hidSystemState),
              let keyDown = CGEvent(
                  keyboardEventSource: source,
                  virtualKey: 9,
                  keyDown: true
              ),
              let keyUp = CGEvent(
                  keyboardEventSource: source,
                  virtualKey: 9,
                  keyDown: false
              ) else {
            return false
        }

        keyDown.flags = .maskCommand
        keyUp.flags = .maskCommand
        keyDown.post(tap: .cghidEventTap)
        keyUp.post(tap: .cghidEventTap)
        return true
    }

    func cleanup(_ url: URL) {
        do {
            try FileManager.default.removeItem(at: url)
        } catch {
            fputs("Warning: temporary audio remains at \(url.path)\n", stderr)
        }
    }

    @discardableResult
    private func preserveFailedRecording(_ url: URL) -> URL? {
        do {
            let savedURL = try failedRecordingStore.preserve(url)
            recordingOverlay.showError("Dictation failed · Audio saved")
            rebuildMenu()
            fputs("Failed dictation audio saved at \(savedURL.path)\n", stderr)
            return savedURL
        } catch {
            NSWorkspace.shared.activateFileViewerSelecting([url])
            recordingOverlay.showError("Dictation failed · Audio shown in Finder")
            fputs(
                "Error: failed dictation audio remains at \(url.path): "
                    + "\(error.localizedDescription)\n",
                stderr
            )
            return nil
        }
    }

    @objc func openFailedRecordings() {
        do {
            try failedRecordingStore.prepare()
            guard NSWorkspace.shared.open(failedRecordingStore.directoryURL) else {
                throw Self.failure("The failed-recording folder could not open.")
            }
        } catch {
            recordingOverlay.showError("Failed recordings couldn't open")
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc func openFailedTranscripts() {
        do {
            try failedTranscriptStore.prepare()
            guard NSWorkspace.shared.open(failedTranscriptStore.directoryURL) else {
                throw Self.failure("The failed-transcript folder could not open.")
            }
        } catch {
            recordingOverlay.showError("Failed transcripts couldn't open")
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }
}

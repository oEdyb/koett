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
        guard state == .ready else { return }
        assistantSpeech.stop()
        do {
            try startRecording()
        } catch {
            state = .ready
            recordingOverlay.showError(
                DictationFailurePresentation.message(
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
        case .parakeet, .parakeetV3:
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
        guard let nemotronRecorder else {
            throw Self.failure("Nemotron is not ready.")
        }

        let (recoveryRecorder, recoveryURL) = try startFileRecording()
        do {
            try nemotronRecorder.start()
        } catch {
            recoveryRecorder.stop()
            cleanup(recoveryURL)
            throw error
        }

        recorder = recoveryRecorder
        recordingURL = recoveryURL
    }

    func startFileRecording() throws -> (AVAudioRecorder, URL) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-\(UUID().uuidString).wav")
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000.0,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 32,
            AVLinearPCMIsFloatKey: true,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ]

        let newRecorder = try AVAudioRecorder(url: url, settings: settings)
        newRecorder.isMeteringEnabled = true
        guard newRecorder.prepareToRecord(), newRecorder.record() else {
            throw Self.failure("The microphone recorder could not start.")
        }

        return (newRecorder, url)
    }

    func stopRecording(releaseEventTimestamp: TimeInterval) {
        recordingOverlay.stop()
        switch speechEngine {
        case .parakeet, .parakeetV3:
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
        state = .transcribing
        play(stopSound)
        print("TRANSCRIBING")

        guard duration >= 0.25 else {
            cleanup(url)
            state = .ready
            print("Ignored: recording was too short.")
            return
        }

        Task { @MainActor [weak self] in
            await self?.transcribe(url, releaseEventTimestamp: releaseEventTimestamp)
        }
    }

    private func stopNemotronRecording(releaseEventTimestamp: TimeInterval) {
        guard let nemotronRecorder,
              let recoveryRecorder = recorder,
              let recoveryURL = recordingURL else { return }

        let duration = recoveryRecorder.currentTime
        recoveryRecorder.stop()
        recorder = nil
        recordingURL = nil

        do {
            let pendingCapture = try nemotronRecorder.stop()
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")

            Task { @MainActor [weak self] in
                await self?.finishNemotron(
                    pendingCapture,
                    recoveryURL: recoveryURL,
                    duration: duration,
                    releaseEventTimestamp: releaseEventTimestamp
                )
            }
        } catch {
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")
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
            state = .ready
        }

        do {
            let transcriptionStartedAt = ProcessInfo.processInfo.systemUptime
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
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
        releaseEventTimestamp: TimeInterval
    ) async {
        defer {
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
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
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
        defer {
            do {
                try saveTranscript(transcript)
            } catch {
                fputs("Warning: transcript was not saved: \(error.localizedDescription)\n", stderr)
            }
        }

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard pasteboard.setString(transcript.text, forType: .string) else {
            throw Self.failure("The transcript could not be copied to the clipboard.")
        }

        guard pasteAtCursor() else {
            throw Self.failure("The transcript is on the clipboard, but Command-V could not be sent.")
        }
        return ProcessInfo.processInfo.systemUptime
    }

    private func saveTranscript(_ transcript: PreparedDictation) throws {
        do {
            try transcriptStore.append(
                transcript.text,
                model: transcript.model,
                rawText: transcript.rawText
            )
        } catch {
            let savedURL = try failedTranscriptStore.preserve(
                transcript.text,
                model: transcript.model,
                rawText: transcript.rawText
            )
            rebuildMenu()
            fputs(
                "Warning: transcript history failed; text recovered at "
                    + "\(savedURL.path): \(error.localizedDescription)\n",
                stderr
            )
        }
    }

    func play(_ player: AVAudioPlayer) {
        player.stop()
        player.currentTime = 0
        _ = player.play()
    }

    private func pasteAtCursor() -> Bool {
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

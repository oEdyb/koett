import AppKit
import FluidAudio
import Foundation
import ScreenCaptureKit

extension KoettController {
    func toggleAssistant() {
        switch state {
        case .ready:
            startAssistantRecording()
        case .assistantRecording:
            stopAssistantRecording()
        case .assistantWorking:
            interruptAssistantAndRestart()
        case .loading, .recording, .transcribing, .media:
            return
        }
    }

    private func startAssistantRecording() {
        guard canStartSession else { return }
        assistantSpeech.stop()

        do {
            _ = try AssistantConfiguration.load(provider: assistantProvider)
            guard try AssistantAPIKeyStore.load(for: assistantProvider) != nil else {
                assistantPanel.showError(
                    "Set a \(assistantProvider.displayName) API key from the Koett menu."
                )
                return
            }

            if assistantSpeechEnabled {
                if let cartesiaKey = try AssistantAPIKeyStore.loadCartesia() {
                    assistantSpeech.prepare(
                        apiKey: cartesiaKey,
                        voice: assistantVoice
                    )
                } else {
                    fputs(
                        "Warning: Set a Cartesia API key to speak Ask replies.\n",
                        stderr
                    )
                }
            }

            let (newRecorder, url) = try startFileRecording()
            let displayID = ScreenContextCapture.activeDisplayID()
            recorder = newRecorder
            recordingURL = url
            assistantWork.captureTask = Task {
                try await ScreenContextCapture.captureJPEG(displayID: displayID)
            }
            state = .assistantRecording
            recordingOverlay.start(recorder: newRecorder)
            assistantPanel.showListening(on: displayID)
            play(startSound)
            rebuildMenu()
            print("KOETT ASK RECORDING")
        } catch {
            assistantWork.finish(stopSpeech: assistantSpeech.stop)
            assistantPanel.showError(error.localizedDescription)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func stopAssistantRecording() {
        guard state == .assistantRecording,
              let recorder,
              let url = recordingURL else { return }

        let duration = recorder.currentTime
        recorder.stop()
        self.recorder = nil
        recordingURL = nil
        recordingOverlay.stop()
        play(stopSound)
        if handleMicrophoneFailure(recorder, url: url) {
            assistantWork.finish(stopSpeech: assistantSpeech.stop)
            assistantPanel.showError(recorder.recordingError?.localizedDescription ?? "Microphone recording failed.")
            rebuildMenu()
            return
        }

        guard duration >= 0.25 else {
            assistantWork.finish(stopSpeech: assistantSpeech.stop)
            cleanup(url)
            state = .ready
            assistantPanel.showError("Speak for a little longer.")
            rebuildMenu()
            return
        }

        state = .assistantWorking
        assistantPanel.showTranscribing()
        rebuildMenu()
        assistantWork.workTask = Task { @MainActor [weak self] in
            await self?.answerAssistantQuestion(from: url)
        }
    }

    private func interruptAssistantAndRestart() {
        guard state == .assistantWorking else { return }
        assistantSpeech.stop()
        guard let currentTask = assistantWork.interrupt() else { return }
        Task { @MainActor [weak self] in
            await currentTask.value
            guard let self, self.state == .ready else { return }
            self.startAssistantRecording()
        }
    }

    private func answerAssistantQuestion(from url: URL) async {
        var shouldCleanup = true
        defer {
            if shouldCleanup {
                cleanup(url)
            }
            assistantWork.finish()
            state = .ready
            rebuildMenu()
        }

        do {
            try await AssistantQuestionPipeline.run(
                transcribe: {
                    return try await DictationTranscriber.transcribe(url, using: self.manager).text
                },
                transcriptionFailed: {
                    shouldCleanup = false
                    self.preserveAssistantRecording(url)
                },
                preserveQuestion: { question in
                    let outcome = self.persistTranscript(
                        question,
                        model: "Parakeet v2 (Koett Ask)"
                    )
                    self.assistantPanel.showStorageWarning(outcome.warning)
                },
                screenshot: {
                    guard let captureTask = self.assistantWork.captureTask else {
                        throw Self.failure("The screen capture did not start.")
                    }
                    return try await captureTask.value
                },
                beginAnswer: { self.assistantPanel.beginAnswer(question: $0) },
                streamAnswer: { question, screenshot in
                    let configuration = try AssistantConfiguration.load(provider: self.assistantProvider)
                    guard let apiKey = try AssistantAPIKeyStore.load(for: self.assistantProvider) else {
                        throw Self.failure(
                            "Set a \(self.assistantProvider.displayName) API key from the Koett menu."
                        )
                    }
                    try await self.assistantClient.streamAnswer(
                        question: question,
                        screenshotJPEG: screenshot,
                        apiKey: apiKey,
                        configuration: configuration
                    ) { [weak self] delta in
                        guard !Task.isCancelled else { return }
                        self?.assistantPanel.appendAnswer(delta)
                        await self?.assistantSpeech.appendResponseDelta(delta)
                    }
                },
                finishSpeech: { await self.assistantSpeech.finishResponse() },
                finishAnswer: { self.assistantPanel.finishAnswer() }
            )
        } catch {
            assistantSpeech.stop()
            if Task.isCancelled {
                return
            }
            let message = Self.shortAssistantError(error)
            assistantPanel.showResponseError(message)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func preserveAssistantRecording(_ url: URL) {
        do {
            let savedURL = try failedRecordingStore.preserve(url)
            recordingOverlay.showError("Ask failed · Audio saved")
            fputs("Failed Ask audio saved at \(savedURL.path)\n", stderr)
        } catch {
            NSWorkspace.shared.activateFileViewerSelecting([url])
            recordingOverlay.showError("Ask failed · Audio shown in Finder")
            fputs("Error: failed Ask audio remains at \(url.path): \(error.localizedDescription)\n", stderr)
        }
    }

    func providerSelector(_ provider: AssistantProvider) -> Selector {
        switch provider {
        case .groq: #selector(useGroqProvider)
        case .openRouter: #selector(useOpenRouterProvider)
        case .custom: #selector(useCustomProvider)
        }
    }

    func voiceSelector(_ voice: AssistantVoice) -> Selector {
        switch voice {
        case .katie: #selector(useKatieVoice)
        case .skylar: #selector(useSkylarVoice)
        case .jameson: #selector(useJamesonVoice)
        case .gemma: #selector(useGemmaVoice)
        case .archie: #selector(useArchieVoice)
        }
    }

    @objc func askAboutScreen() {
        toggleAssistant()
    }

    @objc private func useGroqProvider() {
        setAssistantProvider(.groq)
    }

    @objc private func useOpenRouterProvider() {
        setAssistantProvider(.openRouter)
    }

    @objc private func useCustomProvider() {
        setAssistantProvider(.custom)
    }

    @objc func toggleAssistantSpeech() {
        guard state == .ready else { return }
        assistantSpeechEnabled.toggle()
        UserDefaults.standard.set(
            assistantSpeechEnabled,
            forKey: "assistantSpeechEnabled"
        )
        if !assistantSpeechEnabled {
            assistantSpeech.stop()
        }
        rebuildMenu()
    }

    @objc private func useKatieVoice() {
        setAssistantVoice(.katie)
    }

    @objc private func useSkylarVoice() {
        setAssistantVoice(.skylar)
    }

    @objc private func useJamesonVoice() {
        setAssistantVoice(.jameson)
    }

    @objc private func useGemmaVoice() {
        setAssistantVoice(.gemma)
    }

    @objc private func useArchieVoice() {
        setAssistantVoice(.archie)
    }

    private func setAssistantVoice(_ voice: AssistantVoice) {
        guard state == .ready else { return }
        assistantSpeech.stop()
        assistantVoice = voice
        UserDefaults.standard.set(voice.rawValue, forKey: "assistantVoice")
        rebuildMenu()
    }

    private func setAssistantProvider(_ provider: AssistantProvider) {
        guard state == .ready else { return }
        assistantProvider = provider
        UserDefaults.standard.set(provider.rawValue, forKey: "assistantProvider")
        rebuildMenu()
    }

    @objc func setAssistantModel() {
        guard state == .ready else { return }
        let current = UserDefaults.standard.string(
            forKey: "assistantModel.\(assistantProvider.rawValue)"
        ) ?? assistantProvider.defaultModel
        guard let model = promptForText(
            title: "Set Assistant Model",
            message: "Enter the model ID for \(assistantProvider.displayName).",
            current: current,
            secure: false
        )?.trimmingCharacters(in: .whitespacesAndNewlines),
        !model.isEmpty else { return }

        AssistantConfiguration.saveModel(model, for: assistantProvider)
        rebuildMenu()
    }

    @objc func setCustomAssistantEndpoint() {
        guard state == .ready, assistantProvider == .custom else { return }
        let current = UserDefaults.standard.string(
            forKey: "assistantEndpoint.custom"
        ) ?? assistantProvider.defaultEndpoint
        guard let endpoint = promptForText(
            title: "Set Assistant Endpoint",
            message: "Enter an OpenAI-compatible HTTPS chat completions endpoint.",
            current: current,
            secure: false
        )?.trimmingCharacters(in: .whitespacesAndNewlines),
        let url = URL(string: endpoint),
        url.scheme == "https",
        url.host?.isEmpty == false else { return }

        AssistantConfiguration.saveEndpoint(endpoint, for: assistantProvider)
        rebuildMenu()
    }

    @objc func setAssistantAPIKey() {
        guard state == .ready else { return }
        guard let key = promptForText(
            title: "Set \(assistantProvider.displayName) API Key",
            message: "Koett stores this key in your macOS Keychain.",
            current: "",
            secure: true
        ) else { return }

        do {
            try AssistantAPIKeyStore.save(key, for: assistantProvider)
        } catch {
            assistantPanel.showError(error.localizedDescription)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc func setCartesiaAPIKey() {
        guard state == .ready else { return }
        guard let key = promptForText(
            title: "Set Cartesia API Key",
            message: "Koett stores this key in your macOS Keychain.",
            current: "",
            secure: true
        ) else { return }

        do {
            try AssistantAPIKeyStore.saveCartesia(key)
        } catch {
            assistantPanel.showError(error.localizedDescription)
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc func openScreenRecordingSettings() {
        guard let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        ), NSWorkspace.shared.open(url) else {
            fputs("Error: Screen Recording settings could not open.\n", stderr)
            return
        }
    }

    private static func shortAssistantError(_ error: Error) -> String {
        let nsError = error as NSError
        let message = error.localizedDescription
        if nsError.domain == SCStreamErrorDomain
            || message.localizedCaseInsensitiveContains("screen capture")
            || message.localizedCaseInsensitiveContains("screen recording") {
            return "Allow Screen Recording, then restart Koett."
        }
        return message
    }
}

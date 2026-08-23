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
        guard state == .ready else { return }
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
            screenCaptureTask = Task {
                try await ScreenContextCapture.captureJPEG(displayID: displayID)
            }
            state = .assistantRecording
            recordingOverlay.start(recorder: newRecorder)
            assistantPanel.showListening(on: displayID)
            play(startSound)
            rebuildMenu()
            print("KOETT ASK RECORDING")
        } catch {
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

        guard duration >= 0.25 else {
            screenCaptureTask?.cancel()
            screenCaptureTask = nil
            cleanup(url)
            state = .ready
            assistantPanel.showError("Speak for a little longer.")
            rebuildMenu()
            return
        }

        state = .assistantWorking
        assistantPanel.showTranscribing()
        rebuildMenu()
        assistantWorkTask = Task { @MainActor [weak self] in
            await self?.answerAssistantQuestion(from: url)
        }
    }

    private func interruptAssistantAndRestart() {
        guard state == .assistantWorking else { return }
        assistantSpeech.stop()
        let currentTask = assistantWorkTask
        currentTask?.cancel()
        Task { @MainActor [weak self] in
            await currentTask?.value
            guard let self, self.state == .ready else { return }
            self.startAssistantRecording()
        }
    }

    private func answerAssistantQuestion(from url: URL) async {
        defer {
            cleanup(url)
            screenCaptureTask = nil
            assistantWorkTask = nil
            state = .ready
            rebuildMenu()
        }

        do {
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let question = result.text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !question.isEmpty else {
                throw Self.failure("Koett did not hear a question.")
            }

            do {
                try transcriptStore.append(
                    question,
                    model: "Parakeet v2 (Koett Ask)"
                )
            } catch {
                fputs(
                    "Warning: assistant transcript was not saved: \(error.localizedDescription)\n",
                    stderr
                )
            }

            guard let captureTask = screenCaptureTask else {
                throw Self.failure("The screen capture did not start.")
            }
            let screenshot = try await captureTask.value
            let configuration = try AssistantConfiguration.load(
                provider: assistantProvider
            )
            guard let apiKey = try AssistantAPIKeyStore.load(
                for: assistantProvider
            ) else {
                throw Self.failure(
                    "Set a \(assistantProvider.displayName) API key from the Koett menu."
                )
            }

            assistantPanel.beginAnswer(question: question)
            try await assistantClient.streamAnswer(
                question: question,
                screenshotJPEG: screenshot,
                apiKey: apiKey,
                configuration: configuration
            ) { [weak self] delta in
                self?.assistantPanel.appendAnswer(delta)
                await self?.assistantSpeech.appendResponseDelta(delta)
            }
            await assistantSpeech.finishResponse()
            assistantPanel.finishAnswer()
        } catch {
            if Task.isCancelled {
                return
            }
            assistantSpeech.stop()
            let message = Self.shortAssistantError(error)
            assistantPanel.showError(message)
            fputs("Error: \(error.localizedDescription)\n", stderr)
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
        url.scheme == "https" else { return }

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

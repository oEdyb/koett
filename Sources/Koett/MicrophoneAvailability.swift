import CoreAudio
import Foundation

extension KoettController {
    func startMicrophoneMonitoring() {
        if microphoneMonitor == nil {
            microphoneMonitor = MicrophoneAvailabilityMonitor { [weak self] in
                self?.refreshMicrophoneAvailability()
            }
        }
        do {
            try microphoneMonitor?.start()
        } catch {
            fputs("Warning: microphone monitoring could not start: \(error.localizedDescription)\n", stderr)
        }
        refreshMicrophoneAvailability()
    }

    func refreshMicrophoneAvailability() {
        do {
            let devices = try availableMicrophones()
            handleMicrophoneRemoval(in: devices)
            let resolved = try? reconcileMicrophoneSelection(in: devices)
            watchMicrophone(deviceID: recorder?.device.id ?? resolved?.id)
            rebuildMenu()
        } catch {
            fputs("Warning: microphones could not be refreshed: \(error.localizedDescription)\n", stderr)
        }
    }

    func refreshMicrophoneMonitorTarget() {
        guard microphoneMonitor != nil else { return }
        let id = recorder?.device.id ?? (try? reconcileMicrophoneSelection(in: availableMicrophones()))?.id
        watchMicrophone(deviceID: id)
    }

    func watchMicrophone(deviceID: AudioDeviceID?) {
        do {
            try microphoneMonitor?.watch(deviceID: deviceID)
        } catch {
            fputs("Warning: microphone monitoring could not follow the input: \(error.localizedDescription)\n", stderr)
        }
    }

    func reconcileMicrophoneSelection(in devices: [MicrophoneDevice]) throws -> MicrophoneDevice {
        let previous = microphoneSelection.uid
        defer {
            if microphoneSelection.uid != previous {
                microphoneSelection.save(to: microphoneDefaults)
            }
        }
        return try microphoneSelection.reconcile(in: devices, defaultID: (try? defaultMicrophoneID()) ?? 0)
    }

    private func handleMicrophoneRemoval(in devices: [MicrophoneDevice]) {
        guard state == .recording || state == .assistantRecording,
              let recorder, recorder.isRecording,
              !devices.contains(where: { $0.id == recorder.device.id && $0.uid == recorder.device.uid }) else { return }
        recorder.markUnavailable()
        stopAfterCaptureFailure()
    }

    /// Starts or ends the poll that notices a dead input while a session records.
    func refreshCaptureWatch() {
        guard state == .recording || state == .assistantRecording else {
            captureWatch?.invalidate()
            captureWatch = nil
            return
        }
        guard captureWatch == nil else { return }
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] timer in
            guard let self else { timer.invalidate(); return }
            MainActor.assumeIsolated { self.checkActiveCapture() }
        }
        RunLoop.main.add(timer, forMode: .common)
        captureWatch = timer
    }

    func checkActiveCapture() {
        guard state == .recording || state == .assistantRecording, let recorder else { return }
        // A failed Nemotron backup file must not end a healthy live transcript.
        guard recorder.captureHasFailed() || (nemotronRecorder == nil && recorder.fileHasFailed) else { return }
        stopAfterCaptureFailure()
    }

    private func stopAfterCaptureFailure() {
        nemotronRecorder?.markUnavailable()
        if state == .recording {
            play(stopSound)
            stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        } else {
            toggleAssistant()
        }
    }
}

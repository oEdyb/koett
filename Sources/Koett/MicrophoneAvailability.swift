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
        nemotronRecorder?.markUnavailable()
        if state == .recording {
            stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
        } else {
            toggleAssistant()
        }
    }
}

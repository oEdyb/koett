import AudioToolbox
import AVFoundation
import Foundation
import Synchronization

@MainActor
final class MicrophoneRecorder {
    private let input: any MicrophoneCapturing
    private let sink: MicrophoneFileSink
    private(set) var isRecording = false
    let device: MicrophoneDevice

    var currentTime: TimeInterval { sink.duration }
    var averagePower: Float { sink.averagePower }
    var recordingError: Error? { input.error ?? sink.error }

    convenience init(url: URL, device: MicrophoneDevice) throws {
        try self.init(url: url, device: device, input: MicrophoneInput(device: device))
    }

    init(url: URL, device: MicrophoneDevice, input: any MicrophoneCapturing) throws {
        self.device = device
        self.input = input
        sink = try MicrophoneFileSink(url: url, inputFormat: input.format)
        let sink = sink
        do {
            try input.start { buffer in sink.append(buffer) }
            isRecording = true
        } catch {
            input.stop()
            sink.close()
            throw error
        }
    }

    func stop() {
        guard isRecording else { return }
        input.stop()
        sink.close()
        isRecording = false
    }
}

final class MicrophoneFileSink: Sendable {
    // The file handle stays inside the mutex, including writes and disposal.
    private struct State: @unchecked Sendable {
        var file: ExtAudioFileRef?
        var frames: Int64 = 0
        var power: Float = -160
        var error: Error?
    }

    private let state: Mutex<State>
    private let sampleRate: Double
    private let droppedFrames = Atomic<Int>(0)

    var duration: TimeInterval { state.withLock { Double($0.frames) / sampleRate } }
    var averagePower: Float { state.withLock { $0.power } }
    var error: Error? {
        if droppedFrames.load(ordering: .acquiring) > 0 {
            return MicrophoneDevices.failure("Microphone audio could not be saved fast enough.")
        }
        return state.withLock { $0.error }
    }

    init(url: URL, inputFormat: AVAudioFormat) throws {
        guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0,
              inputFormat.commonFormat == .pcmFormatFloat32,
              let output = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1) else {
            throw MicrophoneDevices.failure("The microphone has no supported audio format.")
        }
        sampleRate = inputFormat.sampleRate
        var file: ExtAudioFileRef?
        var outputDescription = output.streamDescription.pointee
        try Self.check(ExtAudioFileCreateWithURL(url as CFURL, kAudioFileWAVEType, &outputDescription, nil, AudioFileFlags.eraseFile.rawValue, &file))
        guard let file else { throw MicrophoneDevices.failure("The recording file could not be created.") }
        do {
            var client = inputFormat.streamDescription.pointee
            try Self.check(ExtAudioFileSetProperty(file, kExtAudioFileProperty_ClientDataFormat, UInt32(MemoryLayout<AudioStreamBasicDescription>.size), &client))
            try Self.check(ExtAudioFileWriteAsync(file, 0, nil))
        } catch {
            ExtAudioFileDispose(file)
            throw error
        }
        state = Mutex(State(file: file))
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        let accepted = state.withLockIfAvailable { state -> Bool in
            guard let file = state.file, state.error == nil else { return true }
            do {
                try Self.check(ExtAudioFileWriteAsync(file, buffer.frameLength, buffer.audioBufferList))
                state.frames += Int64(buffer.frameLength)
                if let samples = buffer.floatChannelData?[0], buffer.frameLength > 0 {
                    var sum: Float = 0
                    for frame in 0..<Int(buffer.frameLength) {
                        let sample = samples[frame * buffer.stride]
                        sum += sample * sample
                    }
                    state.power = max(-160, 10 * log10(max(sum / Float(buffer.frameLength), 1e-16)))
                }
            } catch {
                state.error = error
            }
            return true
        } ?? false
        if !accepted {
            droppedFrames.wrappingAdd(Int(buffer.frameLength), ordering: .relaxed)
        }
    }

    func fail(_ message: String) {
        state.withLock { state in
            if state.error == nil { state.error = MicrophoneDevices.failure(message) }
        }
    }

    func close() {
        state.withLock { state in
            guard let file = state.file else { return }
            let status = ExtAudioFileDispose(file)
            state.file = nil
            if status != noErr, state.error == nil {
                state.error = MicrophoneDevices.failure("The recording could not be saved (\(status)).")
            }
        }
    }

    deinit { close() }

    private static func check(_ status: OSStatus) throws {
        guard status == noErr else {
            throw MicrophoneDevices.failure("The microphone recording failed (\(status)).")
        }
    }
}

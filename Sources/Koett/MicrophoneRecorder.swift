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
    private let clock: () -> TimeInterval
    private let startedAt: TimeInterval
    private var everPaused = false
    private var lastFrames = 0
    private var lastProgressAt: TimeInterval?
    private var disconnectedSince: TimeInterval?

    /// A format or running-state change this long is not a brief reconfiguration.
    static let disconnectLimit: TimeInterval = 2
    /// Bluetooth inputs can take a few seconds to deliver their first audio.
    static let stallLimit: TimeInterval = 5

    var currentTime: TimeInterval { sink.duration }
    var averagePower: Float { sink.averagePower }
    /// While paused, microphone buffers are dropped before they reach the file.
    var isPaused: Bool {
        get { sink.isPaused }
        set {
            sink.isPaused = newValue
            if newValue { everPaused = true }
        }
    }
    var recordingError: Error? { input.error ?? sink.error }

    convenience init(url: URL, device: MicrophoneDevice) throws {
        try self.init(url: url, device: device, input: MicrophoneInput(device: device))
    }

    init(
        url: URL,
        device: MicrophoneDevice,
        input: any MicrophoneCapturing,
        clock: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    ) throws {
        self.device = device
        self.input = input
        self.clock = clock
        startedAt = clock()
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

    func markUnavailable() {
        sink.fail("Microphone disconnected · Recording stopped")
    }

    /// Polled while recording, so a dead input stops the session instead of the
    /// clock counting on. Reads atomics only: taking the sink's lock here could
    /// make the audio callback drop a buffer.
    func captureHasFailed() -> Bool {
        guard isRecording else { return false }
        if input.error != nil { return true }
        let now = clock()
        disconnectedSince = input.isConnected ? nil : disconnectedSince ?? now
        if let disconnectedSince, now - disconnectedSince >= Self.disconnectLimit {
            sink.fail(MicrophoneInput.stoppedMessage)
            return true
        }
        guard !isPaused else {
            lastProgressAt = nil
            return false
        }
        let frames = sink.frameCount
        if frames != lastFrames || lastProgressAt == nil {
            lastFrames = frames
            lastProgressAt = now
        }
        if let lastProgressAt, now - lastProgressAt >= Self.stallLimit {
            sink.fail(MicrophoneInput.stoppedMessage)
            return true
        }
        return false
    }

    var fileHasFailed: Bool { sink.hasFailed }

    /// True when the recording ran for a while without one frame arriving.
    var heardNothing: Bool {
        sink.frameCount == 0 && !everPaused && clock() - startedAt >= 1
    }
}

final class MicrophoneFileSink: Sendable {
    // The file handle stays inside the mutex, including writes and disposal.
    private struct State: @unchecked Sendable {
        var file: ExtAudioFileRef?
        var error: Error?
        /// Holds the mono mix of a multichannel input; only the audio callback writes it.
        var mix: AVAudioPCMBuffer?
    }

    // Meter reads use atomics so the overlay never holds the lock the audio callback needs.
    private let state: Mutex<State>
    private let sampleRate: Double
    private let frames = Atomic<Int>(0)
    private let powerBits = Atomic<UInt32>(Float(-160).bitPattern)
    private let droppedFrames = Atomic<Int>(0)
    private let paused = Atomic<Bool>(false)
    private let failed = Atomic<Bool>(false)
    static let writeBufferBytes: UInt32 = 4 << 20

    var isPaused: Bool {
        get { paused.load(ordering: .acquiring) }
        set {
            paused.store(newValue, ordering: .releasing)
            if newValue { powerBits.store(Float(-160).bitPattern, ordering: .relaxed) }
        }
    }

    var frameCount: Int { frames.load(ordering: .relaxed) }
    var duration: TimeInterval { Double(frames.load(ordering: .relaxed)) / sampleRate }
    /// Lock-free, unlike `error`, so it is safe to poll while audio is flowing.
    var hasFailed: Bool {
        failed.load(ordering: .acquiring) || droppedFrames.load(ordering: .acquiring) > 0
    }
    var averagePower: Float { Float(bitPattern: powerBits.load(ordering: .relaxed)) }
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
        // Core Audio's multichannel-to-mono conversion keeps only the first input,
        // so a microphone on another input would record silence. Mix it here.
        let clientFormat = inputFormat.channelCount == 1
            ? inputFormat
            : AVAudioFormat(standardFormatWithSampleRate: inputFormat.sampleRate, channels: 1)
        let mix = inputFormat.channelCount == 1 ? nil : clientFormat.flatMap {
            AVAudioPCMBuffer(pcmFormat: $0, frameCapacity: Self.mixFrames)
        }
        guard let clientFormat, inputFormat.channelCount == 1 || mix != nil else {
            throw MicrophoneDevices.failure("The microphone has no supported audio format.")
        }
        var file: ExtAudioFileRef?
        var outputDescription = output.streamDescription.pointee
        try Self.check(ExtAudioFileCreateWithURL(url as CFURL, kAudioFileWAVEType, &outputDescription, nil, AudioFileFlags.eraseFile.rawValue, &file))
        guard let file else { throw MicrophoneDevices.failure("The recording file could not be created.") }
        do {
            var client = clientFormat.streamDescription.pointee
            try Self.check(ExtAudioFileSetProperty(file, kExtAudioFileProperty_ClientDataFormat, UInt32(MemoryLayout<AudioStreamBasicDescription>.size), &client))
            // The default 64 KB buffer overflows when the writer thread stalls for about one second.
            var bufferBytes = Self.writeBufferBytes
            try Self.check(ExtAudioFileSetProperty(file, kExtAudioFileProperty_IOBufferSizeBytes, UInt32(MemoryLayout<UInt32>.size), &bufferBytes))
            try Self.check(ExtAudioFileWriteAsync(file, 0, nil))
        } catch {
            ExtAudioFileDispose(file)
            throw error
        }
        state = Mutex(State(file: file, mix: mix))
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        guard !paused.load(ordering: .acquiring) else { return }
        let accepted = state.withLockIfAvailable { state -> Bool in
            guard let file = state.file, state.error == nil else { return true }
            do {
                let meter: UnsafePointer<Float>?
                let meterFrames: Int
                if let mix = state.mix {
                    (meter, meterFrames) = try Self.writeMix(of: buffer, using: mix, to: file)
                } else {
                    try Self.check(ExtAudioFileWriteAsync(file, buffer.frameLength, buffer.audioBufferList))
                    meter = buffer.floatChannelData.map { UnsafePointer($0[0]) }
                    meterFrames = Int(buffer.frameLength)
                }
                frames.wrappingAdd(Int(buffer.frameLength), ordering: .relaxed)
                if let meter, meterFrames > 0 {
                    var sum: Float = 0
                    for frame in 0..<meterFrames {
                        sum += meter[frame] * meter[frame]
                    }
                    let power = max(-160, 10 * log10(max(sum / Float(meterFrames), 1e-16)))
                    powerBits.store(power.bitPattern, ordering: .relaxed)
                }
            } catch {
                state.error = error
                failed.store(true, ordering: .releasing)
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
        failed.store(true, ordering: .releasing)
    }

    func close() {
        state.withLock { state in
            guard let file = state.file else { return }
            let status = ExtAudioFileDispose(file)
            state.file = nil
            if status != noErr, state.error == nil {
                state.error = MicrophoneDevices.failure("The recording could not be saved (\(status)).")
                failed.store(true, ordering: .releasing)
            }
        }
    }

    deinit { close() }

    private static let mixFrames: AVAudioFrameCount = 16_384
    /// Below this power a channel is an unused input, left out so it can't halve the speech.
    private static let silentChannelPower: Float = 1e-8

    /// Averages the channels that carry signal and writes the mono result. Returns
    /// the last mixed block for the meter. Allocates nothing: this is the audio callback.
    private static func writeMix(
        of buffer: AVAudioPCMBuffer,
        using mix: AVAudioPCMBuffer,
        to file: ExtAudioFileRef
    ) throws -> (UnsafePointer<Float>?, Int) {
        guard let channels = buffer.floatChannelData, let output = mix.floatChannelData?[0] else { return (nil, 0) }
        let total = Int(buffer.frameLength)
        let channelCount = Int(buffer.format.channelCount)
        let capacity = Int(mix.frameCapacity)
        var offset = 0
        var written = 0
        while offset < total {
            let count = min(capacity, total - offset)
            output.initialize(repeating: 0, count: count)
            var live: Float = 0
            for channel in 0..<channelCount {
                let samples = channels[channel] + offset
                var sum: Float = 0
                for frame in 0..<count { sum += samples[frame] * samples[frame] }
                guard sum / Float(count) > silentChannelPower else { continue }
                live += 1
                for frame in 0..<count { output[frame] += samples[frame] }
            }
            if live > 1 {
                for frame in 0..<count { output[frame] /= live }
            }
            mix.frameLength = AVAudioFrameCount(count)
            try check(ExtAudioFileWriteAsync(file, mix.frameLength, mix.audioBufferList))
            offset += count
            written = count
        }
        return (UnsafePointer(output), written)
    }

    private static func check(_ status: OSStatus) throws {
        guard status == noErr else {
            throw MicrophoneDevices.failure("The microphone recording failed (\(status)).")
        }
    }
}

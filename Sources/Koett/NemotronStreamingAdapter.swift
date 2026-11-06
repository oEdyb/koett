@preconcurrency import AVFoundation
@preconcurrency import CoreML
import FluidAudio
import Foundation
import Synchronization

struct StreamingAudioChunk: Sendable {
    let samples: [Float]
    let sampleRate: Double

    func makeBuffer() throws -> AVAudioPCMBuffer {
        guard
            let format = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: sampleRate,
                channels: 1,
                interleaved: false
            ),
            let buffer = AVAudioPCMBuffer(
                pcmFormat: format,
                frameCapacity: AVAudioFrameCount(samples.count)
            )
        else {
            throw NSError(
                domain: "Koett",
                code: 4,
                userInfo: [NSLocalizedDescriptionKey: "An audio buffer could not be created."]
            )
        }

        buffer.frameLength = buffer.frameCapacity
        buffer.floatChannelData![0].update(from: samples, count: samples.count)
        return buffer
    }
}

protocol StreamingTranscriptionAdapter: Sendable {
    func prepare() async throws
    func append(_ chunk: StreamingAudioChunk) async throws
    func finish() async throws -> String
    func cancel() async
}

struct NemotronStreamingAdapter: StreamingTranscriptionAdapter {
    private let manager: StreamingNemotronAsrManager

    init() {
        let configuration = MLModelConfiguration()
        configuration.computeUnits = .cpuAndNeuralEngine
        manager = StreamingNemotronAsrManager(
            configuration: configuration,
            requestedChunkSize: .ms560
        )
    }

    func prepare() async throws {
        try await prepare(progressHandler: nil)
    }

    func prepare(progressHandler: ProgressHandler?) async throws {
        try await manager.loadModels(progressHandler: progressHandler)
        let loadedChunkMilliseconds = await manager.config.chunkMs
        guard loadedChunkMilliseconds == NemotronChunkSize.ms560.rawValue else {
            throw NSError(
                domain: "Koett",
                code: 5,
                userInfo: [
                    NSLocalizedDescriptionKey:
                        "Nemotron loaded the \(loadedChunkMilliseconds) ms model instead of 560 ms."
                ]
            )
        }
    }

    func append(_ chunk: StreamingAudioChunk) async throws {
        _ = try await manager.process(audioBuffer: chunk.makeBuffer())
    }

    func finish() async throws -> String {
        do {
            let text = try await manager.finish()
            await manager.reset()
            return text
        } catch {
            await manager.reset()
            throw error
        }
    }

    func cancel() async {
        await manager.reset()
    }
}

final class LiveAudioStore: Sendable {
    struct Read: Sendable {
        let chunk: StreamingAudioChunk?
        let nextFrame: Int
        let isFinished: Bool
    }

    private final class Chunk: @unchecked Sendable {
        let samples: UnsafeMutablePointer<Float>
        init(frames: Int) {
            samples = .allocate(capacity: frames)
            samples.initialize(repeating: 0, count: frames)
        }
        deinit { samples.deallocate() }
    }

    private struct State {
        var chunks: [Chunk]
        var frameCount = 0
        var isFinished = false
    }

    let sampleRate: Double
    private let state: Mutex<State>
    private let maximumFrames: Int
    private let chunkFrames: Int
    private let capturedFrameCount = Atomic<Int>(0)
    private let droppedFrameCount = Atomic<Int>(0)
    private let paused = Atomic<Bool>(false)
    var isPaused: Bool {
        get { paused.load(ordering: .acquiring) }
        set { paused.store(newValue, ordering: .releasing) }
    }

    init(sampleRate: Double, maximumDuration: TimeInterval) {
        self.sampleRate = sampleRate
        maximumFrames = max(0, Int(ceil(sampleRate * maximumDuration)))
        chunkFrames = max(1, min(maximumFrames, Int(ceil(sampleRate * 5))))
        state = Mutex(State(chunks: maximumFrames > 0 ? [Chunk(frames: chunkFrames)] : []))
    }

    var allocatedFrameCapacity: Int { state.withLock { $0.chunks.count * chunkFrames } }

    // Only the consumer allocates ahead; the audio callback never grows storage.
    private func prepareCapacity() {
        let needed = state.withLock { state -> Int in
            guard !state.isFinished else { return 0 }
            let frames = min(maximumFrames, state.frameCount + chunkFrames * 2)
            return max(0, (frames + chunkFrames - 1) / chunkFrames - state.chunks.count)
        }
        guard needed > 0 else { return }
        let chunks = (0..<needed).map { _ in Chunk(frames: chunkFrames) }
        state.withLock { state in
            guard !state.isFinished else { return }
            state.chunks.append(contentsOf: chunks)
        }
    }

    var duration: TimeInterval { state.withLock { Double($0.frameCount) / sampleRate } }
    var capturedDuration: TimeInterval { Double(capturedFrameCount.load(ordering: .acquiring)) / sampleRate }

    func reset() {
        state.withLock { state in
            state.frameCount = 0
            state.isFinished = false
        }
        capturedFrameCount.store(0, ordering: .releasing)
        droppedFrameCount.store(0, ordering: .releasing)
        isPaused = false
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        guard !isPaused else { return }
        let requestedFrames = Int(buffer.frameLength)
        guard requestedFrames > 0,
              let input = buffer.floatChannelData else {
            return
        }

        _ = capturedFrameCount.wrappingAdd(requestedFrames, ordering: .releasing)
        let channelCount = Int(buffer.format.channelCount)
        let copiedFrames = state.withLockIfAvailable { state in
            guard !state.isFinished else { return 0 }
            let availableFrames = min(maximumFrames, state.chunks.count * chunkFrames) - state.frameCount
            let framesToCopy = min(requestedFrames, availableFrames)
            guard framesToCopy > 0 else { return 0 }

            if channelCount == 1 {
                var offset = 0
                while offset < framesToCopy {
                    let frame = state.frameCount + offset
                    let count = min(framesToCopy - offset, chunkFrames - frame % chunkFrames)
                    state.chunks[frame / chunkFrames].samples.advanced(by: frame % chunkFrames)
                        .update(from: input[0].advanced(by: offset), count: count)
                    offset += count
                }
            } else {
                for frame in 0..<framesToCopy {
                    state.chunks[(state.frameCount + frame) / chunkFrames].samples[(state.frameCount + frame) % chunkFrames] = 0
                }
                var liveChannels: Float = 0
                for channel in 0..<channelCount {
                    var power: Float = 0
                    for frame in 0..<framesToCopy { power += input[channel][frame] * input[channel][frame] }
                    guard power / Float(framesToCopy) > 1e-8 else { continue }
                    liveChannels += 1
                    for frame in 0..<framesToCopy {
                        state.chunks[(state.frameCount + frame) / chunkFrames].samples[(state.frameCount + frame) % chunkFrames] += input[channel][frame]
                    }
                }
                if liveChannels > 1 {
                    for frame in 0..<framesToCopy {
                        state.chunks[(state.frameCount + frame) / chunkFrames].samples[(state.frameCount + frame) % chunkFrames] /= liveChannels
                    }
                }
            }

            state.frameCount += framesToCopy
            return framesToCopy
        } ?? 0

        if copiedFrames < requestedFrames {
            _ = droppedFrameCount.wrappingAdd(
                requestedFrames - copiedFrames,
                ordering: .relaxed
            )
        }
    }

    func finish() {
        state.withLock { $0.isFinished = true }
    }

    func read(from startFrame: Int, maximumFrames: Int) -> Read {
        prepareCapacity()
        return state.withLock { state in
            let endFrame = min(state.frameCount, startFrame + maximumFrames)
            let chunk: StreamingAudioChunk?
            if endFrame > startFrame {
                var samples = [Float](repeating: 0, count: endFrame - startFrame)
                samples.withUnsafeMutableBufferPointer { destination in
                    var frame = startFrame
                    while frame < endFrame {
                        let count = min(endFrame - frame, chunkFrames - frame % chunkFrames)
                        destination.baseAddress!.advanced(by: frame - startFrame).update(
                            from: state.chunks[frame / chunkFrames].samples.advanced(by: frame % chunkFrames), count: count)
                        frame += count
                    }
                }
                chunk = StreamingAudioChunk(samples: samples, sampleRate: sampleRate)
            } else {
                chunk = nil
            }

            return Read(
                chunk: chunk,
                nextFrame: endFrame,
                isFinished: state.isFinished && endFrame == state.frameCount
            )
        }
    }

    var droppedFrames: Int {
        droppedFrameCount.load(ordering: .acquiring)
    }
}

@MainActor
final class NemotronLiveRecorder {
    struct PendingCapture: Sendable {
        let store: LiveAudioStore
        let processingTask: Task<Void, Error>
        let error: Error?
    }

    var currentTime: TimeInterval { store.capturedDuration }

    private let adapter: any StreamingTranscriptionAdapter
    private let input: any MicrophoneCapturing
    private let inputFormat: AVAudioFormat
    private let store: LiveAudioStore
    private var processingTask: Task<Void, Error>?
    private var isRecording = false
    private var captureError: Error?
    var isPaused: Bool {
        get { store.isPaused }
        set { store.isPaused = newValue }
    }

    convenience init(
        adapter: any StreamingTranscriptionAdapter,
        device: MicrophoneDevice,
        maximumDuration: TimeInterval = RecordingLimit.defaultLimit.seconds
    ) throws {
        try self.init(adapter: adapter, input: MicrophoneInput(device: device), maximumDuration: maximumDuration)
    }

    init(
        adapter: any StreamingTranscriptionAdapter,
        input: any MicrophoneCapturing,
        maximumDuration: TimeInterval = RecordingLimit.defaultLimit.seconds
    ) throws {
        self.adapter = adapter
        self.input = input
        let format = input.format
        guard format.sampleRate > 0, format.channelCount > 0 else {
            throw NSError(
                domain: "Koett",
                code: 6,
                userInfo: [NSLocalizedDescriptionKey: "No valid microphone input format is available."]
            )
        }
        guard format.commonFormat == .pcmFormatFloat32, !format.isInterleaved else {
            throw NSError(
                domain: "Koett",
                code: 7,
                userInfo: [NSLocalizedDescriptionKey: "Koett needs non-interleaved Float32 microphone audio."]
            )
        }

        inputFormat = format
        store = LiveAudioStore(
            sampleRate: format.sampleRate,
            maximumDuration: maximumDuration
        )
    }

    func start() throws {
        guard processingTask == nil, !isRecording else {
            throw NSError(
                domain: "Koett",
                code: 8,
                userInfo: [NSLocalizedDescriptionKey: "Nemotron is already recording."]
            )
        }

        store.reset()
        captureError = nil
        let framesPerRead = max(1, Int(inputFormat.sampleRate / 10))
        let task = Task { @concurrent [adapter, store] in
            try await Self.consume(
                store: store,
                adapter: adapter,
                maximumFrames: framesPerRead
            )
        }
        processingTask = task

        isRecording = true

        do {
            try input.start(onAudio: Self.makeTap(store: store))
        } catch {
            input.stop()
            isRecording = false
            store.finish()
            task.cancel()
            processingTask = nil
            throw error
        }
    }

    func stop() throws -> PendingCapture {
        guard let processingTask, isRecording else {
            throw NSError(
                domain: "Koett",
                code: 9,
                userInfo: [NSLocalizedDescriptionKey: "Nemotron was not recording."]
            )
        }

        isRecording = false
        input.stop()
        store.finish()
        self.processingTask = nil

        return PendingCapture(store: store, processingTask: processingTask, error: captureError ?? input.error)
    }

    func markUnavailable() {
        captureError = MicrophoneDevices.failure("Microphone disconnected · Recording stopped")
    }

    isolated deinit {
        input.stop()
        store.finish()
        processingTask?.cancel()
    }

    nonisolated private static func consume(
        store: LiveAudioStore,
        adapter: any StreamingTranscriptionAdapter,
        maximumFrames: Int
    ) async throws {
        var nextFrame = 0
        while true {
            try Task.checkCancellation()
            let read = store.read(from: nextFrame, maximumFrames: maximumFrames)
            if let chunk = read.chunk {
                try await adapter.append(chunk)
                nextFrame = read.nextFrame
            }
            if read.isFinished {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    nonisolated private static func makeTap(
        store: LiveAudioStore
    ) -> @Sendable (AVAudioPCMBuffer) -> Void {
        { buffer in
            store.append(buffer)
        }
    }
}

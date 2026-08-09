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
        try await manager.loadModels()
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

    private struct State {
        var samples: [Float]
        var frameCount = 0
        var isFinished = false
    }

    let sampleRate: Double
    private let state: Mutex<State>
    private let droppedFrameCount = Atomic<Int>(0)

    init(sampleRate: Double, maximumDuration: TimeInterval) {
        self.sampleRate = sampleRate
        state = Mutex(State(
            samples: Array(repeating: 0, count: Int(ceil(sampleRate * maximumDuration)))
        ))
    }

    func reset() {
        state.withLock { state in
            state.frameCount = 0
            state.isFinished = false
        }
        droppedFrameCount.store(0, ordering: .releasing)
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        let requestedFrames = Int(buffer.frameLength)
        guard requestedFrames > 0,
              let input = buffer.floatChannelData else {
            return
        }

        let channelCount = Int(buffer.format.channelCount)
        let copiedFrames = state.withLockIfAvailable { state in
            guard !state.isFinished else { return 0 }
            let availableFrames = state.samples.count - state.frameCount
            let framesToCopy = min(requestedFrames, availableFrames)
            guard framesToCopy > 0 else { return 0 }

            if channelCount == 1 {
                state.samples.withUnsafeMutableBufferPointer { destination in
                    destination.baseAddress!
                        .advanced(by: state.frameCount)
                        .update(from: input[0], count: framesToCopy)
                }
            } else {
                for frame in 0..<framesToCopy {
                    var mixedSample: Float = 0
                    for channel in 0..<channelCount {
                        mixedSample += input[channel][frame]
                    }
                    state.samples[state.frameCount + frame] = mixedSample / Float(channelCount)
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
        state.withLock { state in
            let endFrame = min(state.frameCount, startFrame + maximumFrames)
            let chunk: StreamingAudioChunk?
            if endFrame > startFrame {
                chunk = StreamingAudioChunk(
                    samples: Array(state.samples[startFrame..<endFrame]),
                    sampleRate: sampleRate
                )
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
    }

    private let adapter: any StreamingTranscriptionAdapter
    private let audioEngine = AVAudioEngine()
    private let inputFormat: AVAudioFormat
    private let store: LiveAudioStore
    private var processingTask: Task<Void, Error>?
    private var tapInstalled = false

    init(
        adapter: any StreamingTranscriptionAdapter,
        maximumDuration: TimeInterval = 600
    ) throws {
        self.adapter = adapter
        let format = audioEngine.inputNode.outputFormat(forBus: 0)
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
        audioEngine.prepare()
    }

    func start() throws {
        guard processingTask == nil, !tapInstalled else {
            throw NSError(
                domain: "Koett",
                code: 8,
                userInfo: [NSLocalizedDescriptionKey: "Nemotron is already recording."]
            )
        }

        store.reset()
        let framesPerRead = max(1, Int(inputFormat.sampleRate / 10))
        let task = Task { @concurrent [adapter, store] in
            try await Self.consume(
                store: store,
                adapter: adapter,
                maximumFrames: framesPerRead
            )
        }
        processingTask = task

        let inputNode = audioEngine.inputNode
        inputNode.installTap(
            onBus: 0,
            bufferSize: 1_024,
            format: inputFormat,
            block: Self.makeTap(store: store)
        )
        tapInstalled = true

        do {
            try audioEngine.start()
        } catch {
            inputNode.removeTap(onBus: 0)
            tapInstalled = false
            store.finish()
            task.cancel()
            processingTask = nil
            throw error
        }
    }

    func stop() throws -> PendingCapture {
        guard let processingTask, tapInstalled else {
            throw NSError(
                domain: "Koett",
                code: 9,
                userInfo: [NSLocalizedDescriptionKey: "Nemotron was not recording."]
            )
        }

        audioEngine.inputNode.removeTap(onBus: 0)
        tapInstalled = false
        audioEngine.stop()
        store.finish()
        self.processingTask = nil

        return PendingCapture(store: store, processingTask: processingTask)
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
    ) -> AVAudioNodeTapBlock {
        { buffer, _ in
            store.append(buffer)
        }
    }
}

import CoreML
import Darwin
import FluidAudio
import Foundation
import MeetingASRProtocol

@main
struct MeetingWorker {
    static func main() async {
        let args = Array(CommandLine.arguments.dropFirst())
        guard args.count == 4, let mode = MeetingASRMode(rawValue: args[0]),
              let parent = Int32(args[3]), parent > 1, getppid() == parent else { exit(2) }
        let watch = DispatchSource.makeProcessSource(identifier: parent, eventMask: .exit, queue: .global(qos: .utility))
        watch.setEventHandler { _exit(1) }
        watch.activate()
        guard getppid() == parent else { exit(2) }
        _ = setpriority(PRIO_DARWIN_PROCESS, 0, PRIO_DARWIN_BG)
        let output = FileHandle(fileDescriptor: dup(STDOUT_FILENO), closeOnDealloc: true)
        dup2(STDERR_FILENO, STDOUT_FILENO)
        let root = URL(fileURLWithPath: args[1], isDirectory: true)
        do {
            let engines = try Engines(root: root, library: args[2], mode: mode)
            while let request = try MeetingASRWire.read(MeetingASRRequest.self, from: .standardInput) {
                var response: MeetingASRResponse
                do { response = try await engines.run(request) }
                catch { response = .init(issue: error.localizedDescription) }
                response.peakBytes = peakFootprint()
                try MeetingASRWire.write(response, to: output)
            }
        } catch {
            fputs("Meeting worker: \(error.localizedDescription)\n", stderr)
            exit(1)
        }
        watch.cancel()
    }

    private static func peakFootprint() -> UInt64 {
        var usage = rusage_info_v4()
        let status = withUnsafeMutablePointer(to: &usage) {
            $0.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
                proc_pid_rusage(getpid(), RUSAGE_INFO_V4, $0)
            }
        }
        return status == 0 ? usage.ri_lifetime_max_phys_footprint : 0
    }
}

private final class Engines {
    private let root: URL
    private let mode: MeetingASRMode
    private let pins: [MeetingModelPins.Pin]
    private let library: String
    private var whisper: WhisperBridge?
    private var parakeet: AsrManager?
    private var primaryProblem: String?

    init(root: URL, library: String, mode: MeetingASRMode) throws {
        self.root = root
        self.library = library
        self.mode = mode
        pins = try MeetingModelPins.load()
    }

    func run(_ request: MeetingASRRequest) async throws -> MeetingASRResponse {
        let count = request.pcm.count / 2
        guard request.pcm.count.isMultiple(of: 2), (1...MeetingASRRequest.maxFrames).contains(count),
              0 <= request.ownershipStart, request.ownershipStart < request.ownershipEnd, request.ownershipEnd <= count
        else { throw MeetingASRError.protocolError }
        let samples: [Float] = request.pcm.withUnsafeBytes { bytes in
            (0..<count).map { Float(Int16(littleEndian: bytes.loadUnaligned(fromByteOffset: $0 * 2, as: Int16.self))) / 32_768 }
        }
        var speech: [Range<Int>]?
        if primaryProblem == nil {
            do {
                let bridge = try runtime()
                speech = try bridge.speech(samples) { try modelPath(MeetingModelPins.vad) }
                if mode != .parakeet {
                    let turns = try whisperTurns(bridge, samples, speech ?? [], request)
                    return .init(turns: turns)
                }
            } catch { primaryProblem = error.localizedDescription }
        }
        do {
            let turns = try await parakeetTurns(samples, speech ?? energySpeech(samples), request)
            return .init(turns: turns, fallback: mode != .parakeet)
        } catch {
            let message = [primaryProblem, error.localizedDescription].compactMap { $0 }.joined(separator: " ")
            throw MeetingASRError.inference(message)
        }
    }

    private func runtime() throws -> WhisperBridge {
        if let whisper { return whisper }
        let runtime = try WhisperBridge(library: library)
        whisper = runtime
        return runtime
    }

    private func modelPath(_ name: String) throws -> URL {
        guard let pin = pins.first(where: { $0.relative == name }) else { throw MeetingASRError.model(name) }
        return try pin.verify(in: root)
    }

    private func whisperTurns(_ bridge: WhisperBridge, _ samples: [Float], _ spans: [Range<Int>],
                              _ request: MeetingASRRequest) throws -> [MeetingASRTurn] {
        guard !spans.isEmpty else { return [] }
        let turbo = try bridge.model(MeetingModelPins.turbo) { try modelPath(MeetingModelPins.turbo) }
        var turns: [MeetingASRTurn] = []
        for (index, speech) in spans.enumerated() {
            let window = padded(speech, count: samples.count)
            let audio = Array(samples[window])
            var (language, confidence) = try bridge.language(turbo, audio)
            if speech.count < 48_000, confidence < 0.99 {
                let extra = context(index, spans, samples, window)
                if extra.count > audio.count { (language, confidence) = try bridge.language(turbo, extra) }
            }
            let swedish = mode == .routed && language == "sv"
            let model = swedish
                ? try bridge.model(MeetingModelPins.swedish) { try modelPath(MeetingModelPins.swedish) }
                : turbo
            let words = try bridge.words(model, audio, language)
            if let turn = turn(words, window: window, speech: speech, request: request, language: language,
                               backend: swedish ? "kb-whisper-large-q5_0" : "whisper-large-v3-turbo-q5_0") {
                turns.append(turn)
            }
        }
        return turns
    }

    private func context(_ index: Int, _ spans: [Range<Int>], _ samples: [Float], _ window: Range<Int>) -> [Float] {
        var remaining = 160_000 - window.count
        var parts = [index: window]
        let neighbors = spans.indices.filter { $0 != index }.sorted { (abs($0 - index), $0) < (abs($1 - index), $1) }
        for neighbor in neighbors where remaining > 0 {
            let speech = spans[neighbor]
            let count = min(remaining, speech.count)
            parts[neighbor] = neighbor < index
                ? (speech.upperBound - count)..<speech.upperBound
                : speech.lowerBound..<(speech.lowerBound + count)
            remaining -= count
        }
        return parts.keys.sorted().flatMap { key in parts[key].map { samples[$0] } ?? [] }
    }

    private func parakeetTurns(_ samples: [Float], _ spans: [Range<Int>],
                              _ request: MeetingASRRequest) async throws -> [MeetingASRTurn] {
        guard !spans.isEmpty else { return [] }
        let manager = try await parakeetManager()
        var turns: [MeetingASRTurn] = []
        for speech in spans {
            let window = padded(speech, count: samples.count)
            var decoder = TdtDecoderState.make(decoderLayers: 2)
            let result = try await manager.transcribe(Array(samples[window]), decoderState: &decoder)
            let words = buildWordTimings(from: result.tokenTimings ?? []).map {
                Word(text: $0.word, first: Int($0.startTime * 16_000), end: Int($0.endTime * 16_000))
            }
            if let turn = turn(words, window: window, speech: speech, request: request,
                               language: nil, backend: "parakeet-v3-int8") {
                turns.append(turn)
            }
        }
        return turns
    }

    private func parakeetManager() async throws -> AsrManager {
        if let parakeet { return parakeet }
        let name = MeetingModelPins.parakeet
        for pin in pins where pin.relative.hasPrefix(name + "/") { _ = try pin.verify(in: root) }

        let models = root.appending(path: name)
        let configuration = MLModelConfiguration()
        configuration.computeUnits = .cpuAndNeuralEngine
        let cpu = MLModelConfiguration()
        cpu.computeUnits = .cpuOnly
        let encoder = try MLModel(contentsOf: models.appending(path: "Encoder.mlmodelc"), configuration: configuration)
        let preprocessor = try MLModel(contentsOf: models.appending(path: "Preprocessor.mlmodelc"), configuration: cpu)
        let decoder = try MLModel(contentsOf: models.appending(path: "Decoder.mlmodelc"), configuration: configuration)
        let joint = try MLModel(contentsOf: models.appending(path: "JointDecisionv3.mlmodelc"), configuration: configuration)
        let data = try Data(contentsOf: models.appending(path: "parakeet_v3_vocab.json"))
        let vocabulary = try JSONDecoder().decode([Int: String].self, from: data)
        let loaded = AsrModels(encoder: encoder, preprocessor: preprocessor, decoder: decoder, joint: joint,
                                         configuration: configuration, vocabulary: vocabulary, version: .v3)
        let manager = AsrManager(config: .init(parallelChunkConcurrency: 1))
        try await manager.loadModels(loaded)
        parakeet = manager
        return manager
    }

    private func padded(_ speech: Range<Int>, count: Int) -> Range<Int> {
        max(0, speech.lowerBound - 4_800)..<min(count, speech.upperBound + 4_800)
    }

    private func turn(_ words: [Word], window: Range<Int>, speech: Range<Int>, request: MeetingASRRequest,
                      language: String?, backend: String) -> MeetingASRTurn? {
        let owned = words.compactMap { word -> Word? in
            let start = window.lowerBound + min(window.count, max(0, word.first))
            let end = window.lowerBound + min(window.count, max(word.first, word.end, 0))
            let midpoint = start + (end - start) / 2
            guard midpoint >= request.ownershipStart, midpoint < request.ownershipEnd,
                  padded(speech, count: window.upperBound).contains(midpoint) else { return nil }
            return .init(text: word.text, first: start, end: end)
        }
        guard let first = owned.first, let last = owned.last else { return nil }
        return .init(first: first.first, end: max(first.first, last.end), text: owned.map(\.text).joined(separator: " "),
                     language: language, backend: backend)
    }

    private func energySpeech(_ audio: [Float]) -> [Range<Int>] {
        let chunk = 320
        var spans: [Range<Int>] = []
        var first: Int?
        var last = 0
        for start in stride(from: 0, to: audio.count, by: chunk) {
            let end = min(audio.count, start + chunk)
            let slice = audio[start..<end]
            if slice.reduce(Float(0), { $0 + $1 * $1 }) / Float(slice.count) >= 0.00002 {
                if first == nil { first = start }
                last = end
            }
            if let beginning = first, start - last >= 8_000 || end - beginning >= 320_000 {
                spans.append(beginning..<last)
                first = nil
            }
        }
        if let first { spans.append(first..<last) }
        return spans
    }
}

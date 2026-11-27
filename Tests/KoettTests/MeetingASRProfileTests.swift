import CoreML
import FluidAudio
import Foundation
import MeetingASRProtocol
import XCTest
@testable import Koett

final class MeetingASRProfileTests: XCTestCase {
    func testBilingualTwoTrackProfile() async throws {
        let environment = ProcessInfo.processInfo.environment
        guard let path = environment["KOETT_MEETING_PROFILE_MODELS"],
              let worker = environment["KOETT_MEETING_PROFILE_WORKER"],
              let library = environment["KOETT_MEETING_PROFILE_LIBRARY"]
        else { throw XCTSkip("Set KOETT_MEETING_PROFILE_MODELS, KOETT_MEETING_PROFILE_WORKER and KOETT_MEETING_PROFILE_LIBRARY") }
        let root = URL(fileURLWithPath: path)
        let temporary = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: temporary) }
        let (tracks, utterances, dictation) = try fixture(temporary)
        let meetingSeconds = Double(tracks.map(\.count).max() ?? 0) / 16_000

        let config = MLModelConfiguration()
        config.computeUnits = .cpuAndNeuralEngine
        let cpu = MLModelConfiguration()
        cpu.computeUnits = .cpuOnly
        let v2 = root.appending(path: "parakeet-tdt-0.6b-v2")
        let vocabulary = try JSONDecoder().decode([Int: String].self, from: Data(contentsOf: v2.appending(path: "parakeet_vocab.json")))
        let manager = AsrManager(config: .init(parallelChunkConcurrency: 1))
        let loaded = try AsrModels(
            encoder: MLModel(contentsOf: v2.appending(path: "Encoder.mlmodelc"), configuration: config),
            preprocessor: MLModel(contentsOf: v2.appending(path: "Preprocessor.mlmodelc"), configuration: cpu),
            decoder: MLModel(contentsOf: v2.appending(path: "Decoder.mlmodelc"), configuration: config),
            joint: MLModel(contentsOf: v2.appending(path: "JointDecision.mlmodelc"), configuration: config),
            configuration: config, vocabulary: vocabulary, version: .v2
        )
        try await manager.loadModels(loaded)
        _ = try await DictationTranscriber.transcribe(dictation, using: manager)
        let baseline = try await measureDictation(dictation, manager: manager)
        print("MEETING_PROFILE fixture \(meetingSeconds)s, baseline dictation \(baseline) ms")
        for mode in MeetingASRMode.allCases {
            let client = try MeetingWorkerClient(executable: URL(fileURLWithPath: worker), models: root,
                                                 library: URL(fileURLWithPath: library), mode: mode, networkDenied: true)
            defer { client.cancel() }
            let requests = tracks.map { MeetingASRRequest(pcm: $0, ownershipStart: 0, ownershipEnd: $0.count) }
            let cold = ContinuousClock.now
            for request in requests {
                let warm = try client.decode(request)
                XCTAssertFalse(warm.fallback)
                XCTAssertFalse(warm.turns.isEmpty)
            }
            let coldMilliseconds = ms(since: cold)
            let started = ContinuousClock.now
            let concurrent = Task.detached(priority: .background) {
                try requests.map(client.decode)
            }
            try await Task.sleep(for: .milliseconds(50))
            let pressure = memoryPressure()
            let during = try await measureDictation(dictation, manager: manager)
            let replies = try await concurrent.value
            let warmMilliseconds = ms(since: started)
            let samples = replies.enumerated().flatMap { track, reply in
                reply.turns.map { (track, $0) }
            }
            let peak = replies.map(\.peakBytes).max() ?? 0
            XCTAssertGreaterThan(peak, 0)
            var wordErrors = ["sv": 0, "en": 0]
            var wordCounts = ["sv": 0, "en": 0]
            var languageErrors = 0
            var billed = Set<Int>()
            for entry in utterances {
                let matches = samples.enumerated().filter { _, sample in
                    guard sample.0 == entry.track else { return false }
                    let center = sample.1.first + (sample.1.end - sample.1.first) / 2
                    return entry.range.contains(center)
                }
                let hypothesis = matches.map { $0.element.1.text }.joined(separator: " ")
                for (index, _) in matches { billed.insert(index) }
                let reference = words(entry.reference)
                wordCounts[entry.language, default: 0] += reference.count
                wordErrors[entry.language, default: 0] += distance(reference, words(hypothesis))
                if mode != .parakeet, matches.first?.element.1.language != entry.language { languageErrors += 1 }
                print("MEETING_PROFILE \(mode.rawValue) \(entry.language) [\(matches.first?.element.1.language ?? "–")] \(entry.reference) => \(hypothesis)")
            }
            let insertions = samples.enumerated().filter { !billed.contains($0.offset) }.reduce(0) { $0 + words($1.element.1.text).count }
            print("MEETING_PROFILE \(mode.rawValue) sv \(wordErrors["sv"] ?? 0)/\(wordCounts["sv"] ?? 0), en \(wordErrors["en"] ?? 0)/\(wordCounts["en"] ?? 0), unassigned insertions \(insertions), language errors \(mode == .parakeet ? "unavailable" : String(languageErrors)), cold ms \(coldMilliseconds), warm s/min \(warmMilliseconds / 1000 * 60 / meetingSeconds), peak bytes \(peak), dictation ms \(during), pressure \(pressure)")
        }
    }

    private struct Utterance {
        let track: Int
        let language: String
        let reference: String
        let range: Range<Int>
    }

    private func fixture(_ root: URL) throws -> ([[Int16]], [Utterance], URL) {
        let lines: [(Int, String, Int, String)] = [
            (0, "sv", 1, "Vi ska inte boka resan på fredag. Be Margareta ringa på måndag."),
            (1, "en", 3, "Do not send the invoice until Eleanor approves the new price."),
            (0, "en", 9, "We need the blue folder before the next meeting."),
            (1, "sv", 11, "Nej, det stämmer inte."),
            (0, "sv", 15, "Tack så mycket."),
            (1, "en", 17, "Hold on, please."),
            (0, "sv", 21, "Ta inte bort Åsas namn från listan. Det behövs när vi öppnar kontoret."),
            (1, "sv", 23, "Kan Oskar flytta samtalet till tisdag eller torsdag?")
        ]
        var tracks = Array(repeating: Array(repeating: Int16(0), count: 29 * 16_000), count: 2)
        var utterances: [Utterance] = []
        for (index, line) in lines.enumerated() {
            let aiff = root.appending(path: "\(index).aiff")
            let wave = root.appending(path: "\(index).wav")
            try run("/usr/bin/say", ["-v", line.1 == "sv" ? "Alva" : "Samantha", "-o", aiff.path, line.3])
            try run("/usr/bin/afconvert", ["-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff.path, wave.path])
            let pcm = try XCTUnwrap(DictationTranscriber.loadSamples(wave))
            let first = line.2 * 16_000
            let end = first + pcm.count
            XCTAssertLessThanOrEqual(end, tracks[line.0].count)
            tracks[line.0].replaceSubrange(first..<end, with: pcm.map { Int16(clamping: Int($0 * 32768)) })
            utterances.append(.init(track: line.0, language: line.1, reference: line.3, range: (first - 2_000)..<(end + 2_000)))
        }
        return (tracks, utterances, root.appending(path: "1.wav"))
    }

    private func measureDictation(_ url: URL, manager: AsrManager) async throws -> Double {
        let point = ContinuousClock.now
        let result = try await DictationTranscriber.transcribe(url, using: manager)
        XCTAssertFalse(result.text.isEmpty)
        return ms(since: point)
    }

    private func ms(since point: ContinuousClock.Instant) -> Double {
        let duration = point.duration(to: .now).components
        return Double(duration.seconds) * 1_000 + Double(duration.attoseconds) / 1e15
    }

    private func memoryPressure() -> Int32 {
        var level: Int32 = -1
        var size = MemoryLayout<Int32>.size
        _ = sysctlbyname("kern.memorystatus_vm_pressure_level", &level, &size, nil, 0)
        return level
    }

    private func words(_ value: String) -> [String] {
        value.lowercased().split { !$0.isLetter && !$0.isNumber }.map(String.init)
    }

    private func distance(_ reference: [String], _ output: [String]) -> Int {
        var row = Array(0...output.count)
        for (index, token) in reference.enumerated() {
            var diagonal = row[0]
            row[0] = index + 1
            for (position, candidate) in output.enumerated() {
                let column = position + 1
                let above = row[column]
                row[column] = min(above + 1, row[column - 1] + 1, diagonal + (token == candidate ? 0 : 1))
                diagonal = above
            }
        }
        return row[output.count]
    }

    private func run(_ command: String, _ args: [String]) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: command)
        process.arguments = args
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw POSIXError(.EIO) }
    }
}

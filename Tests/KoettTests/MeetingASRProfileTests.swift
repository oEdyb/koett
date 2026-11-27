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
        let mode = environment["KOETT_MEETING_PROFILE_MODE"].flatMap(MeetingASRMode.init(rawValue:)) ?? .routed
        let (tracks, utterances, _) = try fixture(temporary)
        let (denseTracks, expected) = denseFixture(tracks, utterances)
        let meetingSeconds = Double(denseTracks.map(\.count).max() ?? 0) / 16_000
        XCTAssertGreaterThanOrEqual(meetingSeconds, 180)

        let history = TranscriptStore(fileURL: temporary.appending(path: "history.sqlite3"))
        let id = UUID()
        try writeMeeting(id, root: temporary, tracks: denseTracks)
        let transcription = MeetingTranscription(root: temporary, history: history, mode: mode,
                                                 models: root, executable: URL(fileURLWithPath: worker),
                                                 library: URL(fileURLWithPath: library), networkDenied: true)
        let start = ContinuousClock.now
        try await transcription.run(id, progress: { _ in })
        let elapsed = ms(since: start)
        let directory = MeetingBundle.directory(in: temporary, id: id)
        let (receipt, markdown) = try XCTUnwrap(MeetingPublication.receipt(at: directory))
        XCTAssertFalse(receipt.fallback)
        XCTAssertFalse(FileManager.default.fileExists(atPath: directory.appending(path: "audio/you.journal").path))
        XCTAssertFalse(try MeetingControl.inspect(id: id, root: temporary).recoverable)
        XCTAssertTrue(try MeetingPublication.projections(at: directory, receipt: receipt).historyIndexed)
        XCTAssertFalse(receipt.backends.isEmpty)
        var errors = [String: Int]()
        var counts = [String: Int]()
        for track in MeetingTrack.allCases {
            let actual = markdown.split(whereSeparator: \.isNewline).compactMap { line -> String? in
                guard let divider = line.range(of: "**\(track.label)** ("), let end = line[divider.upperBound...].range(of: "): ")
                else { return nil }
                return String(line[end.upperBound...])
            }.joined(separator: " ")
            let reference = words(expected[track == .you ? 0 : 1])
            errors[track.rawValue] = distance(reference, words(actual))
            counts[track.rawValue] = reference.count
            XCTAssertFalse(actual.isEmpty)
        }
        print("MEETING_PROFILE \(mode.rawValue) \(meetingSeconds) meeting seconds, cold \(elapsed / 1000) s, \(elapsed * 0.06 / meetingSeconds) s/min, errors \(errors), words \(counts), backends \(receipt.backends)")
    }

    private func denseFixture(_ tracks: [[Int16]], _ utterances: [Utterance]) -> ([[Int16]], [String]) {
        var result = [[Int16]]()
        var references = [String]()
        for track in tracks.indices {
            // Each cycle contains adjacent same-language sentences and a switch.
            let lines = utterances.filter { $0.track == track }.sorted { $0.language < $1.language }
            var samples = Array(repeating: Int16(0), count: track * 8_000)
            var text: [String] = []
            while samples.count < 180 * 16_000 {
                for line in lines {
                    samples.append(contentsOf: tracks[track][line.range.clamped(to: tracks[track].indices)])
                    samples.append(contentsOf: repeatElement(0, count: 10_400))
                    text.append(line.reference)
                }
            }
            result.append(samples)
            references.append(text.joined(separator: " "))
        }
        return (result, references)
    }

    private func writeMeeting(_ id: UUID, root: URL, tracks: [[Int16]]) throws {
        let cutoff = UInt64(tracks.map(\.count).max() ?? 0)
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: id, startedAt: .now))
        for (track, samples) in zip(MeetingTrack.allCases, tracks) {
            let writer = try MeetingJournal.Writer(bundle.audio(track))
            try writer.append(.epoch(first: 0, generation: 1))
            for first in stride(from: 0, to: samples.count, by: MeetingJournal.maxFrames) {
                let end = min(first + MeetingJournal.maxFrames, samples.count)
                try writer.append(.pcm(first: UInt64(first), samples: Array(samples[first..<end])))
            }
            try writer.sync()
        }
        _ = try bundle.seal(.init(cutoff: cutoff, goal: .finish))
    }

    func testWorkerFallsBackWithMissingAndCorruptTurboOffline() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let path = environment["KOETT_MEETING_PROFILE_MODELS"],
              let worker = environment["KOETT_MEETING_PROFILE_WORKER"],
              let library = environment["KOETT_MEETING_PROFILE_LIBRARY"]
        else { throw XCTSkip("Provision the pinned meeting profile assets") }
        let root = URL(fileURLWithPath: path)
        let temporary = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: temporary) }
        let (tracks, _, _) = try fixture(temporary)
        for component in [MeetingModelPins.parakeet, MeetingModelPins.vad] {
            try FileManager.default.createSymbolicLink(at: temporary.appending(path: component),
                                                       withDestinationURL: root.appending(path: component))
        }
        for corrupt in [false, true] {
            if corrupt {
                try Data([0, 1, 2]).write(to: temporary.appending(path: MeetingModelPins.turbo))
            }
            let client = try MeetingWorkerClient(executable: URL(fileURLWithPath: worker), models: temporary,
                                                 library: URL(fileURLWithPath: library), mode: .routed, networkDenied: true)
            defer { client.cancel() }
            let samples = Array(tracks[0].prefix(7 * 16_000))
            let reply = try client.decode(.init(pcm: samples, ownershipStart: 0, ownershipEnd: samples.count))
            XCTAssertTrue(reply.fallback)
            XCTAssertFalse(reply.turns.isEmpty)
            XCTAssertEqual(Set(reply.turns.map(\.backend)), ["parakeet-v3-int8"])
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

    private func ms(since point: ContinuousClock.Instant) -> Double {
        let duration = point.duration(to: .now).components
        return Double(duration.seconds) * 1_000 + Double(duration.attoseconds) / 1e15
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

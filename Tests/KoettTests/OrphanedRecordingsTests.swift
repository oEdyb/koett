@testable import Koett
import AVFoundation
import Darwin
import XCTest

@MainActor
final class OrphanedRecordingsTests: XCTestCase {
    private var directory: URL!
    private var transcribed: [URL] = []
    private var persisted: [PreparedDictation] = []
    private var preserved: [URL] = []
    private var deleted: [URL] = []

    override func setUp() async throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-orphan-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
    }

    // MARK: Header repair

    /// A SIGKILLed recording: the header says zero bytes and the audio stops mid-frame.
    private func killedRecording(seconds: Int = 2, name: String? = nil, pid: pid_t = 999_999) throws -> URL {
        let finished = directory.appendingPathComponent("finished-\(UUID().uuidString).wav")
        defer { try? FileManager.default.removeItem(at: finished) }
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let sink = try MicrophoneFileSink(url: finished, inputFormat: format)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_600))
        buffer.frameLength = 1_600
        buffer.floatChannelData![0].initialize(repeating: 0.25, count: 1_600)
        for _ in 0..<(seconds * 10 + 2) { sink.append(buffer) }
        sink.close()
        XCTAssertNil(sink.error)

        var bytes = try Data(contentsOf: finished)
        let tag = Data("data".utf8)
        let dataTag = try XCTUnwrap(bytes.range(of: tag)).lowerBound
        let keptBytes = dataTag + 8 + seconds * 64_000 + 3
        XCTAssertLessThan(keptBytes, bytes.count)
        bytes = bytes.prefix(keptBytes)
        bytes.replaceSubrange(4..<8, with: withUnsafeBytes(of: UInt32(dataTag).littleEndian) { Data($0) })
        bytes.replaceSubrange((dataTag + 4)..<(dataTag + 8), with: Data(count: 4))
        let url = name.map { directory.appendingPathComponent($0) }
            ?? OrphanedRecordings.recordingURL(in: directory, pid: pid)
        try bytes.write(to: url)
        return url
    }

    func testRepairMakesATruncatedSinkFileReadableWithEveryWholeFrame() throws {
        let url = try killedRecording(seconds: 2)
        XCTAssertEqual(try AVAudioFile(forReading: url).length, 0)

        XCTAssertEqual(try XCTUnwrap(WAVHeaderRepair.repair(url)), 2, accuracy: 0.0001)

        let samples = try XCTUnwrap(DictationTranscriber.loadSamples(url))
        XCTAssertEqual(samples.count, 32_000)
        XCTAssertEqual(samples[31_999], samples[100], accuracy: 0.01)
        XCTAssertGreaterThan(samples[31_999], 0.2)
    }

    func testRepairLeavesAFinalizedRecordingUntouched() throws {
        let url = directory.appendingPathComponent("complete.wav")
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let sink = try MicrophoneFileSink(url: url, inputFormat: format)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 16_000))
        buffer.frameLength = 16_000
        buffer.floatChannelData![0].initialize(repeating: 0.25, count: 16_000)
        sink.append(buffer)
        sink.close()
        let before = try Data(contentsOf: url)

        XCTAssertEqual(try XCTUnwrap(WAVHeaderRepair.repair(url)), 1, accuracy: 0.05)
        XCTAssertEqual(try Data(contentsOf: url), before)
    }

    func testRepairRefusesAFileWithoutWAVChunks() throws {
        let url = directory.appendingPathComponent("garbage.wav")
        try Data(repeating: 7, count: 8_192).write(to: url)
        XCTAssertNil(try WAVHeaderRepair.repair(url))
        XCTAssertEqual(try Data(contentsOf: url), Data(repeating: 7, count: 8_192))
    }

    func testAudioReachesDiskWithinTheCrashLossBoundBeforeClose() async throws {
        for rate in [48_000.0, 16_000.0] {
            let url = directory.appendingPathComponent("bound-\(Int(rate)).wav")
            let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1))
            let sink = try MicrophoneFileSink(url: url, inputFormat: format)
            let frames = AVAudioFrameCount(rate / 100)
            let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames))
            buffer.frameLength = frames
            buffer.floatChannelData![0].initialize(repeating: 0.25, count: Int(frames))
            let seconds = 12
            for _ in 0..<(seconds * 100) { sink.append(buffer) }
            XCTAssertNil(sink.error)

            // At least everything older than the bound must be readable without close().
            let wanted = (Double(seconds) - MicrophoneFileSink.writeBufferSeconds - 0.1) * 64_000
            let deadline = Date(timeIntervalSinceNow: 3)
            var onDisk = 0.0
            while onDisk < wanted, Date() < deadline {
                try await Task.sleep(for: .milliseconds(20))
                let size = try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int ?? 0
                onDisk = Double(size - 4_096)
            }
            XCTAssertGreaterThanOrEqual(onDisk, wanted, "rate \(rate)")
            sink.close()
        }
    }

    // MARK: Ownership

    func testOnlyNewFormatNamesParse() {
        let uuid = UUID().uuidString
        XCTAssertEqual(OrphanedRecordings.parse(fileName: "koett-42-\(uuid).wav")?.pid, 42)
        XCTAssertEqual(OrphanedRecordings.parse(fileName: "koett-recovering-42-\(uuid).wav")?.recovering, true)
        XCTAssertNil(OrphanedRecordings.parse(fileName: "koett-\(uuid).wav"))
        // An old-format UUID whose first group is all digits must not read as a pid.
        XCTAssertNil(OrphanedRecordings.parse(fileName: "koett-12345678-ABCD-4ABC-8ABC-ABCDEFABCDEF.wav"))
        XCTAssertNil(OrphanedRecordings.parse(fileName: "koett-42-\(uuid).txt"))
    }

    func testFindAdoptsDeadAndOwnPidsButNeverALiveInstance() throws {
        let dead = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        let own = OrphanedRecordings.recordingURL(in: directory, pid: 200)
        let liveOther = OrphanedRecordings.recordingURL(in: directory, pid: 300)
        let oldFormat = directory.appendingPathComponent("koett-\(UUID().uuidString).wav")
        for url in [dead, own, liveOther, oldFormat] { try Data().write(to: url) }

        var recordings = OrphanedRecordings(directory: directory)
        recordings.ownPID = 200
        recordings.isProcessAlive = { $0 == 200 || $0 == 300 }

        XCTAssertEqual(
            Set(recordings.find().map(\.url.lastPathComponent)),
            [dead.lastPathComponent, own.lastPathComponent]
        )
    }

    func testRealLivenessCheckSeesThisProcessAndNotAnImpossiblePid() {
        XCTAssertTrue(OrphanedRecordings.processExists(getpid()))
        XCTAssertFalse(OrphanedRecordings.processExists(99_999_999))
    }

    func testFindReturnsOldestFirst() throws {
        let older = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        let newer = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        for url in [newer, older] { try Data().write(to: url) }
        try FileManager.default.setAttributes(
            [.modificationDate: Date(timeIntervalSinceNow: -60)], ofItemAtPath: older.path)

        var recordings = OrphanedRecordings(directory: directory)
        recordings.isProcessAlive = { _ in false }
        XCTAssertEqual(recordings.find().map(\.url.lastPathComponent), [older.lastPathComponent, newer.lastPathComponent])
    }

    // MARK: Routing

    private func recordings(minimumDeadline: TimeInterval = 120) -> OrphanedRecordings {
        var recordings = OrphanedRecordings(directory: directory)
        recordings.isProcessAlive = { _ in false }
        recordings.minimumDeadline = minimumDeadline
        return recordings
    }

    private func actions(
        durable: Bool = true,
        transcribe: @escaping @MainActor () async throws -> PreparedDictation
    ) -> OrphanedRecordings.Actions {
        OrphanedRecordings.Actions(
            transcribe: { [unowned self] url in
                transcribed.append(url)
                return try await transcribe()
            },
            persist: { [unowned self] prepared in
                persisted.append(prepared)
                return durable
            },
            preserve: { [unowned self] url in preserved.append(url) },
            delete: { [unowned self] url in
                deleted.append(url)
                try? FileManager.default.removeItem(at: url)
            }
        )
    }

    private func orphan(_ url: URL) -> OrphanedRecordings.Orphan {
        OrphanedRecordings.Orphan(url: url, recoveryInterrupted: url.lastPathComponent.hasPrefix("koett-recovering-"))
    }

    func testTextIsPersistedOnceAndTheAudioDeleted() async throws {
        let url = try killedRecording()
        let outcome = await recordings().recover(orphan(url), actions: actions {
            PreparedDictation(text: "Hello.", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .recovered(text: "Hello."))
        XCTAssertEqual(persisted.map(\.text), ["Hello."])
        XCTAssertEqual(deleted.count, 1)
        XCTAssertEqual(preserved, [])
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), [])
    }

    func testTranscriberReceivesARepairedFile() async throws {
        let url = try killedRecording()
        var frames = 0
        let actions = OrphanedRecordings.Actions(
            transcribe: { url in
                frames = try DictationTranscriber.loadSamples(url)?.count ?? 0
                return PreparedDictation(text: "x", model: "m", rawText: nil)
            },
            persist: { _ in true },
            preserve: { _ in },
            delete: { _ in }
        )
        _ = await recordings().recover(orphan(url), actions: actions)
        XCTAssertEqual(frames, 32_000)
    }

    func testNoSpeechIsDeletedQuietly() async throws {
        let url = try killedRecording()
        let outcome = await recordings().recover(orphan(url), actions: actions {
            PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .discarded)
        XCTAssertEqual(persisted.count, 0)
        XCTAssertEqual(preserved, [])
        XCTAssertEqual(deleted.count, 1)
    }

    func testCleanupThatRemovedAllTextStillSavesTheRawText() async throws {
        let url = try killedRecording()
        let outcome = await recordings().recover(orphan(url), actions: actions {
            PreparedDictation(text: "", model: "m", rawText: "Um.")
        })
        XCTAssertEqual(outcome, .recovered(text: ""))
        XCTAssertEqual(persisted.map(\.rawText), ["Um."])
    }

    func testUnderAQuarterSecondIsDeletedWithoutASR() async throws {
        let url = try killedRecording(seconds: 2)
        let handle = try FileHandle(forUpdating: url)
        let bytes = try Data(contentsOf: url)
        let dataStart = try XCTUnwrap(bytes.range(of: Data("data".utf8))).upperBound + 4
        try handle.truncate(atOffset: UInt64(dataStart + 4_000))
        try handle.close()

        let outcome = await recordings().recover(orphan(url), actions: actions {
            XCTFail("ASR must not run")
            return PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .discarded)
        XCTAssertEqual(transcribed, [])
        XCTAssertEqual(deleted.count, 1)
    }

    func testEmptyFileHoldsNothingAndIsDeleted() async throws {
        let url = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        try Data().write(to: url)
        let outcome = await recordings().recover(orphan(url), actions: actions {
            PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .discarded)
        XCTAssertEqual(preserved, [])
    }

    func testASRFailurePreservesAudio() async throws {
        let url = try killedRecording()
        let outcome = await recordings().recover(orphan(url), actions: actions {
            throw NSError(domain: "test", code: 1)
        })
        XCTAssertEqual(outcome, .preserved)
        XCTAssertEqual(preserved.count, 1)
        XCTAssertTrue(FileManager.default.fileExists(atPath: preserved[0].path))
        XCTAssertEqual(deleted, [])
        XCTAssertEqual(persisted.count, 0)
    }

    func testUnrepairableFileIsPreservedAndNeverDeleted() async throws {
        let url = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        try Data(repeating: 7, count: 8_192).write(to: url)
        let outcome = await recordings().recover(orphan(url), actions: actions {
            XCTFail("ASR must not run")
            return PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .preserved)
        XCTAssertEqual(try Data(contentsOf: preserved[0]), Data(repeating: 7, count: 8_192))
        XCTAssertEqual(deleted, [])
    }

    func testStuckTranscriberHitsTheDeadlineAndPreservesAudio() async throws {
        let url = try killedRecording(seconds: 1)
        let outcome = await recordings(minimumDeadline: 0).recover(orphan(url), actions: actions {
            // Ignores cancellation, as a stuck model would.
            await withCheckedContinuation { (_: CheckedContinuation<Void, Never>) in }
            return PreparedDictation(text: "late", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .preserved)
        XCTAssertEqual(persisted.count, 0)
        XCTAssertEqual(preserved.count, 1)
    }

    func testARecoveryThatDiedIsPreservedInsteadOfRetried() async throws {
        let claimed = directory.appendingPathComponent("koett-recovering-100-\(UUID().uuidString).wav")
        _ = try killedRecording(name: claimed.lastPathComponent)
        let found = recordings().find()
        XCTAssertEqual(found.map(\.recoveryInterrupted), [true])

        let outcome = await recordings().recover(found[0], actions: actions {
            XCTFail("ASR must not run")
            return PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .preserved)
        XCTAssertEqual(transcribed, [])
    }

    func testClaimMarksTheFileBeforeTranscribing() async throws {
        let url = try killedRecording()
        var seenDuringASR: [String] = []
        _ = await recordings().recover(orphan(url), actions: actions { [directory] in
            seenDuringASR = try FileManager.default.contentsOfDirectory(atPath: directory!.path)
            return PreparedDictation(text: "x", model: "m", rawText: nil)
        })
        XCTAssertEqual(seenDuringASR.count, 1)
        XCTAssertTrue(seenDuringASR[0].hasPrefix("koett-recovering-\(getpid())-"), seenDuringASR[0])
    }

    func testFileClaimedByAnotherInstanceIsSkipped() async throws {
        let url = OrphanedRecordings.recordingURL(in: directory, pid: 100)
        let outcome = await recordings().recover(orphan(url), actions: actions {
            XCTFail("ASR must not run")
            return PreparedDictation(text: "", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .skipped)
        XCTAssertEqual(deleted + preserved, [])
    }

    func testTextWithoutADurableCopyKeepsTheAudio() async throws {
        let url = try killedRecording()
        let outcome = await recordings().recover(orphan(url), actions: actions(durable: false) {
            PreparedDictation(text: "Hello.", model: "m", rawText: nil)
        })
        XCTAssertEqual(outcome, .unsaved)
        XCTAssertEqual(deleted, [])
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path).count, 1)
    }
}

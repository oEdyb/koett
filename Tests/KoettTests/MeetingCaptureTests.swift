import AVFoundation
import CoreAudio
import Foundation
import XCTest
@testable import Koett

@MainActor
private final class FakeMeetingSource: MeetingAudioSource {
    let ring: MeetingAudioRing
    var isConnected = true
    var stops = 0
    init(capacityFrames: Int? = nil) { ring = MeetingAudioRing(rate: 16_000, capacityFrames: capacityFrames) }
    func stop() { stops += 1; ring.stop() }

    func send(_ hostTime: UInt64, frames: Int = 32, value: Float = 0.25) {
        sendInterleaved(ring, at: hostTime, frames: frames, value: value)
    }
}

final class MeetingCaptureTests: XCTestCase {
    @MainActor
    func testConfirmationNeverStopsSourcesAndDrainFencesLateCallbacks() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let you = FakeMeetingSource()
        let other = FakeMeetingSource()
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in },
                                           makeSource: { $0 == .you ? you : other })
        let id = UUID()
        try controller.send(.start(id))
        try await Task.sleep(for: .milliseconds(10))
        let first = AudioGetCurrentHostTime()
        you.send(first); other.send(first)
        let key = try XCTUnwrap(controller.send(.stop(id)).confirmation)
        XCTAssertEqual(try controller.send(.toggle).confirmation, key)
        try await Task.sleep(for: .milliseconds(3))
        let second = AudioGetCurrentHostTime()
        you.send(second); other.send(second)
        try controller.send(.resume(key))
        await waitForMeeting { controller.snapshot.tracks["other"]?.level ?? 0 > 0 }
        XCTAssertEqual(you.stops, 0)
        XCTAssertEqual(other.stops, 0)
        XCTAssertGreaterThan(controller.snapshot.tracks["other"]?.level ?? 0, 0)
        let fresh = try XCTUnwrap(controller.send(.stop(id)).confirmation)
        try await Task.sleep(for: .milliseconds(4))
        XCTAssertEqual(try controller.send(.cancel(fresh)).phase, .saving)
        XCTAssertEqual(you.stops, 1)
        XCTAssertEqual(other.stops, 1)
        await waitForMeeting { controller.snapshot.phase == .idle }
        you.send(AudioGetCurrentHostTime())
        var late = 0
        you.ring.drain { _, _ in late += 1 }
        XCTAssertEqual(late, 0)
        let bundle = try MeetingBundle.open(in: root, id: id)
        let seal = try bundle.recoverSeal()
        XCTAssertEqual(seal.intent.goal, .hold)
        for track in MeetingTrack.allCases {
            var packets = 0
            _ = try MeetingJournal.read(bundle.audio(track), cutoff: seal.intent.cutoff) {
                if case .pcm = $0 { packets += 1 }
            }
            XCTAssertEqual(packets, 2)
        }
    }

    @MainActor
    func testRouteLossKeepsOtherTrackAndOpensNewEpoch() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let first = FakeMeetingSource(), replacement = FakeMeetingSource(), other = FakeMeetingSource()
        var you = first
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in },
                                           makeSource: { $0 == .you ? you : other })
        let id = UUID()
        try controller.send(.start(id))
        let now = AudioGetCurrentHostTime()
        first.send(now); other.send(now)
        controller.poll()
        you = replacement
        first.isConnected = false
        try await Task.sleep(for: .milliseconds(270))
        controller.poll()
        XCTAssertEqual(controller.snapshot.tracks["you"]?.health, .reconnecting)
        XCTAssertEqual(other.stops, 0)
        try await Task.sleep(for: .milliseconds(510))
        controller.poll()
        replacement.send(AudioGetCurrentHostTime())
        other.send(AudioGetCurrentHostTime())
        try await Task.sleep(for: .milliseconds(5))
        controller.poll()
        XCTAssertEqual(controller.snapshot.tracks["you"]?.health, .receiving)
        controller.shutdown()
        await waitForMeeting { controller.snapshot.phase == .idle }
        let bundle = try MeetingBundle.open(in: root, id: id)
        var epochs: [UInt64] = []
        var gaps: [MeetingGap] = []
        _ = try MeetingJournal.read(bundle.audio(.you)) {
            if case let .epoch(_, generation, _) = $0 { epochs.append(generation) }
            if case let .gap(_, _, cause) = $0 { gaps.append(cause) }
        }
        XCTAssertEqual(epochs, [1, 2])
        XCTAssertTrue(gaps.contains(.sourceRestart))
    }

    @MainActor
    func testFailedStopIntentKeepsBothSourcesRecording() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let you = FakeMeetingSource(), other = FakeMeetingSource()
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in },
                                           makeSource: { $0 == .you ? you : other })
        let id = UUID()
        try controller.send(.start(id))
        let key = try XCTUnwrap(controller.send(.stop(id)).confirmation)
        let intent = MeetingBundle.directory(in: root, id: id).appending(path: "intent.json")
        try FileManager.default.createDirectory(at: intent, withIntermediateDirectories: false)
        XCTAssertThrowsError(try controller.send(.finish(key))) {
            XCTAssertEqual(($0 as? MeetingCommandError)?.code, "intent_write_failed")
        }
        XCTAssertEqual(controller.snapshot.phase, .confirmStop)
        XCTAssertEqual(controller.snapshot.confirmation, key)
        XCTAssertEqual(you.stops + other.stops, 0)
        try FileManager.default.removeItem(at: intent)
        try controller.send(.cancel(key))
        XCTAssertEqual(you.stops + other.stops, 2)
        await waitForMeeting { controller.snapshot.phase == .idle }
    }

    func testSystemAudioCallbackAcceptsBuffersOffTheMainActor() async {
        let ring = MeetingAudioRing(rate: 48_000)
        await Task.detached {
            let callback = MeetingSystemAudio.callback(for: ring)
            var samples: [Float] = [0.25, 0.5]
            samples.withUnsafeMutableBytes {
                var input = AudioBufferList(mNumberBuffers: 1, mBuffers: .init(
                    mNumberChannels: 1, mDataByteSize: UInt32($0.count), mData: $0.baseAddress))
                var output = AudioBufferList()
                var now = AudioTimeStamp(), inputTime = AudioTimeStamp(), outputTime = AudioTimeStamp()
                inputTime.mFlags = .hostTimeValid
                inputTime.mHostTime = 123
                callback(&now, &input, &inputTime, &output, &outputTime)
            }
        }.value
        var frames = 0
        ring.drain { stamp, samples in
            XCTAssertEqual(stamp.hostTime, 123)
            XCTAssertEqual(Array(samples), [0.25, 0.5])
            frames += stamp.frames
        }
        XCTAssertEqual(frames, 2)
    }

    @MainActor
    func testRingOverrunMarksNextBuffer() {
        let source = FakeMeetingSource(capacityFrames: 32)
        source.send(1); source.send(2)
        var stamps: [UInt64] = []
        source.ring.drain { stamp, _ in stamps.append(stamp.hostTime) }
        XCTAssertEqual(stamps, [1])
        source.send(3)
        source.ring.drain { stamp, _ in
            XCTAssertEqual(stamp.hostTime, 3)
            XCTAssertTrue(stamp.dropped)
        }
    }
    func testStalledRingHoldsEightSecondsOfSmallAndLargeMultichannelCallbacks() {
        for frames in [128, 480, 1_024] {
            let ring = MeetingAudioRing(rate: 48_000, channels: 18)
            let callbacks = (48_000 * 8) / frames
            for index in 0..<callbacks {
                sendInterleaved(ring, at: UInt64(index + 1), frames: frames, value: 0.25)
            }
            XCTAssertFalse(ring.hasTrailingOverrun)
            sendInterleaved(ring, at: UInt64(callbacks + 1), frames: frames, value: 0.5)
            XCTAssertTrue(ring.hasTrailingOverrun)
            var received = 0
            ring.drain { stamp, samples in
                received += 1
                XCTAssertEqual(stamp.hostTime, UInt64(received))
                XCTAssertEqual(samples.count, frames * 18)
                XCTAssertEqual(samples.first, 0.25)
                XCTAssertEqual(samples.last, 0.25)
            }
            XCTAssertEqual(received, callbacks)
            sendInterleaved(ring, at: UInt64(callbacks + 2), frames: frames, value: 0.5)
            ring.drain { stamp, samples in
                XCTAssertTrue(stamp.dropped)
                XCTAssertTrue(samples.allSatisfy { $0 == 0.5 })
            }
        }
    }

    func testPlanarPacketAcrossRingWrapKeepsChannelOrder() {
        let ring = MeetingAudioRing(rate: 48_000, channels: 2, capacityFrames: 5)
        sendInterleaved(ring, at: 1, frames: 4, value: 1)
        ring.drain { _, _ in }

        var left: [Float] = [0.125, 0.25, 0.5]
        var right: [Float] = [-0.125, -0.25, -0.5]
        let buffers = AudioBufferList.allocate(maximumBuffers: 2)
        defer { buffers.unsafeMutablePointer.deallocate() }
        left.withUnsafeMutableBytes { left in
            right.withUnsafeMutableBytes { right in
                buffers[0] = .init(mNumberChannels: 1, mDataByteSize: UInt32(left.count), mData: left.baseAddress)
                buffers[1] = .init(mNumberChannels: 1, mDataByteSize: UInt32(right.count), mData: right.baseAddress)
                ring.copy(buffers.unsafePointer, hostTime: 2)
            }
        }

        var packets = 0
        ring.drain { stamp, samples in
            packets += 1
            XCTAssertTrue(stamp.planar)
            XCTAssertEqual(stamp.frames, 3)
            XCTAssertEqual(Array(samples), left + right)
        }
        XCTAssertEqual(packets, 1)
    }

    @MainActor
    func testWriterTimerJournalsOverrunAndResamplesWithClockDrift() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let bundle = try MeetingBundle.create(in: root, manifest: .init(id: UUID(), startedAt: .now))
        let first = AudioGetCurrentHostTime()
        let clock = MeetingAudioClock(zero: first)
        let writer = try MeetingAudioWriter(bundle: bundle, clock: clock)
        let ring = MeetingAudioRing(rate: 48_000, channels: 2, capacityFrames: 480)
        func send(at offset: UInt64) {
            sendInterleaved(ring, at: first + AudioConvertNanosToHostTime(offset * 62_500), frames: 480, value: 0.25)
        }
        send(at: 0)
        send(at: 160)
        writer.setSource(ring, for: .you)
        await waitForMeeting { writer.reading.levels[.you] == 0.25 }
        let driftRing = MeetingAudioRing(rate: 48_000, channels: 2)
        send(at: 320)
        writer.setSource(driftRing, for: .you)
        for index in 0..<180 {
            let offset = 480 + UInt64(index * 161)
            sendInterleaved(driftRing, at: first + AudioConvertNanosToHostTime(offset * 62_500), frames: 480, value: 0.25)
        }
        ring.stop()
        driftRing.stop()
        let cutoff: UInt64 = 30_000
        try await writer.finish(.init(cutoff: cutoff, goal: .hold))
        let seal = try bundle.recoverSeal()
        var nominalFrameCount = 0
        var overrun: Range<UInt64>?
        let actual = try MeetingJournal.read(bundle.audio(.you), cutoff: cutoff) { record in
            switch record {
            case let .gap(first, count, .overrun): overrun = first..<(first + count)
            case let .pcm(first, samples):
                if first < 480 { nominalFrameCount += samples.count }
                XCTAssertTrue(samples.allSatisfy { $0 == 8_192 })
            default: break
            }
        }
        XCTAssertEqual(nominalFrameCount, 320)
        XCTAssertEqual(overrun, 160..<320)
        XCTAssertEqual(actual.capturedUntil, seal.tracks[.you]?.capturedUntil)
        XCTAssertEqual(Double(actual.capturedUntil), Double(480 + 180 * 161), accuracy: 4)
    }

    @MainActor
    func testSystemAudioZerosRaiseSnapshotIssueAndRealSignalClearsIt() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let you = FakeMeetingSource()
        let other = FakeMeetingSource()
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in },
                                           makeSource: { $0 == .you ? you : other })
        try controller.send(.start(UUID()))
        let first = AudioGetCurrentHostTime()
        you.send(first)
        for index in 0..<6 {
            other.send(first + AudioConvertNanosToHostTime(UInt64(index) * 1_000_000_000), frames: 16_000, value: 0)
        }
        await waitForMeeting { controller.snapshot.tracks["other"]?.issue != nil }
        XCTAssertEqual(controller.snapshot.tracks["other"]?.health, .receiving)
        XCTAssertNil(controller.snapshot.tracks["you"]?.issue)
        other.send(first + AudioConvertNanosToHostTime(6_000_000_000))
        await waitForMeeting { controller.snapshot.tracks["other"]?.issue == nil && controller.snapshot.tracks["other"]?.level == 0.25 }
        controller.shutdown()
        await waitForMeeting { controller.snapshot.phase == .idle }
    }

    func testSystemAudioReconnectsOnlyForTapFormatAndRequiresSelfExclusion() throws {
        var old = AudioStreamBasicDescription()
        old.mSampleRate = 48_000
        old.mChannelsPerFrame = 2
        XCTAssertTrue(MeetingSystemAudio.sameFormat(old, old))
        var new = old
        new.mSampleRate = 44_100
        XCTAssertFalse(MeetingSystemAudio.sameFormat(old, new))
        new = old
        new.mChannelsPerFrame = 1
        XCTAssertFalse(MeetingSystemAudio.sameFormat(old, new))
        XCTAssertThrowsError(try MeetingSystemAudio.excludingProcess(kAudioObjectUnknown))
        XCTAssertEqual(try MeetingSystemAudio.excludingProcess(123), [123])
    }

}

final class MeetingAdmissionTests: XCTestCase {
    @MainActor
    func testPreviewReservationIsReleasedOnPartialOpenFailure() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let mic = FakeMeetingSource()
        var reservations: [Bool] = []
        let controller = MeetingController(root: root, canStart: { true }, reserve: { reservations.append($0) },
                                           makeSource: {
            if $0 == .other { throw POSIXError(.EPERM) }
            return mic
        })
        let id = UUID()
        XCTAssertThrowsError(try controller.send(.start(id)))
        XCTAssertEqual(mic.stops, 1)
        await waitForMeeting { controller.snapshot.phase == .idle }
        XCTAssertEqual(reservations, [true, false])
        XCTAssertFalse(controller.state.reservesForeground)
        let bundle = try MeetingBundle.open(in: root, id: id)
        XCTAssertEqual(try bundle.recoverSeal().intent.goal, .interrupted)
    }

    @MainActor
    func testShortcutDefaultsOffAndSurfacesStartFailureWhileCLIStillStarts() async throws {
        let name = UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        XCTAssertFalse(MeetingController.shortcutEnabled(in: defaults))
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        var shouldFail = true
        let controller = MeetingController(root: root, canStart: { true }, reserve: { _ in }, makeSource: { _ in
            if shouldFail { throw MeetingCommandError(code: "fixture", message: "Capture unavailable") }
            return FakeMeetingSource()
        })
        defaults.set(true, forKey: "meetingShortcutEnabled")
        XCTAssertTrue(MeetingController.shortcutEnabled(in: defaults))
        controller.sendShortcut()
        XCTAssertEqual(controller.snapshot.issue, "Capture unavailable")
        await waitForMeeting { controller.snapshot.phase == .idle }
        defaults.removeObject(forKey: "meetingShortcutEnabled")
        shouldFail = false
        XCTAssertEqual(try controller.send(.toggle).phase, .recording)
        controller.shutdown()
        await waitForMeeting { controller.snapshot.phase == .idle }
    }

    @MainActor
    func testBusyForegroundCannotStartCapture() async throws {
        let root = try meetingTestDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let controller = MeetingController(root: root, canStart: { false }, reserve: { _ in XCTFail() },
                                           makeSource: { _ in XCTFail(); return FakeMeetingSource() })
        XCTAssertThrowsError(try controller.send(.toggle))
        XCTAssertEqual(controller.snapshot.phase, .idle)
    }
}

private func sendInterleaved(_ ring: MeetingAudioRing, at hostTime: UInt64, frames: Int, value: Float) {
    var pcm = [Float](repeating: value, count: frames * ring.channels)
    pcm.withUnsafeMutableBytes {
        var buffers = AudioBufferList(mNumberBuffers: 1, mBuffers: .init(
            mNumberChannels: UInt32(ring.channels), mDataByteSize: UInt32($0.count), mData: $0.baseAddress))
        ring.copy(&buffers, hostTime: hostTime)
    }
}

@MainActor
func waitForMeeting(_ condition: () -> Bool, file: StaticString = #filePath, line: UInt = #line) async {
    let deadline = ContinuousClock.now + .seconds(2)
    while !condition(), ContinuousClock.now < deadline { try? await Task.sleep(for: .milliseconds(10)) }
    XCTAssertTrue(condition(), file: file, line: line)
}

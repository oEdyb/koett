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
    init(capacity: Int = 64) { ring = MeetingAudioRing(rate: 16_000, capacity: capacity) }
    func stop() { stops += 1; ring.stop() }

    func send(_ hostTime: UInt64, frames: Int = 32, value: Float = 0.25) {
        var pcm = [Float](repeating: value, count: frames)
        pcm.withUnsafeMutableBytes {
            var buffers = AudioBufferList(mNumberBuffers: 1, mBuffers: .init(
                mNumberChannels: 1, mDataByteSize: UInt32($0.count), mData: $0.baseAddress))
            ring.copy(&buffers, hostTime: hostTime)
        }
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
        controller.poll()
        XCTAssertEqual(you.stops, 0)
        XCTAssertEqual(other.stops, 0)
        XCTAssertGreaterThan(controller.snapshot.tracks["other"]?.level ?? 0, 0)
        let fresh = try XCTUnwrap(controller.send(.stop(id)).confirmation)
        try await Task.sleep(for: .milliseconds(4))
        try controller.send(.cancel(fresh))
        XCTAssertEqual(you.stops, 1)
        XCTAssertEqual(other.stops, 1)
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
        let source = FakeMeetingSource(capacity: 1)
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
        XCTAssertEqual(reservations, [true, false])
        XCTAssertFalse(controller.state.reservesForeground)
        let bundle = try MeetingBundle.open(in: root, id: id)
        XCTAssertEqual(try bundle.recoverSeal().intent.goal, .interrupted)
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

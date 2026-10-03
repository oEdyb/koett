import AVFoundation
import FluidAudio
@testable import Koett
import XCTest

final class DictationTranscriberTests: XCTestCase {
    func testReadsEveryFrameAcrossBufferBoundariesWithoutChangingSamples() throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let samples = (0..<33_017).map { Float($0 % 127) / 127 }
        try write(samples, to: url)
        XCTAssertEqual(try DictationTranscriber.loadSamples(url), samples)
    }

    func testFiveMinuteRecordingKeepsAllAudioInOrder() throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let frames = 16_000 * 301
        var samples = [Float](repeating: 0.125, count: frames)
        samples[0] = -0.5
        samples[frames - 1] = 0.75
        try write(samples, to: url)
        let loaded = try XCTUnwrap(DictationTranscriber.loadSamples(url))
        XCTAssertEqual(loaded.count, frames)
        XCTAssertEqual(loaded.first, -0.5)
        XCTAssertEqual(loaded.last, 0.75)
        XCTAssertEqual(loaded[16_383], 0.125)
    }

    func testMemoryIsBoundedAndOtherFormatsKeepExistingFilePath() throws {
        let canonical = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1))
        let limit = DictationTranscriber.maximumBufferedFrames
        XCTAssertTrue(DictationTranscriber.canBuffer(format: canonical, frames: limit))
        XCTAssertFalse(DictationTranscriber.canBuffer(format: canonical, frames: limit + 1))
        XCTAssertFalse(DictationTranscriber.canBuffer(format: canonical, frames: 0))
        for (rate, channels) in [(48_000.0, UInt32(1)), (16_000.0, UInt32(2))] {
            let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: rate, channels: channels))
            XCTAssertFalse(DictationTranscriber.canBuffer(format: format, frames: 16_000))
        }
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try write([Float](repeating: 0.1, count: 48_000), to: url, sampleRate: 48_000)
        XCTAssertNil(try DictationTranscriber.loadSamples(url))
    }

    func testUnreadableAudioThrowsInsteadOfReturningPartialText() throws {
        let url = temporaryURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try Data("broken audio".utf8).write(to: url)
        XCTAssertThrowsError(try DictationTranscriber.loadSamples(url))
    }

    @MainActor
    func testCancelledTranscriptionDoesNotStartASR() async {
        let task = Task {
            try await DictationTranscriber.transcribe(temporaryURL(), using: AsrManager(config: .default))
        }
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("Cancelled transcription should not run")
        } catch {
            XCTAssertTrue(error is CancellationError)
        }
    }

    private func temporaryURL() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("koett-buffer-test-\(UUID()).wav")
    }

    private func write(_ samples: [Float], to url: URL, sampleRate: Double = 16_000) throws {
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1))
        let file = try AVAudioFile(forWriting: url, settings: format.settings)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)))
        buffer.frameLength = buffer.frameCapacity
        buffer.floatChannelData![0].update(from: samples, count: samples.count)
        try file.write(from: buffer)
    }
}

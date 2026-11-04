import AVFoundation
@testable import Koett
import XCTest

final class LocalMediaTests: XCTestCase {
    func testConversionUsesPrivateTemporaryAudioAndPreservesOriginal() async throws {
        guard FileManager.default.isExecutableFile(atPath: "/opt/homebrew/bin/ffmpeg") else {
            throw XCTSkip("FFmpeg is required for the local media integration test")
        }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("koett-local-media-\(UUID())")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let original = root.appendingPathComponent("test audio with spaces.wav")
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 44_100, channels: 2))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4_410))
        buffer.frameLength = buffer.frameCapacity
        for channel in 0..<2 { buffer.floatChannelData![channel].initialize(repeating: 0, count: 4_410) }
        do {
            let file = try AVAudioFile(forWriting: original, settings: format.settings)
            try file.write(from: buffer)
        }
        let bytes = try Data(contentsOf: original)
        let audio = try await BrowserMedia.localAudio(at: original)
        XCTAssertEqual(audio.source.platformName, "Local File")
        XCTAssertEqual(audio.source.url, original)
        XCTAssertEqual(audio.title, original.lastPathComponent)
        XCTAssertNotEqual(audio.temporaryDirectoryURL, root)
        let converted = try AVAudioFile(forReading: audio.audioURL)
        XCTAssertEqual(converted.processingFormat.sampleRate, 16_000)
        XCTAssertEqual(converted.processingFormat.channelCount, 1)
        XCTAssertGreaterThan(converted.length, 0)
        await BrowserMedia.removeTemporaryAudio(at: audio.temporaryDirectoryURL)
        XCTAssertEqual(try Data(contentsOf: original), bytes)
        XCTAssertFalse(FileManager.default.fileExists(atPath: audio.temporaryDirectoryURL.path))
    }
}

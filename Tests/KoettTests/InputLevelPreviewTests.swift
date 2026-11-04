import AppKit
import AVFoundation
@testable import Koett
import XCTest

@MainActor
final class InputLevelPreviewTests: XCTestCase {
    private final class Input: MicrophoneCapturing {
        let format = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1)!
        var error: Error?
        var starts = 0
        var stops = 0
        var onAudio: (@Sendable (AVAudioPCMBuffer) -> Void)?
        func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
            starts += 1
            self.onAudio = onAudio
        }
        func stop() { stops += 1; onAudio = nil }
    }

    func testChangingInputsAndYieldingReleasesTheOldCaptureAndClearsLevel() throws {
        let first = Input()
        let second = Input()
        let preview = InputLevelPreview { $0.id == 1 ? first : second }
        try preview.start(device: .init(id: 1, uid: "one", name: "One"))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: first.format, frameCapacity: 4))
        buffer.frameLength = 4
        buffer.floatChannelData![0].update(from: [Float](repeating: 0.5, count: 4), count: 4)
        first.onAudio?(buffer)
        XCTAssertEqual(preview.power.level, 0.5, accuracy: 0.001)
        try preview.start(device: .init(id: 2, uid: "two", name: "Two"))
        XCTAssertEqual(first.stops, 1)
        XCTAssertEqual(second.starts, 1)
        XCTAssertEqual(preview.power.level, 0)
        preview.stop()
        XCTAssertEqual(second.stops, 1)
        XCTAssertEqual(preview.power.level, 0)
        XCTAssertFalse(preview.isConnected)
    }

    func testNativeWindowLightAndDarkSnapshots() throws {
        guard let path = ProcessInfo.processInfo.environment["KOETT_PREVIEW_SNAPSHOT_DIR"] else {
            throw XCTSkip("Set KOETT_PREVIEW_SNAPSHOT_DIR for native window captures")
        }
        let root = URL(fileURLWithPath: path)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let suite = "koett-preview-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("test-history.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("test-recovery"))))
        let preview = MicrophonePreviewWindow(controller: controller)
        let content = try XCTUnwrap(preview.window?.contentView)
        for (name, appearance) in [("light", NSAppearance.Name.aqua), ("dark", .darkAqua)] {
            preview.window?.appearance = NSAppearance(named: appearance)
            content.appearance = NSAppearance(named: appearance)
            content.wantsLayer = true
            content.layer?.backgroundColor = (name == "light" ? NSColor.white : NSColor.windowBackgroundColor).cgColor
            content.layoutSubtreeIfNeeded()
            content.displayIfNeeded()
            let bitmap = try XCTUnwrap(content.bitmapImageRepForCachingDisplay(in: content.bounds))
            let context = try XCTUnwrap(NSGraphicsContext(bitmapImageRep: bitmap))
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = context
            NSAppearance(named: appearance)?.performAsCurrentDrawingAppearance {
                content.cacheDisplay(in: content.bounds, to: bitmap)
            }
            NSGraphicsContext.restoreGraphicsState()
            try XCTUnwrap(bitmap.representation(using: .png, properties: [:])).write(to: root.appendingPathComponent(name + ".png"))
        }
    }
}

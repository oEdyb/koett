import AppKit
@testable import Koett
import XCTest

final class RecordingLimitTests: XCTestCase {
    func testAllowedValuesDefaultAndInvalidPreference() throws {
        let suite = "koett-limit-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        XCTAssertEqual(RecordingLimit.allCases.map(\.rawValue), [10, 30, 60, 120])
        XCTAssertEqual(RecordingLimit.load(from: defaults).seconds, 3600)
        defaults.set(17, forKey: RecordingLimit.key)
        XCTAssertEqual(RecordingLimit.load(from: defaults), .sixty)
        for limit in RecordingLimit.allCases {
            defaults.set(limit.rawValue, forKey: RecordingLimit.key)
            XCTAssertEqual(RecordingLimit.load(from: defaults), limit)
        }
    }

    func testWarnsOnceOneMinuteBeforeLimitAndStopsOnce() {
        var deadline = RecordingDeadline(limit: .sixty)
        XCTAssertNil(deadline.update(elapsed: 600))
        XCTAssertNil(deadline.update(elapsed: 3539.9))
        XCTAssertEqual(deadline.update(elapsed: 3540), .warn)
        XCTAssertNil(deadline.update(elapsed: 3541))
        XCTAssertNil(deadline.update(elapsed: 3599.9))
        XCTAssertEqual(deadline.update(elapsed: 3600), .stop)
        XCTAssertNil(deadline.update(elapsed: 3601))
        var late = RecordingDeadline(limit: .ten)
        XCTAssertEqual(late.update(elapsed: 610), .stop)
        XCTAssertNil(late.update(elapsed: 611))
    }

    @MainActor func testSettingIsSingleChoiceAndCannotChangeActiveSession() throws {
        let suite = "koett-limit-menu-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(suite)
        defer { defaults.removePersistentDomain(forName: suite); try? FileManager.default.removeItem(at: root) }
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("history.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("recovery"))),
            availableMicrophones: { [] }, defaultMicrophoneID: { 0 })
        controller.state = .ready
        let menu = try XCTUnwrap(controller.recordingLimitMenuItem().submenu)
        XCTAssertEqual(menu.items.map(\.title), ["10 min", "30 min", "60 min", "120 min"])
        XCTAssertEqual(menu.items.filter { $0.state == .on }.map(\.tag), [60])
        controller.selectRecordingLimit(menu.items[3])
        XCTAssertEqual(RecordingLimit.load(from: defaults), .oneTwenty)
        _ = controller.beginDictationSession()
        controller.state = .recording
        controller.selectRecordingLimit(menu.items[0])
        XCTAssertEqual(RecordingLimit.load(from: defaults), .oneTwenty)
        XCTAssertEqual(controller.recordingDeadline?.limit, 7200)
        controller.state = .ready
    }

    @MainActor func testNoticeKeepsRecordingStateClockAndControls() {
        let overlay = RecordingOverlayController()
        overlay.pillView.content = .recording(paused: false)
        overlay.pillView.hasRecordingControls = true
        overlay.pillView.canPause = true
        overlay.pillView.elapsed = 3540
        overlay.showRecordingNotice("1 min left")
        XCTAssertEqual(overlay.pillView.content, .recording(paused: false))
        XCTAssertEqual(overlay.pillView.elapsed, 3540)
        XCTAssertEqual(overlay.pillView.recordingNotice, "1 min left")
        XCTAssertEqual(overlay.pillView.statusElement.accessibilityValue() as? String, "1 min left")
        XCTAssertTrue(overlay.pillView.canPause)
        XCTAssertEqual(overlay.pillView.bounds.size, NSSize(width: 300, height: 48))
        if let path = ProcessInfo.processInfo.environment["KOETT_LIMIT_ARTIFACTS"],
           let bitmap = overlay.pillView.bitmapImageRepForCachingDisplay(in: overlay.pillView.bounds) {
            overlay.pillView.cacheDisplay(in: overlay.pillView.bounds, to: bitmap)
            try? bitmap.representation(using: .png, properties: [:])?.write(to:
                URL(fileURLWithPath: path).appendingPathComponent("limit-notice.png"))
        }
    }
}

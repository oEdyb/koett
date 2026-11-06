import Foundation
@testable import Koett
import XCTest

final class RecordingSoundsTests: XCTestCase {
    func withDefaults(_ check: (UserDefaults) throws -> Void) throws {
        let name = "koett-sounds-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        try check(defaults)
    }

    func testUntouchedInstallIsSilentWithoutWritingAnExplicitChoice() throws {
        try withDefaults { defaults in
            let preferences = RecordingSoundPreferences(defaults: defaults)
            XCTAssertFalse(preferences.enabled)
            XCTAssertNil(preferences.playbackVolume)
            XCTAssertEqual(preferences.volume, 0.5)
            XCTAssertNil(defaults.object(forKey: RecordingSoundPreferences.enabledKey))
        }
    }

    func testExplicitChoicesAndVolumeSurviveReloads() throws {
        try withDefaults { defaults in
            var preferences = RecordingSoundPreferences(defaults: defaults)
            preferences.enabled = true
            preferences.volume = 0.23
            preferences.save(to: defaults)
            let restored = RecordingSoundPreferences(defaults: defaults)
            XCTAssertTrue(restored.enabled)
            XCTAssertEqual(restored.volume, 0.23)
            XCTAssertEqual(try XCTUnwrap(restored.playbackVolume), 0.23, accuracy: 0.0001)
            preferences.enabled = false
            preferences.save(to: defaults)
            XCTAssertFalse(RecordingSoundPreferences(defaults: defaults).enabled)
            XCTAssertEqual(RecordingSoundPreferences(defaults: defaults).volume, 0.23)
        }
    }

    func testVolumeIsBoundedAndZeroDoesNotPlay() throws {
        try withDefaults { defaults in
            defaults.set(true, forKey: RecordingSoundPreferences.enabledKey)
            for (input, expected) in [(-2.0, 0.0), (2.0, 1.0), (Double.nan, 0.5), (Double.infinity, 0.5)] {
                defaults.set(input, forKey: RecordingSoundPreferences.volumeKey)
                let preferences = RecordingSoundPreferences(defaults: defaults)
                XCTAssertEqual(preferences.volume, expected)
                if expected == 0 { XCTAssertNil(preferences.playbackVolume) }
            }
        }
    }

    func testStartCueIsLowerSoftAndFadesWithoutClipping() {
        let samples = RecordingStartCue.samples()
        XCTAssertEqual(samples.count, 7_056)
        XCTAssertEqual(samples.first, 0)
        XCTAssertEqual(samples.last, 0)
        XCTAssertLessThan(samples.map { abs(Int($0)) }.max()!, Int(Double(Int16.max) * 0.24))
        let begin = RecordingStartCue.sampleRate / 50
        let end = RecordingStartCue.sampleRate * 3 / 25
        let crossings = (begin..<end).filter { samples[$0 - 1] <= 0 && samples[$0] > 0 }.count
        XCTAssertEqual(Double(crossings) / 0.1, 440, accuracy: 10)
        let data = RecordingStartCue.waveData()
        XCTAssertEqual(data.count, 44 + samples.count * 2)
        XCTAssertEqual(String(decoding: data.prefix(4), as: UTF8.self), "RIFF")
        XCTAssertEqual(String(decoding: data[8..<12], as: UTF8.self), "WAVE")
    }

    @MainActor func testMutedSetupDoesNotPrepareThePlaybackDevice() throws {
        try withDefaults { defaults in
            let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            defer { try? FileManager.default.removeItem(at: root) }
            let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
                transcriptPersistence: TranscriptPersistence(
                    history: TranscriptStore(fileURL: root.appendingPathComponent("History.md")),
                    recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))))
            controller.prepareRecordingSounds()
            XCTAssertNil(controller.startSound)
            XCTAssertNil(controller.stopSound)
            XCTAssertNil(controller.recordingSoundError)
        }
    }
}

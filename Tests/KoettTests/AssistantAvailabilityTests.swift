import AppKit
@testable import Koett
import XCTest

@MainActor
final class AssistantAvailabilityTests: XCTestCase {
    func testMenuHasNoAskProviderVoiceOrKeyActions() throws {
        let controller = try makeController()
        let item = NSStatusBar.system.statusItem(withLength: 24)
        controller.statusItem = item
        defer {
            NSStatusBar.system.removeStatusItem(item)
            controller.statusItem = nil
        }
        controller.state = .ready
        func titles(_ menu: NSMenu) -> [String] {
            menu.items.flatMap { [$0.title] + ($0.submenu.map(titles) ?? []) }
        }
        let allTitles = titles(try XCTUnwrap(item.menu))
        for word in ["Ask", "Provider", "API Key", "Speak Replies", "Voice", "Assistant"] {
            XCTAssertFalse(allTitles.contains { $0.contains(word) }, word)
        }
        XCTAssertTrue(allTitles.contains("History…"))
        XCTAssertTrue(allTitles.contains("Draw & Pin While Dictating"))
    }

    func testSavedAskShortcutAndSpeechCannotStartAskOrChangePreferences() throws {
        let controller = try makeController()
        controller.state = .ready
        let savedShortcut = controller.assistantShortcut
        let savedSpeech = controller.assistantSpeechEnabled
        let savedVoice = controller.assistantVoice
        controller.askAboutScreen()
        controller.toggleAssistantSpeech()
        controller.setAssistantAPIKey()
        controller.setCartesiaAPIKey()
        controller.setAssistantModel()
        controller.bindAssistantShortcut()
        controller.handleShortcutEvent(type: .keyDown,
            keyCode: try XCTUnwrap(savedShortcut.keyCodes.first), modifierFlags: savedShortcut.modifiers.rawValue,
            eventTimestamp: 1, isRepeat: false)
        XCTAssertEqual(controller.state, .ready)
        XCTAssertNil(controller.recorder)
        XCTAssertNil(controller.recordingURL)
        XCTAssertNil(controller.assistantWork.captureTask)
        XCTAssertEqual(controller.assistantShortcut, savedShortcut)
        XCTAssertEqual(controller.assistantSpeechEnabled, savedSpeech)
        XCTAssertEqual(controller.assistantVoice, savedVoice)
        XCTAssertFalse(controller.assistantSpeech.hasPreparedAudio)
    }

    private func makeController() throws -> KoettController {
        let suite = "koett-disabled-ask-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(suite)
        addTeardownBlock {
            UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite)
            try? FileManager.default.removeItem(at: root)
        }
        defaults.set(true, forKey: "assistantSpeechEnabled")
        return try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("history.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("recovery"))))
    }
}

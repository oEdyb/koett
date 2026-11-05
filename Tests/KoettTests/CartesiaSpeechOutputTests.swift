@testable import Koett
import Foundation
import XCTest

final class CartesiaSpeechOutputTests: XCTestCase {
    @MainActor
    func testUnusedSpeechAndCancellationDoNotPrepareAudio() async {
        let output = CartesiaSpeechOutput()
        XCTAssertFalse(output.hasPreparedAudio)
        output.stop()
        output.stop()
        XCTAssertFalse(output.hasPreparedAudio)
    }

    @MainActor
    func testMalformedAudioDoesNotOpenThePlaybackDevice() async {
        let output = CartesiaSpeechOutput()
        XCTAssertThrowsError(try output.play(Data([1])))
        XCTAssertThrowsError(try output.play(Data()))
        XCTAssertFalse(output.hasPreparedAudio)
    }

    @MainActor
    func testControllerInitializationLeavesAudioBehindPermissionOnboarding() async throws {
        let name = "koett-unprepared-audio-" + UUID().uuidString
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("Transcripts.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("Failed"))))
        XCTAssertNil(controller.startSound)
        XCTAssertNil(controller.stopSound)
        XCTAssertFalse(controller.assistantSpeech.hasPreparedAudio)
        controller.assistantSpeech.stop()
        XCTAssertFalse(controller.assistantSpeech.hasPreparedAudio)
    }

    func testStreamsCompleteSentencesInOrder() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(buffer.append("Yeah, this "), [])
        XCTAssertEqual(
            buffer.append("works. It feels fast!"),
            ["Yeah, this works.", "It feels fast!"]
        )
        XCTAssertEqual(buffer.finish(), [])
    }

    func testStopsBeforeDetailedMarkdown() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("Yes, use Cartesia.\n\n## Details\nDo more work."),
            ["Yes, use Cartesia."]
        )
        XCTAssertEqual(buffer.finish(), [])
    }

    func testRemovesInlineMarkdownFromSpeech() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("Use **Cartesia**. Read the [docs](https://cartesia.ai)."),
            ["Use Cartesia.", "Read the docs."]
        )
    }

    func testSpeaksAtMostThreeSentences() {
        var buffer = SpokenLeadBuffer()

        XCTAssertEqual(
            buffer.append("One. Two. Three. Four."),
            ["One.", "Two.", "Three."]
        )
        XCTAssertEqual(buffer.finish(), [])
    }
}

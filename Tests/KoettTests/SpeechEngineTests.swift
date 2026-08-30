@testable import Koett
import FluidAudio
import XCTest

final class SpeechEngineTests: XCTestCase {
    func testKeepsParakeetV2AsDefault() {
        XCTAssertEqual(
            SpeechEngine.selected(arguments: [], savedRawValue: nil),
            .parakeet
        )
    }

    func testRestoresParakeetV3Selection() {
        XCTAssertEqual(
            SpeechEngine.selected(
                arguments: [],
                savedRawValue: SpeechEngine.parakeetV3.rawValue
            ),
            .parakeetV3
        )
    }

    func testRestoresLegacySavedSelections() {
        XCTAssertEqual(
            SpeechEngine.selected(
                arguments: [],
                savedRawValue: SpeechEngine.parakeet.rawValue
            ),
            .parakeet
        )
        XCTAssertEqual(
            SpeechEngine.selected(
                arguments: [],
                savedRawValue: SpeechEngine.nemotron.rawValue
            ),
            .nemotron
        )
    }

    func testUnknownSavedEngineFallsBackToParakeetV2() {
        XCTAssertEqual(
            SpeechEngine.selected(arguments: [], savedRawValue: "unknown"),
            .parakeet
        )
    }

    func testCommandLineCanSelectEachEngine() {
        XCTAssertEqual(
            SpeechEngine.selected(arguments: ["--parakeet"], savedRawValue: nil),
            .parakeet
        )
        XCTAssertEqual(
            SpeechEngine.selected(
                arguments: ["--parakeet-v3"],
                savedRawValue: SpeechEngine.nemotron.rawValue
            ),
            .parakeetV3
        )
        XCTAssertEqual(
            SpeechEngine.selected(arguments: ["--nemotron"], savedRawValue: nil),
            .nemotron
        )
    }

    func testMapsParakeetVersionsToFluidAudio() {
        assertVersion(.v2, for: .parakeet)
        assertVersion(.v3, for: .parakeetV3)
        assertVersion(.v2, for: .nemotron)
    }

    func testUsesDocumentedLongAudioConfigurationForV3() {
        XCTAssertTrue(SpeechEngine.parakeet.asrConfiguration.melChunkContext)
        XCTAssertFalse(SpeechEngine.parakeetV3.asrConfiguration.melChunkContext)
        XCTAssertTrue(SpeechEngine.nemotron.asrConfiguration.melChunkContext)
    }

    func testExposesClearMenuTitlesAndRelaunchArguments() {
        XCTAssertEqual(
            SpeechEngine.parakeet.menuTitle,
            "Model: Parakeet v2 (English)"
        )
        XCTAssertEqual(
            SpeechEngine.parakeetV3.menuTitle,
            "Model: Parakeet v3 (Multilingual)"
        )
        XCTAssertEqual(SpeechEngine.parakeetV3.launchArgument, "--parakeet-v3")
    }

    func testLabelsTheParakeetModelThatActuallyLoads() {
        XCTAssertEqual(SpeechEngine.parakeet.parakeetModelName, "Parakeet v2")
        XCTAssertEqual(SpeechEngine.parakeetV3.parakeetModelName, "Parakeet v3")
        XCTAssertEqual(SpeechEngine.nemotron.parakeetModelName, "Parakeet v2")
    }

    private func assertVersion(
        _ expected: AsrModelVersion,
        for engine: SpeechEngine,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        switch (expected, engine.parakeetVersion) {
        case (.v2, .v2), (.v3, .v3):
            return
        default:
            XCTFail("Unexpected FluidAudio model version.", file: file, line: line)
        }
    }
}

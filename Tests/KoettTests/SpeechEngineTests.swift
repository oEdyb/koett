import Foundation
import FluidAudio
@testable import Koett
import XCTest

final class SpeechEngineTests: XCTestCase {
    func testEveryModelSurvivesRelaunchWithoutChangingTheSavedChoice() throws {
        let name = "koett-models-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        XCTAssertEqual(SpeechEngine.selected(arguments: [], defaults: defaults), .parakeet)
        defaults.set(SpeechEngine.parakeetV3.rawValue, forKey: "speechEngine")
        for engine in SpeechEngine.allCases {
            XCTAssertEqual(SpeechEngine.selected(arguments: [engine.launchArgument], defaults: defaults), engine)
            XCTAssertEqual(defaults.string(forKey: "speechEngine"), SpeechEngine.parakeetV3.rawValue)
        }
        XCTAssertEqual(SpeechEngine.selected(arguments: [], defaults: defaults), .parakeetV3)
    }

    func testModelVersionsAndNemotronRecoveryRemainExplicit() {
        if case .v2 = SpeechEngine.parakeet.parakeetVersion {} else { XCTFail("Default must remain v2") }
        if case .v2 = SpeechEngine.nemotron.parakeetVersion {} else { XCTFail("Recovery must remain v2") }
        if case .v3 = SpeechEngine.parakeetV3.parakeetVersion {} else { XCTFail("v3 must load v3") }
        if case .tdtCtc110m = SpeechEngine.parakeet110m.parakeetVersion {} else { XCTFail("110M must use its own decoder") }
    }
}

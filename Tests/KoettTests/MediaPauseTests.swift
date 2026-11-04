import AppKit
@testable import Koett
import XCTest

final class MediaPauseTests: XCTestCase {
    func testOnlyExplicitPauseIsAllowed() {
        for label in ["Play", "Play/Pause", "", "Resume", "Stop", "Toggle playback"] {
            XCTAssertFalse(MediaPausePolicy.canPause(bundleID: "com.spotify.client", role: kAXButtonRole, label: label, classes: []))
        }
        XCTAssertTrue(MediaPausePolicy.canPause(bundleID: "com.spotify.client", role: kAXButtonRole, label: "Pause", classes: []))
        XCTAssertFalse(MediaPausePolicy.canPause(bundleID: "unknown", role: kAXButtonRole, label: "Pause", classes: []))
    }

    func testBrowserRequiresTheYouTubePlayerControl() {
        XCTAssertFalse(MediaPausePolicy.canPause(bundleID: "com.google.Chrome", role: kAXButtonRole, label: "Pause", classes: []))
        XCTAssertTrue(MediaPausePolicy.canPause(bundleID: "com.google.Chrome", role: kAXButtonRole, label: "Pause (k)", classes: ["ytp-play-button"]))
        XCTAssertFalse(MediaPausePolicy.canPause(bundleID: "com.google.Chrome", role: kAXButtonRole, label: "Play (k)", classes: ["ytp-play-button"]))
        XCTAssertFalse(MediaPausePolicy.canPause(bundleID: "com.google.Chrome", role: kAXTextFieldRole, label: "Pause (k)", classes: ["ytp-play-button"]))
    }
}

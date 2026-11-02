import AppKit
import AVFoundation
@testable import Koett
import SwiftUI
import XCTest

@MainActor
final class PermissionsTests: XCTestCase {
    func testMicrophoneRowFollowsTheSystemStatus() {
        XCTAssertEqual(PermissionRow.make(.microphone, microphone: .notDetermined).action, .allow)
        XCTAssertEqual(PermissionRow.make(.microphone, microphone: .denied).action, .openSettings)
        XCTAssertEqual(PermissionRow.make(.microphone, microphone: .restricted).action, .openSettings)
        let allowed = PermissionRow.make(.microphone, microphone: .authorized)
        XCTAssertTrue(allowed.allowed)
        XCTAssertNil(allowed.action)
    }

    func testFirstClickPromptsAndLaterClicksOpenTheExactPane() {
        XCTAssertEqual(PermissionRow.make(.accessibility, granted: false, asked: false).action, .allow)
        XCTAssertEqual(PermissionRow.make(.accessibility, granted: false, asked: true).action, .openSettings)
        XCTAssertNil(PermissionRow.make(.accessibility, granted: true, asked: true).action)
    }

    func testOnlyScreenRecordingOffersRelaunchAndOnlyAfterAsking() {
        XCTAssertFalse(PermissionRow.make(.screenRecording, granted: false, asked: false).offersRelaunch)
        XCTAssertTrue(PermissionRow.make(.screenRecording, granted: false, asked: true).offersRelaunch)
        XCTAssertFalse(PermissionRow.make(.screenRecording, granted: true, asked: true).offersRelaunch)
        XCTAssertFalse(PermissionRow.make(.accessibility, granted: false, asked: true).offersRelaunch)
    }

    func testSettingsLinksOpenTheExactPrivacyPanes() {
        XCTAssertEqual(
            PermissionKind.allCases.map(\.settingsURL.absoluteString),
            [
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            ]
        )
        XCTAssertEqual(PermissionKind.allCases.filter(\.isRequired), [.microphone, .accessibility])
    }

    func testAskedStateIsRememberedPerPermission() throws {
        let name = "koett-permissions-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        addTeardownBlock { UserDefaults(suiteName: name)?.removePersistentDomain(forName: name) }
        let center = PermissionCenter(defaults: defaults)
        XCTAssertFalse(center.asked(.screenRecording))
        defaults.set(true, forKey: PermissionKind.screenRecording.askedDefaultsKey)
        XCTAssertTrue(center.asked(.screenRecording))
        XCTAssertFalse(center.asked(.accessibility))
    }

    /// Writes the window in each state when KOETT_PERMISSIONS_SNAPSHOT_DIR is set.
    func testRenderPermissionStates() throws {
        guard let directory = ProcessInfo.processInfo.environment["KOETT_PERMISSIONS_SNAPSHOT_DIR"] else {
            throw XCTSkip("Set KOETT_PERMISSIONS_SNAPSHOT_DIR to render the permissions window.")
        }
        let folder = URL(fileURLWithPath: directory, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let states: [(String, [PermissionRow])] = [
            ("first-launch", [
                .make(.microphone, microphone: .notDetermined),
                .make(.accessibility),
                .make(.screenRecording),
            ]),
            ("screen-needs-relaunch", [
                .make(.microphone, microphone: .authorized),
                .make(.accessibility, granted: true),
                .make(.screenRecording, granted: false, asked: true),
            ]),
            ("all-allowed", [
                .make(.microphone, microphone: .authorized),
                .make(.accessibility, granted: true),
                .make(.screenRecording, granted: true),
            ]),
        ]
        for (appearanceName, appearance) in [("dark", NSAppearance.Name.darkAqua), ("light", .aqua)] {
            for (name, rows) in states {
                let model = PermissionsModel(center: PermissionCenter())
                model.rows = rows
                let view = NSHostingView(rootView: PermissionsView(model: model, close: {}).background(Color(nsColor: .windowBackgroundColor)))
                view.appearance = NSAppearance(named: appearance)
                view.frame = NSRect(origin: .zero, size: view.fittingSize)
                view.layoutSubtreeIfNeeded()
                let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
                view.cacheDisplay(in: view.bounds, to: rep)
                let png = try XCTUnwrap(rep.representation(using: .png, properties: [:]))
                try png.write(to: folder.appendingPathComponent("\(name)-\(appearanceName).png"))
            }
        }
    }
}

import AppKit
@testable import Koett
import XCTest

/// The overlay's display handling, with fake displays, no visible panels and no
/// real capture.
@MainActor
final class SketchOverlayDisplayTests: XCTestCase {
    private let left = SketchOverlay.Display(id: 1, frame: CGRect(x: 0, y: 0, width: 800, height: 600), scale: 1)
    private let right = SketchOverlay.Display(id: 2, frame: CGRect(x: 800, y: 0, width: 800, height: 600), scale: 1)

    @MainActor
    private final class Fake {
        var displays: [SketchOverlay.Display] = []
        var modifiers: NSEvent.ModifierFlags = []
        var pointer = CGPoint.zero
        var captured: [CGDirectDisplayID] = []
        var connected: Set<CGDirectDisplayID> = []
        var updates: [SketchUpdate] = []
        var panels: [NSPanel] = []
    }

    private func overlay(_ fake: Fake) -> SketchOverlay {
        SketchOverlay(environment: SketchOverlay.Environment(
            displays: { fake.displays },
            capture: { id, _ in
                fake.captured.append(id)
                guard fake.connected.contains(id) else { throw CocoaError(.fileNoSuchFile) }
                return try XCTUnwrap(SketchSheet.canvas(width: 8, height: 8)?.makeImage())
            },
            modifiers: { fake.modifiers },
            pointer: { fake.pointer },
            showPanel: { fake.panels.append($0) }
        ))
    }

    private func start(_ overlay: SketchOverlay, _ fake: Fake, frames: SketchFrames = SketchFrames()) {
        frames.begin(1)
        overlay.start(SketchOverlay.Session(
            id: 1, key: .shift, frames: frames, audioClock: { 1 }, includeSelection: false,
            onUpdate: { fake.updates.append($0) }
        ), startedAt: 0)
    }

    private func draw(_ overlay: SketchOverlay, _ fake: Fake, from: CGPoint) {
        fake.modifiers = .shift
        for step in 0..<3 {
            fake.pointer = CGPoint(x: from.x + CGFloat(step) * 10, y: from.y)
            overlay.tick()
        }
    }

    private func settle() async {
        try? await Task.sleep(for: .milliseconds(50))
    }

    func testUnpluggedMonitorClosesItsSketchAsAFailedCapture() async {
        let fake = Fake()
        fake.displays = [left, right]
        fake.connected = [1]
        let overlay = overlay(fake)
        start(overlay, fake)
        defer { overlay.discard() }
        draw(overlay, fake, from: CGPoint(x: 900, y: 100))

        fake.displays = [left]
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
        await settle()

        XCTAssertEqual(fake.captured, [2], "the open sketch is captured from the display it was drawn on")
        XCTAssertEqual(overlay.failedCaptures, 1, "not silently dropped")
        XCTAssertEqual(fake.updates.last, .removed(index: 1))
        XCTAssertEqual(fake.panels.count, 3, "two at start, one rebuilt")
    }

    func testSketchKeepsDrawingWhenTheLayoutDidNotChange() async {
        let fake = Fake()
        fake.displays = [left, right]
        let overlay = overlay(fake)
        start(overlay, fake)
        defer { overlay.discard() }
        draw(overlay, fake, from: CGPoint(x: 100, y: 100))
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
        await settle()
        XCTAssertTrue(fake.captured.isEmpty, "the notification also fires when no display moved")
        XCTAssertEqual(fake.panels.count, 2)
    }

    func testInkLandsOnTheRightDisplayAfterThePrimaryDisplayChanges() async {
        let fake = Fake()
        fake.displays = [left, right]
        fake.connected = [1, 2]
        let overlay = overlay(fake)
        start(overlay, fake)
        defer { overlay.discard() }

        // Display 2 becomes primary: both origins move.
        fake.displays = [
            SketchOverlay.Display(id: 2, frame: CGRect(x: 0, y: 0, width: 800, height: 600), scale: 1),
            SketchOverlay.Display(id: 1, frame: CGRect(x: -800, y: 0, width: 800, height: 600), scale: 1),
        ]
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)

        // The same global x now belongs to display 2.
        draw(overlay, fake, from: CGPoint(x: 100, y: 100))
        fake.modifiers = []
        overlay.tick()
        await settle()
        XCTAssertEqual(fake.captured, [2], "with stale frames this ink would land on display 1")
        XCTAssertEqual(fake.panels.last?.frame.minX, -800)
    }

    func testDisplayChangeAfterStopLeavesThePanelsClosed() {
        let fake = Fake()
        fake.displays = [left]
        let overlay = overlay(fake)
        start(overlay, fake)
        draw(overlay, fake, from: CGPoint(x: 100, y: 100))
        overlay.finish()
        defer { overlay.discard() }
        fake.displays = [left, right]
        overlay.displaysChanged()
        XCTAssertEqual(fake.panels.count, 1)
    }

    func testAbandonedCapturesCountAsFailed() {
        let fake = Fake()
        fake.displays = [left]
        let overlay = overlay(fake)
        start(overlay, fake)
        defer { overlay.discard() }
        draw(overlay, fake, from: CGPoint(x: 100, y: 100))
        overlay.finish()
        // The capture task has not run yet: the main actor is still here.
        overlay.abandonPendingCaptures(1)
        XCTAssertEqual(overlay.failedCaptures, 1)
    }

    func testInkPanelsFollowThePillAcrossSpacesAndFullscreenApps() throws {
        let fake = Fake()
        fake.displays = [left]
        let overlay = overlay(fake)
        start(overlay, fake)
        defer { overlay.discard() }
        let panel = try XCTUnwrap(fake.panels.first)
        XCTAssertEqual(panel.collectionBehavior, PillPanel.overlayBehavior)
        XCTAssertTrue(panel.collectionBehavior.contains(.canJoinAllApplications))
        XCTAssertTrue(panel.collectionBehavior.contains(.fullScreenAuxiliary))
        XCTAssertEqual(panel.level, .screenSaver)
    }

    func testSketchWarningReplacesOnlyThisDictationsPill() {
        let controller = RecordingOverlayController()
        controller.showSketchWarning("Sketches not pasted")
        XCTAssertNil(controller.latestOutcome, "another message or dictation owns the pill")
        controller.showTranscribing()
        controller.showSketchWarning("Sketches not pasted")
        XCTAssertEqual(controller.latestOutcome, .failure("Sketches not pasted"))
    }
}

import AppKit
import CoreGraphics
import Foundation
@testable import Koett
import XCTest

@MainActor
final class PasteReadinessTests: XCTestCase {
    private let saved = TranscriptPersistence.Outcome(historySaved: true, recoveryURL: nil, lastTranscriptSaved: true, warning: nil)

    func testHeldStopModifierIsRetriedThenPastesBeforeOneSave() async throws {
        var time: TimeInterval = 0
        var events: [String] = []
        var records: [PasteDiagnostic] = []
        let coordinator = PasteCoordinator(snapshot: {
            PasteSnapshot(physicalFlags: time < 0.1 ? CGEventFlags.maskAlternate.rawValue : 0)
        }, postEvent: { _ in events.append("paste"); return true }, record: { records.append($0) },
           clock: { time }, pause: { time += 0.02 })
        XCTAssertEqual(PasteSnapshot(physicalFlags: CGEventFlags.maskAlternate.rawValue).reason, .modifiersHeld,
                       "The old one-shot guard rejects this snapshot even though the key releases next")
        let ready = await coordinator.prepare(stage: .dictation)
        _ = try DictationDelivery.deliver("Test fixture", writeToClipboard: { _ in events.append("clipboard"); return true },
            postPaste: { ready && coordinator.post(flags: .maskCommand) }, persist: { events.append("save"); return self.saved })
        XCTAssertEqual(events, ["clipboard", "paste", "save"])
        XCTAssertEqual(records.count, 1)
        XCTAssertEqual(records.first?.outcome, "posted")
        XCTAssertEqual(records.first?.waitedFor, .modifiersHeld)
        XCTAssertGreaterThanOrEqual(records.first?.waitedMilliseconds ?? 0, 100)
    }

    func testTemporaryAXWindowTimeoutRetriesWithoutDroppingDictation() async {
        var time: TimeInterval = 0
        let coordinator = PasteCoordinator(snapshot: {
            PasteSnapshot(targetFailure: time < 0.08 ? .targetNotReady : nil)
        }, postEvent: { _ in true }, clock: { time }, pause: { time += 0.02 })
        let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertTrue(ready)
        XCTAssertTrue(coordinator.post(flags: .maskCommand))
        XCTAssertNil(coordinator.lastFailure)
    }

    func testChangingFocusWhileWaitingNeverPostsAndSavesOnce() async {
        var time: TimeInterval = 0
        var posts = 0
        var saves = 0
        let coordinator = PasteCoordinator(snapshot: {
            time == 0 ? PasteSnapshot(physicalFlags: CGEventFlags.maskShift.rawValue)
                : PasteSnapshot(targetFailure: .focusChanged)
        }, postEvent: { _ in posts += 1; return true }, clock: { time }, pause: { time += 0.02 })
        let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertFalse(ready)
        do {
            _ = try DictationDelivery.deliver("Test fixture", writeToClipboard: { _ in true },
                postPaste: { ready && coordinator.post(flags: .maskCommand) }, persist: { saves += 1; return self.saved })
            XCTFail("A target change must reach recovery")
        } catch {}
        XCTAssertEqual(posts, 0)
        XCTAssertEqual(saves, 1)
        XCTAssertEqual(coordinator.lastFailure, .focusChanged)
    }

    func testTimeoutAndPermissionDenialHaveExplicitReasons() async {
        for reason in [PasteBlockReason.modifiersHeld, .targetNotReady, .accessibilityDenied, .windowChanged] {
            var time: TimeInterval = 0
            var records: [PasteDiagnostic] = []
            let coordinator = PasteCoordinator(snapshot: {
                if reason == .modifiersHeld { return PasteSnapshot(physicalFlags: CGEventFlags.maskControl.rawValue) }
                if reason == .accessibilityDenied { return PasteSnapshot(access: false) }
                return PasteSnapshot(targetFailure: reason)
            }, postEvent: { _ in XCTFail("Blocked paste must not post"); return false }, record: { records.append($0) },
               clock: { time }, pause: { time += 0.02 })
            let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertFalse(ready)
            XCTAssertEqual(coordinator.lastFailure, reason)
            XCTAssertEqual(records.last?.reason, reason)
            XCTAssertEqual(records.last?.outcome, "blocked")
            if reason.retryable { XCTAssertGreaterThanOrEqual(time, PasteCoordinator.timeout) }
            else { XCTAssertEqual(time, 0) }
        }
    }

    func testSyntheticCombinedFlagsDoNotCountAsHeldHardwareModifiers() async {
        let coordinator = PasteCoordinator(snapshot: { PasteSnapshot(combinedFlags: CGEventFlags.maskCommand.rawValue) }, postEvent: { _ in true })
        let ready = await coordinator.prepare(stage: .sketchText)
        XCTAssertTrue(ready)
        XCTAssertTrue(coordinator.post(flags: .maskCommand))
    }

    func testOwnPasteEventsUseAnIndependentSourceState() throws {
        let events = try XCTUnwrap(PasteEvents.make(flags: .maskCommand))
        let source = try XCTUnwrap(CGEventSource(event: events.down))
        XCTAssertNotEqual(source.sourceStateID, .hidSystemState)
        XCTAssertNotEqual(source.sourceStateID, .combinedSessionState)
        XCTAssertEqual(events.down.flags, .maskCommand)
        XCTAssertTrue(events.up.flags.isEmpty)
        XCTAssertEqual(events.down.type, .keyDown)
        XCTAssertEqual(events.up.type, .keyUp)
    }

    func testPasteChordEndsBeforePreparingTheSketchTextStage() async throws {
        for flags in [CGEventFlags.maskCommand, .maskControl] {
            let events = try XCTUnwrap(PasteEvents.make(flags: flags))
            XCTAssertEqual(events.down.flags, flags)
            let coordinator = PasteCoordinator(
                snapshot: { PasteSnapshot(physicalFlags: events.up.flags.rawValue) },
                postEvent: { _ in true }
            )
            let ready = await coordinator.prepare(stage: .sketchText)
            XCTAssertTrue(ready, "The image chord must finish before text readiness is checked")
            XCTAssertTrue(coordinator.post(flags: .maskCommand))
        }
    }

    func testFocusIsRecheckedImmediatelyBeforePosting() async {
        var changed = false
        let coordinator = PasteCoordinator(snapshot: { PasteSnapshot(targetFailure: changed ? .focusChanged : nil) },
            postEvent: { _ in XCTFail("Changed target must never receive an event"); return false })
        let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertTrue(ready)
        changed = true
        XCTAssertFalse(coordinator.post(flags: .maskCommand))
        XCTAssertEqual(coordinator.lastFailure, .focusChanged)
    }

    func testCancellationDuringReadinessWaitDoesNotPost() async {
        let coordinator = PasteCoordinator(snapshot: { PasteSnapshot(physicalFlags: CGEventFlags.maskShift.rawValue) },
            postEvent: { _ in XCTFail("Cancelled dictation must not post"); return false },
            pause: { throw CancellationError() })
        let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertFalse(ready)
        XCTAssertEqual(coordinator.lastFailure, .cancelled)
    }

    func testFailurePillKeepsTheReasonInItsPersistentMenuOutcome() {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let overlay = RecordingOverlayController()
        defer { overlay.stop() }
        overlay.showTranscribing()
        XCTAssertTrue(overlay.showNotPasted(text: "Test fixture", reason: PasteBlockReason.modifiersHeld.message, pasteboard: board))
        XCTAssertTrue(overlay.latestOutcome?.message.contains("Modifier keys still held") == true)
    }

    func testReasonLogUsesPrivateBoundedFilesWithoutTranscriptContents() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("koett-paste-log-\(UUID())")
        defer { try? FileManager.default.removeItem(at: directory) }
        let diagnostics = PasteDiagnostics(directory: directory)
        let coordinator = PasteCoordinator(snapshot: { PasteSnapshot(access: false, expectedApp: "test.editor") },
            postEvent: { _ in false }, record: diagnostics.record)
        let ready = await coordinator.prepare(stage: .dictation)
        XCTAssertFalse(ready)
        await diagnostics.flush()
        let url = directory.appendingPathComponent("Paste.jsonl")
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let entry = try decoder.decode(PasteDiagnostic.self, from: data.dropLast())
        XCTAssertEqual(entry.reason, .accessibilityDenied)
        XCTAssertEqual(entry.expectedApp, "test.editor")
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        XCTAssertEqual((attributes[.posixPermissions] as? NSNumber)?.intValue, 0o600)
        try Data(repeating: 32, count: PasteDiagnostics.maximumBytes).write(to: url)
        diagnostics.record(entry)
        await diagnostics.flush()
        XCTAssertTrue(FileManager.default.fileExists(atPath: directory.appendingPathComponent("Paste.previous.jsonl").path))
        XCTAssertLessThan(try Data(contentsOf: url).count, PasteDiagnostics.maximumBytes)
    }
}

@MainActor
final class SketchReadinessTests: XCTestCase {
    func testNewCopyWhileWaitingForTextReadinessWinsWithoutAnotherPaste() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let png = try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 3, height: 3)?.makeImage())))
        var saves = 0
        var textPosts = 0
        var failure: PasteBlockReason?
        do {
            _ = try await SketchPaste.deliver(png: png, text: "Test fixture", pasteboard: board,
                postImage: { _ = board.data(forType: .png); return true },
                postText: { textPosts += 1; return true }, prepareText: {
                    await Task.yield()
                    board.clearContents()
                    _ = board.setString("New test copy", forType: .string)
                    return true
                }, clipboardFailure: { failure = $0 }, persist: {
                    saves += 1
                    return .init(historySaved: true, recoveryURL: nil, lastTranscriptSaved: true, warning: nil)
                })
            XCTFail("New clipboard ownership must enter recovery")
        } catch {}
        XCTAssertEqual(textPosts, 0)
        XCTAssertEqual(saves, 1)
        XCTAssertEqual(failure, .clipboardChanged)
        XCTAssertEqual(board.string(forType: .string), "New test copy")
    }

    func testRejectedImagePreflightPreservesTheBundleAndSavesOnce() async throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let png = try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 3, height: 3)?.makeImage())))
        var posts = 0
        var saves = 0
        do {
            _ = try await SketchPaste.deliver(png: png, text: "Test fixture", pasteboard: board,
                postImage: { posts += 1; return true }, postText: { posts += 1; return true },
                prepareImage: { false }, persist: {
                    saves += 1
                    return .init(historySaved: true, recoveryURL: nil, lastTranscriptSaved: true, warning: nil)
                })
            XCTFail("Failed readiness must enter recovery")
        } catch {}
        XCTAssertEqual(posts, 0)
        XCTAssertEqual(saves, 1)
        XCTAssertEqual(board.data(forType: .png), png)
        XCTAssertEqual(board.string(forType: .string), "Test fixture")
        XCTAssertNotNil(board.string(forType: .html))
    }
}

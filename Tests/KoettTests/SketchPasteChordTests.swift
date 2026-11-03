import AppKit
@testable import Koett
import XCTest

final class SketchPasteChordTests: XCTestCase {
    private func defaults() throws -> UserDefaults {
        let name = "koett-chord-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        addTeardownBlock { UserDefaults(suiteName: name)?.removePersistentDomain(forName: name) }
        return defaults
    }

    func testTerminalsGetControlVAndOtherAppsCommandV() {
        for terminal in [
            "com.apple.Terminal", "com.googlecode.iterm2", "com.mitchellh.ghostty", "net.kovidgoyal.kitty",
            "com.github.wez.wezterm", "org.alacritty", "dev.warp.Warp-Stable",
        ] {
            XCTAssertEqual(SketchPasteChord.forApp(terminal, overrides: [:]), .control, terminal)
        }
        for app in ["com.apple.Notes", "com.tinyspeck.slackmacgap", "com.microsoft.VSCode", "com.t3tools.t3code"] {
            XCTAssertEqual(SketchPasteChord.forApp(app, overrides: [:]), .command, app)
        }
        XCTAssertEqual(SketchPasteChord.forApp(nil, overrides: [:]), .command)
    }

    func testChordFlagsNeverMixCommandAndControl() {
        XCTAssertEqual(SketchPasteChord.control.flags, .maskControl)
        XCTAssertEqual(SketchPasteChord.command.flags, .maskCommand)
    }

    func testPerAppOverrideTurnsControlVOnForAnEditorTerminal() throws {
        let defaults = try defaults()
        SketchPasteChord.toggle("com.microsoft.VSCode", in: defaults)
        XCTAssertEqual(SketchPasteChord.forApp("com.microsoft.VSCode", overrides: SketchPasteChord.overrides(in: defaults)), .control)
        SketchPasteChord.toggle("com.microsoft.VSCode", in: defaults)
        XCTAssertEqual(SketchPasteChord.forApp("com.microsoft.VSCode", overrides: SketchPasteChord.overrides(in: defaults)), .command)
        XCTAssertTrue(SketchPasteChord.overrides(in: defaults).isEmpty, "back to detection leaves no override")
    }

    func testPerAppOverrideCanTurnControlVOffForATerminal() throws {
        let defaults = try defaults()
        SketchPasteChord.toggle("com.googlecode.iterm2", in: defaults)
        XCTAssertEqual(SketchPasteChord.forApp("com.googlecode.iterm2", overrides: SketchPasteChord.overrides(in: defaults)), .command)
        XCTAssertEqual(SketchPasteChord.forApp("com.apple.Terminal", overrides: SketchPasteChord.overrides(in: defaults)), .control)
    }

    @MainActor
    func testSheetIsPastedWithTheChosenChordAndReadBeforeTheText() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let png = try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 2, height: 2)?.makeImage())))
        var posted: [CGEventFlags] = []
        var read: Data?
        let started = ContinuousClock.now
        let result = await SketchPaste.paste(png: png, pasteboard: pasteboard) {
            posted.append(SketchPasteChord.forApp("com.mitchellh.ghostty", overrides: [:]).flags)
            read = pasteboard.data(forType: .png)
            return true
        }
        XCTAssertEqual(result, .read)
        XCTAssertEqual(posted, [.maskControl])
        XCTAssertEqual(read, png, "a terminal agent reading PNG gets the sheet")
        XCTAssertLessThan(ContinuousClock.now - started, .seconds(1), "the text follows the read, not the 1.5 s timeout")
    }

    @MainActor
    private func samplePNG() throws -> Data {
        try XCTUnwrap(SketchSheet.png(XCTUnwrap(SketchSheet.canvas(width: 2, height: 2)?.makeImage())))
    }

    @MainActor
    func testUnreadSheetIsReportedAsUnread() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let result = await SketchPaste.paste(png: try samplePNG(), pasteboard: pasteboard) { true }
        XCTAssertEqual(result, .unread)
    }

    @MainActor
    func testFailedPasteKeyIsReportedAsNotPasted() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let result = await SketchPaste.paste(png: try samplePNG(), pasteboard: pasteboard) { false }
        XCTAssertEqual(result, .notPasted)
    }

    /// A clipboard manager reads at once; the target's own read comes later,
    /// and must still find the image rather than the text that follows it.
    @MainActor
    func testImageOutlivesAnEarlierReadByAnotherProcess() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let png = try samplePNG()
        let late = Task { @MainActor () -> Data? in
            try? await Task.sleep(for: .milliseconds(400))
            return pasteboard.data(forType: .png)
        }
        _ = await SketchPaste.paste(png: png, pasteboard: pasteboard) {
            _ = pasteboard.data(forType: .png)
            return true
        }
        pasteboard.clearContents()
        pasteboard.setString("text", forType: .string)
        let lateRead = await late.value
        XCTAssertEqual(lateRead, png)
    }

    func testControlVIsClosedSoAShellCannotRunThePastedLines() {
        XCTAssertEqual(
            SketchPasteChord.control.closingKeys(after: .unread),
            [SketchPasteChord.spaceKey, SketchPasteChord.backspaceKey]
        )
        XCTAssertEqual(
            SketchPasteChord.control.closingKeys(after: .read),
            [SketchPasteChord.spaceKey],
            "a clipboard manager's read must not leave the shell in quoted insert; no Backspace near an agent's image"
        )
        XCTAssertEqual(SketchPasteChord.control.closingKeys(after: .notPasted), [])
        XCTAssertEqual(SketchPasteChord.command.closingKeys(after: .unread), [])
        XCTAssertEqual(SketchPasteChord.command.closingKeys(after: .read), [])
    }

    func testDeliveryNoticeNamesWhatWasLost() {
        XCTAssertNil(SketchDeliveryNotice.message(failedCaptures: 0, sheet: nil))
        XCTAssertNil(SketchDeliveryNotice.message(failedCaptures: 0, sheet: .read))
        XCTAssertEqual(SketchDeliveryNotice.message(failedCaptures: 0, sheet: .unread), "Sketches not pasted")
        XCTAssertEqual(SketchDeliveryNotice.message(failedCaptures: 2, sheet: .notPasted), "Sketches not pasted")
        XCTAssertEqual(SketchDeliveryNotice.message(failedCaptures: 1, sheet: nil), "1 sketch not captured")
        XCTAssertEqual(SketchDeliveryNotice.message(failedCaptures: 3, sheet: .read), "3 sketches not captured")
    }

    @MainActor
    func testPermissionPromptWaitsForThePasteAndOpensOncePerLaunch() async {
        let prompt = SketchPermissionPrompt()
        var events: [String] = []
        let task = prompt.request(
            failedCaptures: 1, isGranted: { false }, isIdle: { true },
            sleep: { XCTAssertGreaterThanOrEqual($0, .seconds(1)) },
            show: { events.append("window") }
        )
        events.append("after request")
        await task?.value
        XCTAssertEqual(events, ["after request", "window"], "never synchronously after Command-V")
        let again = prompt.request(failedCaptures: 1, isGranted: { false }, isIdle: { true }, sleep: { _ in }, show: {
            events.append("again")
        })
        await again?.value
        XCTAssertEqual(events, ["after request", "window"])
    }

    @MainActor
    func testPermissionPromptSkipsANewDictationAndAGrantedPermission() async {
        let prompt = SketchPermissionPrompt()
        var shown = 0
        await prompt.request(failedCaptures: 1, isGranted: { false }, isIdle: { false }, sleep: { _ in }, show: {
            shown += 1
        })?.value
        await prompt.request(failedCaptures: 0, isGranted: { false }, isIdle: { true }, sleep: { _ in }, show: {
            shown += 1
        })?.value
        await prompt.request(failedCaptures: 1, isGranted: { true }, isIdle: { true }, sleep: { _ in }, show: {
            shown += 1
        })?.value
        XCTAssertEqual(shown, 0)
        await prompt.request(failedCaptures: 1, isGranted: { false }, isIdle: { true }, sleep: { _ in }, show: {
            shown += 1
        })?.value
        XCTAssertEqual(shown, 1, "skipping while busy does not use up the one prompt")
    }
}

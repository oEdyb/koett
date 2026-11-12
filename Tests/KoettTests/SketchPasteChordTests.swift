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
        let pasted = await SketchPaste.paste(png: png, pasteboard: pasteboard) {
            posted.append(SketchPasteChord.forApp("com.mitchellh.ghostty", overrides: [:]).flags)
            read = pasteboard.data(forType: .png)
            return true
        }
        XCTAssertTrue(pasted)
        XCTAssertEqual(posted, [.maskControl])
        XCTAssertEqual(read, png, "a terminal agent reading PNG gets the sheet")
        XCTAssertLessThan(ContinuousClock.now - started, SketchPaste.readTimeout, "the settle follows the read, not the 1.5 s timeout")
    }
}

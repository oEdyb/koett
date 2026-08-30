@testable import Koett
import AppKit
import IOKit.hidsystem
import XCTest

final class ShortcutBindingTests: XCTestCase {
    func testDefaultDictationRequiresOptionAlone() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58
        ).down)
        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58
        ).up)
    }

    func testResetRecoversAfterMissedModifierRelease() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58
        ).down)

        state.reset()

        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58
        ).down)
        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58
        ).releasedWithoutChord)
    }

    func testResyncTracksModifierHeldThroughEventTapRecovery() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertTrue(state.resync(
            binding: binding,
            isKeyDown: { $0 == 58 }
        ))

        let recoveredRelease = state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58,
            keyIsDown: false
        )
        XCTAssertTrue(recoveredRelease.up)
        XCTAssertFalse(recoveredRelease.down)
        XCTAssertFalse(recoveredRelease.releasedWithoutChord)

        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58,
            keyIsDown: true
        ).down)
        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58,
            keyIsDown: false
        ).releasedWithoutChord)
    }

    func testDefaultMediaUsesControlShiftT() {
        let binding = ShortcutBinding.defaultMedia

        XCTAssertTrue(binding.matchesDown(
            type: .keyDown,
            keyCode: 17,
            modifierFlags: NSEvent.ModifierFlags([.control, .shift]).rawValue,
            isRepeat: false
        ))
        XCTAssertFalse(binding.matchesDown(
            type: .keyDown,
            keyCode: 17,
            modifierFlags: NSEvent.ModifierFlags([.option, .command]).rawValue,
            isRepeat: false
        ))
    }

    func testDefaultAssistantUsesControlShiftSpace() {
        let binding = ShortcutBinding.defaultAssistant

        XCTAssertTrue(binding.matchesDown(
            type: .keyDown,
            keyCode: 49,
            modifierFlags: NSEvent.ModifierFlags([.control, .shift]).rawValue,
            isRepeat: false
        ))
        XCTAssertFalse(binding.matchesDown(
            type: .keyDown,
            keyCode: 49,
            modifierFlags: NSEvent.ModifierFlags([.command, .shift]).rawValue,
            isRepeat: false
        ))
        XCTAssertFalse(ShortcutBinding.shortcutsConflict(
            dictation: .defaultDictation,
            media: binding,
            dictationUsesToggle: false
        ))
    }

    func testCustomKeySupportsHoldRelease() {
        let binding = ShortcutBinding.custom(
            keyCode: 8,
            modifiers: [.control, .option],
            keyName: "C",
            modifierOnly: false
        )

        XCTAssertTrue(binding.matchesDown(
            type: .keyDown,
            keyCode: 8,
            modifierFlags: NSEvent.ModifierFlags([.control, .option]).rawValue,
            isRepeat: false
        ))
        XCTAssertTrue(binding.matchesUp(
            type: .keyUp,
            keyCode: 8,
            modifierFlags: 0
        ))
        XCTAssertEqual(binding.displayName, "⌃⌥C")
    }

    func testDetectsConflictingBindings() {
        XCTAssertTrue(ShortcutBinding.defaultMedia.conflicts(with: .defaultMedia))
        XCTAssertFalse(ShortcutBinding.defaultMedia.conflicts(with: .defaultDictation))

        let optionCommandT = ShortcutBinding.custom(
            keyCode: 17,
            modifiers: [.option, .command],
            keyName: "T",
            modifierOnly: false
        )
        XCTAssertTrue(
            ShortcutBinding.defaultDictation.conflicts(with: optionCommandT)
        )
        XCTAssertFalse(ShortcutBinding.shortcutsConflict(
            dictation: .defaultDictation,
            media: optionCommandT,
            dictationUsesToggle: true
        ))
        XCTAssertTrue(ShortcutBinding.shortcutsConflict(
            dictation: .defaultDictation,
            media: optionCommandT,
            dictationUsesToggle: false
        ))
    }

    func testModifierTapTriggersToggleOnlyWhenUsedAlone() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertTrue(
            state.update(binding: binding, type: .flagsChanged, keyCode: 58)
                .releasedWithoutChord
        )

        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        state.noteKeyDown()
        XCTAssertFalse(
            state.update(binding: binding, type: .flagsChanged, keyCode: 58)
                .releasedWithoutChord
        )
    }

    func testModifierTapWithAnotherModifierDoesNotToggle() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 56)
        XCTAssertFalse(
            state.update(binding: binding, type: .flagsChanged, keyCode: 58)
                .releasedWithoutChord
        )

        _ = state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 58,
            modifierFlags: NSEvent.ModifierFlags([.shift, .option]).rawValue
        )
        XCTAssertFalse(
            state.update(binding: binding, type: .flagsChanged, keyCode: 58)
                .releasedWithoutChord
        )
    }

    func testEitherOptionTracksBothPhysicalKeys() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        XCTAssertTrue(state.update(binding: binding, type: .flagsChanged, keyCode: 58).down)
        let rightDown = state.update(binding: binding, type: .flagsChanged, keyCode: 61)
        XCTAssertFalse(rightDown.down)
        XCTAssertFalse(rightDown.up)
        let leftUp = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertFalse(leftUp.down)
        XCTAssertFalse(leftUp.up)
        XCTAssertTrue(state.update(binding: binding, type: .flagsChanged, keyCode: 61).up)
    }

    func testBothOptionKeysDoNotCountAsOnePlainTap() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()

        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 61)
        _ = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertFalse(
            state.update(binding: binding, type: .flagsChanged, keyCode: 61)
                .releasedWithoutChord
        )
    }

    func testRightOptionIgnoresLeftOption() {
        let binding = ShortcutBinding.migratedDictation("rightOption")
        var state = ModifierShortcutState()

        let leftDown = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertFalse(leftDown.down)
        XCTAssertFalse(leftDown.up)
        XCTAssertTrue(state.update(binding: binding, type: .flagsChanged, keyCode: 61).down)
        XCTAssertTrue(state.update(binding: binding, type: .flagsChanged, keyCode: 61).up)
        let leftUp = state.update(binding: binding, type: .flagsChanged, keyCode: 58)
        XCTAssertFalse(leftUp.down)
        XCTAssertFalse(leftUp.up)
    }

    func testRightOptionUsesEventFlagsWhenEventSourceStateIsStale() {
        let binding = ShortcutBinding.defaultDictation
        var state = ModifierShortcutState()
        let rightOptionDownFlags = NSEvent.ModifierFlags.option.rawValue
            | UInt(NX_DEVICERALTKEYMASK)

        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 61,
            modifierFlags: rightOptionDownFlags,
            keyIsDown: false
        ).down)
        XCTAssertTrue(state.update(
            binding: binding,
            type: .flagsChanged,
            keyCode: 61,
            modifierFlags: 0,
            keyIsDown: false
        ).releasedWithoutChord)
    }
}

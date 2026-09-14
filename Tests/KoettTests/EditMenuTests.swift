import AppKit
@testable import Koett
import XCTest

final class EditMenuTests: XCTestCase {
    func testEditMenuRoutesStandardCommandKeys() {
        let menu = EditMenu.make()
        let byKey = Dictionary(
            uniqueKeysWithValues: menu.items
                .filter { !$0.isSeparatorItem }
                .map { ($0.keyEquivalent, $0) }
        )

        XCTAssertEqual(byKey["v"]?.title, "Paste")
        XCTAssertEqual(byKey["v"]?.action, #selector(NSText.paste(_:)))
        XCTAssertEqual(byKey["c"]?.title, "Copy")
        XCTAssertEqual(byKey["x"]?.title, "Cut")
        XCTAssertEqual(byKey["a"]?.title, "Select All")
        XCTAssertTrue(menu.items.allSatisfy {
            $0.isSeparatorItem || $0.keyEquivalentModifierMask == .command
        })
    }
}

@testable import Koett
import XCTest

final class PinContextPrivacyTests: XCTestCase {
    private let context = PinContext(app: "Google Chrome", title: "Private fixture", url: "https://example.test/private",
        videoSeconds: 12, link: "https://example.test/link", image: "https://example.test/image", selection: "Private selection")

    func testIncognitoWindowOmitsEveryPageFieldAndKeepsAppOnly() {
        let filtered = PinContextPrivacy.filtered(context, bundleID: "com.google.Chrome",
            windowTitle: "Private fixture - Google Chrome (Incognito)")
        XCTAssertEqual(filtered, PinContext(app: "Google Chrome"))
        XCTAssertEqual(filtered.lines, ["Google Chrome"])
    }

    func testPageTitleContainingIncognitoIsNotAWindowModeSignal() {
        let normal = "Private fixture (Incognito) - Google Chrome"
        XCTAssertEqual(PinContextPrivacy.filtered(context, bundleID: "com.google.Chrome", windowTitle: normal), context)
        XCTAssertEqual(PinContextPrivacy.filtered(context, bundleID: "com.google.Chrome",
            windowTitle: "Incognito tutorial - Google Chrome"), context)
    }

    func testMissingChromeWindowIdentityFailsClosed() {
        for title in [nil, ""] as [String?] {
            XCTAssertEqual(PinContextPrivacy.filtered(context, bundleID: "com.google.Chrome", windowTitle: title),
                           PinContext(app: "Google Chrome"))
        }
    }

    func testOtherAppsAreNotClassifiedFromTheirTitles() {
        for bundle in [nil, "com.apple.Safari", "com.example.editor"] as [String?] {
            XCTAssertEqual(PinContextPrivacy.filtered(context, bundleID: bundle, windowTitle: "Test (Incognito)"), context)
        }
    }

    func testChromeChannelsUseSameExplicitWindowMarker() {
        for bundle in ["com.google.Chrome.beta", "com.google.Chrome.dev", "com.google.Chrome.canary", "org.chromium.Chromium"] {
            XCTAssertTrue(PinContextPrivacy.omitsPageDetails(bundleID: bundle, windowTitle: "Test (Incognito)"))
        }
    }
}

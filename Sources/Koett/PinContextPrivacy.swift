import Foundation

enum PinContextPrivacy {
    private static let chromiumBrowsers: Set<String> = [
        "com.google.Chrome", "com.google.Chrome.beta", "com.google.Chrome.dev",
        "com.google.Chrome.canary", "org.chromium.Chromium",
    ]

    static func omitsPageDetails(bundleID: String?, windowTitle: String?) -> Bool {
        guard let bundleID, chromiumBrowsers.contains(bundleID) else { return false }
        guard let windowTitle, !windowTitle.isEmpty else { return true }
        // Chromium's accessible-window formatter appends this after the browser
        // name for an Incognito profile; page text is not a privacy signal.
        return windowTitle.hasSuffix(" (Incognito)")
    }

    static func filtered(_ context: PinContext, bundleID: String?, windowTitle: String?) -> PinContext {
        omitsPageDetails(bundleID: bundleID, windowTitle: windowTitle) ? PinContext(app: context.app) : context
    }
}

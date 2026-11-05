import ServiceManagement
@testable import Koett
import XCTest

final class LoginPreferenceMenuTests: XCTestCase {
    func testInitialMenuReadsStatusEvenDuringBusySetup() {
        var preference = LoginPreference()
        var reads = 0
        let status = preference.menuStatus(refresh: false) {
            reads += 1
            return .enabled
        }
        XCTAssertEqual(status, .enabled)
        XCTAssertEqual(reads, 1)
    }

    func testBusyMenuReusesStatusWithoutCallingService() {
        var preference = LoginPreference()
        var reads = 0
        _ = preference.menuStatus(refresh: true) { reads += 1; return .enabled }
        for _ in 0..<3 {
            let status = preference.menuStatus(refresh: false) {
                reads += 1
                return .notRegistered
            }
            XCTAssertEqual(status, .enabled)
        }
        XCTAssertEqual(reads, 1)
    }

    func testMenuOpeningOrReadyRefreshesExternallyChangedStatus() {
        var preference = LoginPreference()
        _ = preference.menuStatus(refresh: true) { .enabled }
        let status = preference.menuStatus(refresh: true) { .requiresApproval }
        XCTAssertEqual(status, .requiresApproval)
        XCTAssertEqual(preference.menuStatus(refresh: false) { .notRegistered }, .requiresApproval)
    }
}

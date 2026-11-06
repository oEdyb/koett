import ServiceManagement
@testable import Koett
import XCTest

final class LoginPreferenceMenuTests: XCTestCase {
    func testUnregisterDoesNotCallServiceWhenNoLoginItemExists() throws {
        for status: SMAppService.Status in [.notRegistered, .notFound] {
            var calls = 0
            try LoginPreference.unregisterIfRegistered(status: status) {
                calls += 1
                throw NSError(domain: "SMAppServiceErrorDomain", code: 1)
            }
            XCTAssertEqual(calls, 0)
        }
    }

    func testUnregisterCallsRegisteredServiceAndPreservesFailures() {
        for status: SMAppService.Status in [.enabled, .requiresApproval] {
            var calls = 0
            XCTAssertThrowsError(try LoginPreference.unregisterIfRegistered(status: status) {
                calls += 1
                throw NSError(domain: "SMAppServiceErrorDomain", code: 1)
            })
            XCTAssertEqual(calls, 1)
        }
    }

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

    func testExplicitRefreshUpdatesExternallyChangedStatus() {
        var preference = LoginPreference()
        _ = preference.menuStatus(refresh: true) { .enabled }
        let status = preference.menuStatus(refresh: true) { .requiresApproval }
        XCTAssertEqual(status, .requiresApproval)
        XCTAssertEqual(preference.menuStatus(refresh: false) { .notRegistered }, .requiresApproval)
    }
}

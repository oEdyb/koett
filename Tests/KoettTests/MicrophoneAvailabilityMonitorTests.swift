import AppKit
import CoreAudio
@testable import Koett
import XCTest

@MainActor
final class MicrophoneAvailabilityMonitorTests: XCTestCase {
    func testRepeatedStartRegistersSystemListenersOnlyOnceAndStopRemovesEachOnce() throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        let monitor = makeMonitor(registrar)
        try monitor.start()
        try monitor.start()
        XCTAssertTrue(monitor.isStarted)
        XCTAssertEqual(registrar.registrations.map(\.objectID), [AudioObjectID(kAudioObjectSystemObject), AudioObjectID(kAudioObjectSystemObject)])
        XCTAssertEqual(registrar.registrations.map(\.selector), [kAudioHardwarePropertyDevices, kAudioHardwarePropertyDefaultInputDevice])
        monitor.stop()
        monitor.stop()
        XCTAssertFalse(monitor.isStarted)
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1, 1])
    }

    func testFailedRegistrationRollsBackAndCanRetry() throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        registrar.failAtAttempt = 2
        let monitor = makeMonitor(registrar)
        XCTAssertThrowsError(try monitor.start())
        XCTAssertFalse(monitor.isStarted)
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1])
        registrar.failAtAttempt = nil
        try monitor.start()
        XCTAssertTrue(monitor.isStarted)
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1, 0, 0])
        monitor.stop()
    }

    func testDeviceWatchRetargetsAndIgnoresStaleDeviceCallbacks() async throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        var changes = 0
        let monitor = makeMonitor(registrar) { changes += 1 }
        try monitor.start()
        try monitor.watch(deviceID: 42)
        try monitor.watch(deviceID: 42)
        XCTAssertEqual(registrar.registrations.count, 3)
        let oldDevice = registrar.registrations[2]
        oldDevice.onChange()
        try monitor.watch(deviceID: 43)
        XCTAssertEqual(oldDevice.cancellations, 1)
        await flushCallbacks()
        XCTAssertEqual(changes, 0)
        let currentDevice = registrar.registrations[3]
        XCTAssertEqual(currentDevice.objectID, 43)
        XCTAssertEqual(currentDevice.selector, kAudioDevicePropertyDeviceIsAlive)
        currentDevice.onChange()
        await flushCallbacks()
        XCTAssertEqual(changes, 1)
        try monitor.watch(deviceID: nil)
        XCTAssertEqual(currentDevice.cancellations, 1)
        currentDevice.onChange()
        await flushCallbacks()
        XCTAssertEqual(changes, 1)
        monitor.stop()
    }

    func testFailedDeviceRetargetKeepsSystemListenersAndAllowsRetry() throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        let monitor = makeMonitor(registrar)
        try monitor.start()
        try monitor.watch(deviceID: 42)
        registrar.failAtAttempt = 4
        XCTAssertThrowsError(try monitor.watch(deviceID: 43))
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [0, 0, 1])
        XCTAssertTrue(monitor.isStarted)
        registrar.failAtAttempt = nil
        try monitor.watch(deviceID: 43)
        XCTAssertEqual(registrar.registrations.last?.objectID, 43)
        monitor.stop()
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1, 1, 1, 1])
    }

    func testDeviceWatchedBeforeStartRegistersTransactionally() throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        let monitor = makeMonitor(registrar)
        try monitor.watch(deviceID: 42)
        XCTAssertEqual(registrar.registrations.count, 0)
        registrar.failAtAttempt = 3
        XCTAssertThrowsError(try monitor.start())
        XCTAssertFalse(monitor.isStarted)
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1, 1])
        registrar.failAtAttempt = nil
        try monitor.start()
        XCTAssertEqual(registrar.registrations.last?.objectID, 42)
        monitor.stop()
    }

    func testCallbacksQueuedBeforeStopOrRestartCannotRefresh() async throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        var changes = 0
        let monitor = makeMonitor(registrar) { changes += 1 }
        try monitor.start()
        let old = registrar.registrations[0]
        old.onChange()
        monitor.stop()
        try monitor.start()
        old.onChange()
        await flushCallbacks()
        XCTAssertEqual(changes, 0)
        registrar.registrations[2].onChange()
        await flushCallbacks()
        XCTAssertEqual(changes, 1)
        monitor.stop()
    }

    func testBothSystemChangesDeliverRefreshes() async throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        var changes = 0
        let monitor = makeMonitor(registrar) { changes += 1 }
        try monitor.start()
        registrar.registrations.forEach { $0.onChange() }
        await flushCallbacks()
        XCTAssertEqual(changes, 2)
        monitor.stop()
    }

    func testWakeAndApplicationActivationRefreshOnlyWhileStarted() async throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        let workspace = NotificationCenter()
        let application = NotificationCenter()
        var changes = 0
        let monitor = MicrophoneAvailabilityMonitor(
            onChange: { changes += 1 },
            registrar: registrar,
            workspaceCenter: workspace,
            applicationCenter: application
        )
        try monitor.start()
        workspace.post(name: NSWorkspace.didWakeNotification, object: nil)
        application.post(name: NSApplication.didBecomeActiveNotification, object: nil)
        await flushCallbacks()
        XCTAssertEqual(changes, 2)
        workspace.post(name: NSWorkspace.didWakeNotification, object: nil)
        monitor.stop()
        application.post(name: NSApplication.didBecomeActiveNotification, object: nil)
        await flushCallbacks()
        XCTAssertEqual(changes, 2)
    }

    func testDroppingMonitorRemovesAllRegistrationsWithoutRetainCycle() async throws {
        let registrar = FakeMicrophoneAvailabilityRegistrar()
        var changes = 0
        var monitor: MicrophoneAvailabilityMonitor? = makeMonitor(registrar) { changes += 1 }
        weak let weakMonitor = monitor
        try monitor?.start()
        try monitor?.watch(deviceID: 42)
        registrar.registrations[0].onChange()
        monitor = nil
        XCTAssertNil(weakMonitor)
        XCTAssertEqual(registrar.registrations.map(\.cancellations), [1, 1, 1])
        registrar.registrations.forEach { $0.onChange() }
        await flushCallbacks()
        XCTAssertEqual(changes, 0)
    }

    private func makeMonitor(
        _ registrar: FakeMicrophoneAvailabilityRegistrar,
        onChange: @escaping @MainActor @Sendable () -> Void = {}
    ) -> MicrophoneAvailabilityMonitor {
        MicrophoneAvailabilityMonitor(
            onChange: onChange,
            registrar: registrar,
            workspaceCenter: NotificationCenter(),
            applicationCenter: NotificationCenter()
        )
    }

    private func flushCallbacks() async {
        for _ in 0..<20 { await Task.yield() }
    }
}

@MainActor
private final class FakeMicrophoneAvailabilityRegistrar: MicrophoneAvailabilityRegistering {
    var registrations: [FakeMicrophoneAvailabilityRegistration] = []
    var failAtAttempt: Int?
    private var attempts = 0

    func register(
        objectID: AudioObjectID,
        selector: AudioObjectPropertySelector,
        onChange: @escaping @Sendable () -> Void
    ) throws -> any MicrophoneAvailabilityRegistration {
        attempts += 1
        if attempts == failAtAttempt { throw MicrophoneDevices.failure("Listener unavailable") }
        let registration = FakeMicrophoneAvailabilityRegistration(objectID: objectID, selector: selector, onChange: onChange)
        registrations.append(registration)
        return registration
    }
}

@MainActor
private final class FakeMicrophoneAvailabilityRegistration: MicrophoneAvailabilityRegistration {
    let objectID: AudioObjectID
    let selector: AudioObjectPropertySelector
    let onChange: @Sendable () -> Void
    private(set) var cancellations = 0

    init(objectID: AudioObjectID, selector: AudioObjectPropertySelector, onChange: @escaping @Sendable () -> Void) {
        self.objectID = objectID
        self.selector = selector
        self.onChange = onChange
    }

    func cancel() {
        cancellations += 1
    }
}

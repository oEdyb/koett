import AppKit
import CoreAudio

@MainActor
protocol MicrophoneAvailabilityRegistration: AnyObject {
    func cancel()
}

@MainActor
protocol MicrophoneAvailabilityRegistering {
    func register(
        objectID: AudioObjectID,
        selector: AudioObjectPropertySelector,
        onChange: @escaping @Sendable () -> Void
    ) throws -> any MicrophoneAvailabilityRegistration
}

@MainActor
final class MicrophoneAvailabilityMonitor {
    private let registrar: any MicrophoneAvailabilityRegistering
    private let onChange: @MainActor @Sendable () -> Void
    private let workspaceCenter: NotificationCenter
    private let applicationCenter: NotificationCenter
    private var systemRegistrations: [any MicrophoneAvailabilityRegistration] = []
    private var deviceRegistration: (any MicrophoneAvailabilityRegistration)?
    private var notificationObservers: [(NotificationCenter, NSObjectProtocol)] = []
    private var generation: UInt64 = 0
    private var deviceGeneration: UInt64 = 0
    private var watchedDeviceID: AudioDeviceID?
    private(set) var isStarted = false

    convenience init(onChange: @escaping @MainActor @Sendable () -> Void) {
        self.init(
            onChange: onChange,
            registrar: CoreAudioMicrophoneAvailabilityRegistrar(),
            workspaceCenter: NSWorkspace.shared.notificationCenter,
            applicationCenter: .default
        )
    }

    init(
        onChange: @escaping @MainActor @Sendable () -> Void,
        registrar: any MicrophoneAvailabilityRegistering,
        workspaceCenter: NotificationCenter,
        applicationCenter: NotificationCenter
    ) {
        self.onChange = onChange
        self.registrar = registrar
        self.workspaceCenter = workspaceCenter
        self.applicationCenter = applicationCenter
    }

    func start() throws {
        guard !isStarted else { return }
        generation &+= 1
        var registrations: [any MicrophoneAvailabilityRegistration] = []
        do {
            for selector in [kAudioHardwarePropertyDevices, kAudioHardwarePropertyDefaultInputDevice] {
                registrations.append(try registrar.register(
                    objectID: AudioObjectID(kAudioObjectSystemObject),
                    selector: selector,
                    onChange: callback()
                ))
            }
            if let watchedDeviceID {
                deviceRegistration = try registerDevice(watchedDeviceID)
            }
        } catch {
            registrations.forEach { $0.cancel() }
            deviceRegistration?.cancel()
            deviceRegistration = nil
            generation &+= 1
            throw error
        }
        systemRegistrations = registrations
        isStarted = true
        observe(NSWorkspace.didWakeNotification, in: workspaceCenter)
        observe(NSApplication.didBecomeActiveNotification, in: applicationCenter)
    }

    func watch(deviceID: AudioDeviceID?) throws {
        let deviceID = deviceID.flatMap { $0 == kAudioObjectUnknown ? nil : $0 }
        guard deviceID != watchedDeviceID || (isStarted && deviceID != nil && deviceRegistration == nil) else { return }
        deviceGeneration &+= 1
        deviceRegistration?.cancel()
        deviceRegistration = nil
        watchedDeviceID = deviceID
        guard isStarted, let deviceID else { return }
        deviceRegistration = try registerDevice(deviceID)
    }

    func stop() {
        generation &+= 1
        deviceGeneration &+= 1
        isStarted = false
        deviceRegistration?.cancel()
        deviceRegistration = nil
        watchedDeviceID = nil
        systemRegistrations.forEach { $0.cancel() }
        systemRegistrations.removeAll()
        for (center, observer) in notificationObservers { center.removeObserver(observer) }
        notificationObservers.removeAll()
    }

    isolated deinit { stop() }

    private func registerDevice(_ id: AudioDeviceID) throws -> any MicrophoneAvailabilityRegistration {
        try registrar.register(
            objectID: id,
            selector: kAudioDevicePropertyDeviceIsAlive,
            onChange: callback(deviceGeneration: deviceGeneration)
        )
    }

    private func callback(deviceGeneration: UInt64? = nil) -> @Sendable () -> Void {
        let generation = generation
        return { [weak self] in
            Task { @MainActor [weak self] in
                guard let self, self.isStarted, self.generation == generation,
                      deviceGeneration == nil || self.deviceGeneration == deviceGeneration else { return }
                self.onChange()
            }
        }
    }

    private func observe(_ name: Notification.Name, in center: NotificationCenter) {
        let callback = callback()
        let observer = center.addObserver(forName: name, object: nil, queue: .main) { _ in callback() }
        notificationObservers.append((center, observer))
    }
}

@MainActor
private struct CoreAudioMicrophoneAvailabilityRegistrar: MicrophoneAvailabilityRegistering {
    func register(
        objectID: AudioObjectID,
        selector: AudioObjectPropertySelector,
        onChange: @escaping @Sendable () -> Void
    ) throws -> any MicrophoneAvailabilityRegistration {
        try CoreAudioMicrophoneAvailabilityRegistration(objectID: objectID, selector: selector, onChange: onChange)
    }
}

@MainActor
private final class CoreAudioMicrophoneAvailabilityRegistration: MicrophoneAvailabilityRegistration {
    private let objectID: AudioObjectID
    private var address: AudioObjectPropertyAddress
    private let listener: AudioObjectPropertyListenerBlock
    private let queue = DispatchQueue.main
    private var registered = false

    init(objectID: AudioObjectID, selector: AudioObjectPropertySelector, onChange: @escaping @Sendable () -> Void) throws {
        self.objectID = objectID
        address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        listener = { _, _ in onChange() }
        let status = AudioObjectAddPropertyListenerBlock(objectID, &address, queue, listener)
        guard status == noErr else {
            throw MicrophoneDevices.failure("Microphone changes could not be monitored (\(status)).")
        }
        registered = true
    }

    func cancel() {
        guard registered else { return }
        registered = false
        AudioObjectRemovePropertyListenerBlock(objectID, &address, queue, listener)
    }

    isolated deinit { cancel() }
}

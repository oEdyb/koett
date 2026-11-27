import CoreAudio
import Darwin
import Foundation

@MainActor
final class MeetingSystemAudio: MeetingAudioSource {
    let ring: MeetingAudioRing
    private let resources: Resources
    private let route: AudioDeviceID
    private let format: AudioStreamBasicDescription

    init() throws {
        route = try Self.defaultOutput()
        let process: AudioObjectID = try Self.property(
            Self.system, kAudioHardwarePropertyTranslatePIDToProcessObject,
            initial: kAudioObjectUnknown, qualifier: getpid()
        )
        let description = CATapDescription(monoGlobalTapButExcludeProcesses: [process])
        description.name = "Koett meeting audio"
        description.isPrivate = true
        description.muteBehavior = .unmuted

        let resources = Resources()
        try Self.check(AudioHardwareCreateProcessTap(description, &resources.tap))
        let format = try Self.tapFormat(resources.tap)
        let ring = MeetingAudioRing(rate: format.mSampleRate)
        let aggregate: [String: Any] = [
            kAudioAggregateDeviceNameKey: "Koett meeting audio",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceTapAutoStartKey: false,
            kAudioAggregateDeviceTapListKey: [[
                kAudioSubTapUIDKey: description.uuid.uuidString,
                kAudioSubTapDriftCompensationKey: true
            ]]
        ]
        try Self.check(AudioHardwareCreateAggregateDevice(aggregate as CFDictionary, &resources.device))
        try Self.check(AudioDeviceCreateIOProcIDWithBlock(&resources.proc, resources.device, nil) { _, input, time, _, _ in
            ring.copy(input, hostTime: time.pointee.mFlags.contains(.hostTimeValid) ? time.pointee.mHostTime : 0)
        })
        guard let proc = resources.proc else { throw Self.failure(kAudioHardwareUnspecifiedError) }
        try Self.check(AudioDeviceStart(resources.device, proc))
        resources.started = true
        self.format = format
        self.ring = ring
        self.resources = resources
    }

    var isConnected: Bool {
        guard (try? Self.defaultOutput()) == route,
              (try? Self.property(route, kAudioDevicePropertyDeviceIsAlive, initial: UInt32(0))) == 1,
              let current = try? Self.tapFormat(resources.tap)
        else { return false }
        return format.mSampleRate == current.mSampleRate && format.mChannelsPerFrame == current.mChannelsPerFrame
            && format.mFormatFlags == current.mFormatFlags
    }

    func stop() { ring.stop(); resources.close() }

    private final class Resources {
        var tap = AudioObjectID(kAudioObjectUnknown)
        var device = AudioObjectID(kAudioObjectUnknown)
        var proc: AudioDeviceIOProcID?
        var started = false

        func close() {
            if let proc {
                if started { AudioDeviceStop(device, proc) }
                AudioDeviceDestroyIOProcID(device, proc)
                self.proc = nil
            }
            if device != kAudioObjectUnknown {
                AudioHardwareDestroyAggregateDevice(device)
                device = kAudioObjectUnknown
            }
            if tap != kAudioObjectUnknown {
                AudioHardwareDestroyProcessTap(tap)
                tap = kAudioObjectUnknown
            }
        }

        deinit { close() }
    }

    private static let system = AudioObjectID(kAudioObjectSystemObject)

    private static func defaultOutput() throws -> AudioDeviceID {
        let id = try property(system, kAudioHardwarePropertyDefaultOutputDevice, initial: kAudioObjectUnknown)
        guard id != kAudioObjectUnknown else { throw failure(kAudioHardwareBadDeviceError) }
        return id
    }

    private static func tapFormat(_ tap: AudioObjectID) throws -> AudioStreamBasicDescription {
        let format = try property(tap, kAudioTapPropertyFormat, initial: AudioStreamBasicDescription())
        guard format.mSampleRate.isFinite, (8_000...384_000).contains(format.mSampleRate),
              format.mFormatID == kAudioFormatLinearPCM, format.mBitsPerChannel == 32,
              format.mFormatFlags & kAudioFormatFlagIsFloat != 0
        else { throw failure(kAudioHardwareUnsupportedOperationError) }
        return format
    }

    private static func property<T: BitwiseCopyable>(
        _ id: AudioObjectID, _ selector: AudioObjectPropertySelector, initial: T, qualifier: pid_t? = nil
    ) throws -> T {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal,
                                                 mElement: kAudioObjectPropertyElementMain)
        var size = UInt32(MemoryLayout<T>.size)
        var result = initial
        if var qualifier {
            try check(AudioObjectGetPropertyData(id, &address, UInt32(MemoryLayout<pid_t>.size), &qualifier, &size, &result))
        } else {
            try check(AudioObjectGetPropertyData(id, &address, 0, nil, &size, &result))
        }
        return result
    }

    private static func check(_ status: OSStatus) throws {
        guard status == noErr else { throw failure(status) }
    }

    private static func failure(_ status: OSStatus) -> NSError {
        NSError(domain: "Koett.Meeting.SystemAudio", code: Int(status), userInfo: [
            NSLocalizedDescriptionKey: "System audio unavailable (\(status)). Check System Audio Recording access and retry."
        ])
    }
}

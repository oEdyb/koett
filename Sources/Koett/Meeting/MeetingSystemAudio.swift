import CoreAudio
import Darwin
import Foundation

@MainActor
final class MeetingSystemAudio: MeetingAudioSource {
    let ring: MeetingAudioRing
    private let resources: Resources
    private let format: AudioStreamBasicDescription

    init() throws {
        let process: AudioObjectID = try Self.property(
            Self.system, kAudioHardwarePropertyTranslatePIDToProcessObject,
            initial: kAudioObjectUnknown, qualifier: getpid()
        )
        let description = CATapDescription(monoGlobalTapButExcludeProcesses: try Self.excludingProcess(process))
        description.name = "Koett meeting audio"
        description.isPrivate = true
        description.muteBehavior = .unmuted

        let resources = Resources()
        try Self.check(AudioHardwareCreateProcessTap(description, &resources.tap))
        let format = try Self.tapFormat(resources.tap)
        let ring = MeetingAudioRing(rate: format.mSampleRate, channels: Int(format.mChannelsPerFrame))
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
        try Self.check(AudioDeviceCreateIOProcIDWithBlock(&resources.proc, resources.device, nil, Self.callback(for: ring)))
        guard let proc = resources.proc else { throw Self.failure(kAudioHardwareUnspecifiedError) }
        try Self.check(AudioDeviceStart(resources.device, proc))
        resources.started = true
        self.format = format
        self.ring = ring
        self.resources = resources
    }

    var isConnected: Bool {
        guard let current = try? Self.property(resources.tap, kAudioTapPropertyFormat, initial: AudioStreamBasicDescription())
        else { return true }
        return Self.sameFormat(format, current)
    }

    nonisolated static func sameFormat(_ old: AudioStreamBasicDescription, _ new: AudioStreamBasicDescription) -> Bool {
        old.mSampleRate == new.mSampleRate && old.mChannelsPerFrame == new.mChannelsPerFrame
            && old.mFormatID == new.mFormatID && old.mFormatFlags == new.mFormatFlags
            && old.mBytesPerFrame == new.mBytesPerFrame && old.mBitsPerChannel == new.mBitsPerChannel
    }

    nonisolated static func excludingProcess(_ process: AudioObjectID) throws -> [AudioObjectID] {
        guard process != kAudioObjectUnknown else {
            throw failure(kAudioHardwareBadObjectError)
        }
        return [process]
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

    nonisolated static func callback(for ring: MeetingAudioRing) -> AudioDeviceIOBlock {
        { _, input, time, _, _ in
            ring.copy(input, hostTime: time.pointee.mFlags.contains(.hostTimeValid) ? time.pointee.mHostTime : 0)
        }
    }

    private static let system = AudioObjectID(kAudioObjectSystemObject)

    private static func tapFormat(_ tap: AudioObjectID) throws -> AudioStreamBasicDescription {
        let format = try property(tap, kAudioTapPropertyFormat, initial: AudioStreamBasicDescription())
        guard format.mSampleRate.isFinite, (8_000...384_000).contains(format.mSampleRate),
              format.mFormatID == kAudioFormatLinearPCM, format.mBitsPerChannel == 32,
              format.mChannelsPerFrame > 0,
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

    nonisolated private static func failure(_ status: OSStatus) -> NSError {
        NSError(domain: "Koett.Meeting.SystemAudio", code: Int(status), userInfo: [
            NSLocalizedDescriptionKey: "System audio unavailable (\(status)). Check System Audio Recording access and retry."
        ])
    }
}

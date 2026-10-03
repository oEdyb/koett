import AudioToolbox
import AVFoundation
import CoreAudio
import Foundation

struct MicrophoneDevice: Equatable, Sendable {
    let id: AudioDeviceID
    let uid: String
    let name: String
}

struct MicrophoneSelection {
    static let defaultsKey = "microphoneUID"
    var uid: String?

    init(defaults: UserDefaults) {
        uid = defaults.string(forKey: Self.defaultsKey)
    }

    func save(to defaults: UserDefaults) {
        defaults.set(uid, forKey: Self.defaultsKey)
    }

    func resolve(in devices: [MicrophoneDevice], defaultID: AudioDeviceID) throws -> MicrophoneDevice {
        if let uid {
            guard let selected = devices.first(where: { $0.uid == uid }) else {
                throw MicrophoneDevices.failure("Selected microphone is unavailable. Choose a microphone in the Koett menu.")
            }
            return selected
        }
        guard let selected = devices.first(where: { $0.id == defaultID }) else {
            throw MicrophoneDevices.failure("No microphone is available. Choose a microphone in the Koett menu.")
        }
        return selected
    }
}

enum MicrophoneDevices {
    static func available() throws -> [MicrophoneDevice] {
        var address = address(kAudioHardwarePropertyDevices)
        var size: UInt32 = 0
        try check(AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size))
        var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        try check(AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &ids))
        return ids.compactMap { id in
            guard hasInput(id),
                  let uid = try? string(id, selector: kAudioDevicePropertyDeviceUID),
                  let name = try? string(id, selector: kAudioObjectPropertyName) else { return nil }
            return MicrophoneDevice(id: id, uid: uid, name: name)
        }.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    static func defaultInputID() throws -> AudioDeviceID {
        var address = address(kAudioHardwarePropertyDefaultInputDevice)
        var id = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        try check(AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &id))
        return id
    }

    static func failure(_ message: String) -> NSError {
        NSError(domain: "Koett.Microphone", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private static func address(_ selector: AudioObjectPropertySelector, scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress(mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain)
    }

    private static func hasInput(_ id: AudioDeviceID) -> Bool {
        var address = address(kAudioDevicePropertyStreams, scope: kAudioObjectPropertyScopeInput)
        var size: UInt32 = 0
        return AudioObjectGetPropertyDataSize(id, &address, 0, nil, &size) == noErr && size > 0
    }

    private static func string(_ id: AudioDeviceID, selector: AudioObjectPropertySelector) throws -> String {
        var address = address(selector)
        var value: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        try check(AudioObjectGetPropertyData(id, &address, 0, nil, &size, &value))
        guard let value else { throw failure("The microphone name could not be read.") }
        return value.takeRetainedValue() as String
    }

    private static func check(_ status: OSStatus) throws {
        guard status == noErr else {
            throw failure("The microphone could not be accessed (\(status)).")
        }
    }
}

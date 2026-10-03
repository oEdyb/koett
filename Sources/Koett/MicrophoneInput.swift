import AudioToolbox
import AVFoundation
import CoreAudio
import Foundation
import Synchronization

@MainActor
protocol MicrophoneCapturing: AnyObject {
    var format: AVAudioFormat { get }
    var error: Error? { get }
    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws
    func stop()
}

@MainActor
final class MicrophoneInput: MicrophoneCapturing {
    let format: AVAudioFormat
    private let unit: AudioUnit
    private let device: MicrophoneDevice
    private var callbackState: InputCallbackState?
    private var callbackReservation = MicrophoneCallbackReservation()
    private var started = false

    var error: Error? {
        let status = callbackState?.errorStatus.load(ordering: .acquiring) ?? noErr
        return status == noErr ? nil : MicrophoneDevices.failure("Microphone capture failed (\(status)).")
    }

    init(device: MicrophoneDevice) throws {
        self.device = device
        var description = AudioComponentDescription(
            componentType: kAudioUnitType_Output,
            componentSubType: kAudioUnitSubType_HALOutput,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0,
            componentFlagsMask: 0
        )
        guard let component = AudioComponentFindNext(nil, &description) else {
            throw MicrophoneDevices.failure("The microphone input could not be opened.")
        }
        var newUnit: AudioUnit?
        try Self.check(AudioComponentInstanceNew(component, &newUnit))
        guard let newUnit else { throw MicrophoneDevices.failure("The microphone input could not be opened.") }
        do {
            var enabled: UInt32 = 1
            try Self.check(AudioUnitSetProperty(newUnit, kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Input, 1, &enabled, 4))
            enabled = 0
            try Self.check(AudioUnitSetProperty(newUnit, kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Output, 0, &enabled, 4))
            var id = device.id
            try Self.check(AudioUnitSetProperty(newUnit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &id, 4))
            var hardware = AudioStreamBasicDescription()
            var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
            try Self.check(AudioUnitGetProperty(newUnit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, 1, &hardware, &size))
            guard hardware.mSampleRate > 0, hardware.mChannelsPerFrame > 0,
                  let format = AVAudioFormat(standardFormatWithSampleRate: hardware.mSampleRate, channels: hardware.mChannelsPerFrame) else {
                throw MicrophoneDevices.failure("The microphone has no supported audio format.")
            }
            var client = format.streamDescription.pointee
            try Self.check(AudioUnitSetProperty(newUnit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Output, 1, &client, size))
            self.format = format
            unit = newUnit
        } catch {
            AudioComponentInstanceDispose(newUnit)
            throw error
        }
    }

    func start(onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        try callbackReservation.reserve()
        var maximumFrames: UInt32 = 0
        var size: UInt32 = 4
        try Self.check(AudioUnitGetProperty(unit, kAudioUnitProperty_MaximumFramesPerSlice, kAudioUnitScope_Global, 0, &maximumFrames, &size))
        guard maximumFrames > 0,
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: maximumFrames) else {
            throw MicrophoneDevices.failure("The microphone buffer could not be created.")
        }
        let state = InputCallbackState(unit: unit, buffer: buffer, onAudio: onAudio)
        callbackState = state
        var callback = AURenderCallbackStruct(
            inputProc: Self.render,
            inputProcRefCon: Unmanaged.passUnretained(state).toOpaque()
        )
        try Self.check(AudioUnitSetProperty(unit, kAudioOutputUnitProperty_SetInputCallback, kAudioUnitScope_Global, 0, &callback, UInt32(MemoryLayout<AURenderCallbackStruct>.size)))
        try Self.check(AudioUnitInitialize(unit))
        try Self.check(AudioOutputUnitStart(unit))
        started = true
    }

    func stop() {
        guard started else { return }
        if !connectionIsValid() {
            callbackState?.errorStatus.store(kAudioHardwareBadDeviceError, ordering: .releasing)
        }
        let status = AudioOutputUnitStop(unit)
        if status != noErr { callbackState?.errorStatus.store(status, ordering: .releasing) }
        started = false
    }

    private func connectionIsValid() -> Bool {
        var running: UInt32 = 0
        var size: UInt32 = 4
        guard AudioUnitGetProperty(unit, kAudioOutputUnitProperty_IsRunning, kAudioUnitScope_Global, 0, &running, &size) == noErr,
              running != 0 else { return false }
        var alive: UInt32 = 0
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyDeviceIsAlive, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        guard AudioObjectGetPropertyData(device.id, &address, 0, nil, &size, &alive) == noErr, alive != 0 else { return false }
        var hardware = AudioStreamBasicDescription()
        size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        return AudioUnitGetProperty(unit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, 1, &hardware, &size) == noErr
            && hardware.mSampleRate == format.sampleRate
            && hardware.mChannelsPerFrame == format.channelCount
    }

    isolated deinit {
        AudioOutputUnitStop(unit)
        AudioUnitUninitialize(unit)
        AudioComponentInstanceDispose(unit)
    }

    nonisolated private static let render: AURenderCallback = { context, flags, time, _, frames, _ in
        let state = Unmanaged<InputCallbackState>.fromOpaque(context).takeUnretainedValue()
        guard frames <= state.buffer.frameCapacity else {
            state.errorStatus.store(kAudioUnitErr_TooManyFramesToProcess, ordering: .releasing)
            return kAudioUnitErr_TooManyFramesToProcess
        }
        state.buffer.frameLength = frames
        let status = AudioUnitRender(state.unit, flags, time, 1, frames, state.buffer.mutableAudioBufferList)
        if status == noErr {
            state.onAudio(state.buffer)
        } else {
            state.errorStatus.store(status, ordering: .releasing)
        }
        return status
    }

    private static func check(_ status: OSStatus) throws {
        guard status == noErr else { throw MicrophoneDevices.failure("The microphone could not start (\(status)).") }
    }
}

struct MicrophoneCallbackReservation {
    private(set) var isReserved = false

    mutating func reserve() throws {
        guard !isReserved else {
            throw MicrophoneDevices.failure("The microphone capture has already started.")
        }
        isReserved = true
    }
}

// Configuration is immutable after startup; only the audio callback touches the buffer.
private final class InputCallbackState {
    let unit: AudioUnit
    let buffer: AVAudioPCMBuffer
    let onAudio: @Sendable (AVAudioPCMBuffer) -> Void
    let errorStatus = Atomic<OSStatus>(noErr)

    init(unit: AudioUnit, buffer: AVAudioPCMBuffer, onAudio: @escaping @Sendable (AVAudioPCMBuffer) -> Void) {
        self.unit = unit
        self.buffer = buffer
        self.onAudio = onAudio
    }
}

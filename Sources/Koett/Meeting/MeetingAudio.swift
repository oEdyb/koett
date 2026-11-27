import AudioToolbox
import AVFoundation
import CoreAudio
import Foundation
import Synchronization

struct MeetingAudioClock: Sendable {
    let zero: UInt64

    init(zero: UInt64 = AudioGetCurrentHostTime()) { self.zero = zero }

    func offset(_ hostTime: UInt64 = AudioGetCurrentHostTime()) -> UInt64 {
        guard hostTime > zero else { return 0 }
        return AudioConvertHostTimeToNanos(hostTime - zero) / 62_500
    }
}

struct MeetingTrackStatus: Codable, Equatable, Sendable {
    enum Health: String, Codable, Sendable { case waiting, receiving, reconnecting }
    var health: Health = .waiting
    var level: Float = 0
    var issue: String? = nil
}

// One audio callback produces and a serial writer queue consumes. The atomics
// publish ownership of each preallocated slot in both directions.
final class MeetingAudioRing: @unchecked Sendable {
    struct Stamp {
        var hostTime: UInt64 = 0
        var frames = 0
        var channels = 0
        var planar = false
        var dropped = false
    }

    let rate: Double
    private let capacity: Int
    private let slotFloats: Int
    private let samples: UnsafeMutablePointer<Float>
    private let stamps: UnsafeMutablePointer<Stamp>
    private let writeIndex = Atomic<Int>(0)
    private let readIndex = Atomic<Int>(0)
    private let overrun = Atomic<Bool>(false)
    private let enabled = Atomic<Bool>(true)
    let received = Atomic<Bool>(false)

    init(rate: Double, capacity: Int = 64, slotFloats: Int = 16_384) {
        self.rate = rate
        self.capacity = capacity
        self.slotFloats = slotFloats
        samples = .allocate(capacity: capacity * slotFloats)
        stamps = .allocate(capacity: capacity)
        stamps.initialize(repeating: .init(), count: capacity)
    }

    deinit {
        samples.deallocate()
        stamps.deinitialize(count: capacity)
        stamps.deallocate()
    }

    func stop() { enabled.store(false, ordering: .releasing) }

    func copy(_ buffers: UnsafePointer<AudioBufferList>, hostTime: UInt64) {
        guard enabled.load(ordering: .acquiring) else { return }
        let list = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffers))
        guard let first = list.first, first.mNumberChannels > 0 else { return }
        let planar = list.count > 1
        let frames = Int(first.mDataByteSize) / (MemoryLayout<Float>.size * Int(first.mNumberChannels))
        let channels = planar ? list.count : Int(first.mNumberChannels)
        guard frames > 0 else { return }
        let write = writeIndex.load(ordering: .relaxed)
        guard frames <= slotFloats / channels, write - readIndex.load(ordering: .acquiring) < capacity else {
            overrun.store(true, ordering: .relaxed)
            return
        }
        let index = write % capacity
        let destination = samples.advanced(by: index * slotFloats)
        let bytes = frames * MemoryLayout<Float>.size
        if planar {
            for (channel, buffer) in list.enumerated() {
                guard buffer.mNumberChannels == 1, buffer.mDataByteSize == bytes, let data = buffer.mData else { return }
                memcpy(destination.advanced(by: channel * frames), data, bytes)
            }
        } else {
            guard let data = first.mData else { return }
            memcpy(destination, data, bytes * channels)
        }
        stamps[index] = .init(hostTime: hostTime, frames: frames, channels: channels, planar: planar,
                              dropped: overrun.exchange(false, ordering: .relaxed))
        writeIndex.store(write + 1, ordering: .releasing)
        received.store(true, ordering: .releasing)
    }

    func drain(_ consume: (Stamp, UnsafeBufferPointer<Float>) throws -> Void) rethrows {
        var read = readIndex.load(ordering: .relaxed)
        let end = writeIndex.load(ordering: .acquiring)
        while read < end {
            let index = read % capacity
            let stamp = stamps[index]
            try consume(stamp, .init(start: samples.advanced(by: index * slotFloats), count: stamp.frames * stamp.channels))
            read += 1
            readIndex.store(read, ordering: .releasing)
        }
    }

    var hasTrailingOverrun: Bool { overrun.load(ordering: .acquiring) }
}

@MainActor
protocol MeetingAudioSource: AnyObject {
    var ring: MeetingAudioRing { get }
    var isConnected: Bool { get }
    func stop()
}

@MainActor
final class MeetingMicrophone: MeetingAudioSource {
    let ring: MeetingAudioRing
    private let input: MicrophoneInput
    private let resolve: () throws -> MicrophoneDevice
    private let device: MicrophoneDevice

    init(resolve: @escaping () throws -> MicrophoneDevice) throws {
        self.resolve = resolve
        device = try resolve()
        input = try MicrophoneInput(device: device)
        let ring = MeetingAudioRing(rate: input.format.sampleRate)
        self.ring = ring
        try input.startTimestamped { buffer, hostTime in ring.copy(buffer.audioBufferList, hostTime: hostTime) }
    }

    var isConnected: Bool { input.isConnected && input.error == nil && (try? resolve().id) == device.id }
    func stop() { ring.stop(); input.stop() }
}

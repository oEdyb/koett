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
    let channels: Int
    private let capacity: Int
    private let samples: UnsafeMutablePointer<Float>
    private let stamps: UnsafeMutablePointer<Stamp>
    private let writeIndex = Atomic<Int>(0)
    private let readIndex = Atomic<Int>(0)
    private let overrun = Atomic<Bool>(false)
    private let enabled = Atomic<Bool>(true)
    let received = Atomic<Bool>(false)
    let lastCallback = Atomic<UInt64>(0)

    init(rate: Double, channels: Int = 1, capacityFrames: Int? = nil) {
        self.rate = rate
        self.channels = channels
        capacity = capacityFrames ?? Int((rate * 8).rounded(.up))
        samples = .allocate(capacity: capacity * channels)
        stamps = .allocate(capacity: capacity)
    }

    deinit {
        samples.deallocate()
        stamps.deallocate()
    }

    func stop() { enabled.store(false, ordering: .releasing) }

    func copy(_ buffers: UnsafePointer<AudioBufferList>, hostTime: UInt64) {
        guard enabled.load(ordering: .acquiring) else { return }
        let list = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffers))
        guard let first = list.first else { return }
        let planar = list.count > 1
        let bufferChannels = planar ? 1 : channels
        let frameBytes = bufferChannels * MemoryLayout<Float>.size
        let frames = Int(first.mDataByteSize) / frameBytes
        guard frames > 0, (planar ? list.count == channels : list.count == 1),
              list.allSatisfy({ $0.mNumberChannels == bufferChannels && $0.mDataByteSize == frames * frameBytes && $0.mData != nil })
        else { return }
        lastCallback.store(AudioGetCurrentHostTime(), ordering: .releasing)
        let write = writeIndex.load(ordering: .relaxed)
        guard frames <= capacity - (write - readIndex.load(ordering: .acquiring)) else {
            overrun.store(true, ordering: .relaxed)
            return
        }
        let index = write % capacity
        let floatCapacity = capacity * channels
        let floatsPerBuffer = frames * bufferChannels
        for (channel, buffer) in list.enumerated() {
            guard let data = buffer.mData else { return }
            let start = (index * channels + channel * floatsPerBuffer) % floatCapacity
            let head = min(floatsPerBuffer, floatCapacity - start) * MemoryLayout<Float>.size
            memcpy(samples.advanced(by: start), data, head)
            let tail = Int(buffer.mDataByteSize) - head
            if tail > 0 { memcpy(samples, data.advanced(by: head), tail) }
        }
        stamps.advanced(by: index).initialize(to: .init(hostTime: hostTime, frames: frames, channels: channels, planar: planar,
                              dropped: overrun.exchange(false, ordering: .relaxed)))
        writeIndex.store(write + frames, ordering: .releasing)
        received.store(true, ordering: .releasing)
    }

    func drain(_ consume: (Stamp, UnsafeBufferPointer<Float>) throws -> Void) rethrows {
        var read = readIndex.load(ordering: .relaxed)
        let end = writeIndex.load(ordering: .acquiring)
        while read < end {
            let index = read % capacity
            let stamp = stamps[index]
            let head = min(stamp.frames, capacity - index) * channels
            let source = UnsafeBufferPointer(start: samples.advanced(by: index * channels), count: head)
            let tail = stamp.frames * channels - head
            if tail > 0 {
                var packet = Array(source)
                packet.append(contentsOf: UnsafeBufferPointer(start: samples, count: tail))
                try packet.withUnsafeBufferPointer { try consume(stamp, $0) }
            } else {
                try consume(stamp, source)
            }
            read += stamp.frames
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
        let ring = MeetingAudioRing(rate: input.format.sampleRate, channels: Int(input.format.channelCount))
        self.ring = ring
        try input.startTimestamped { buffer, hostTime in ring.copy(buffer.audioBufferList, hostTime: hostTime) }
    }

    var isConnected: Bool { input.isConnected && input.error == nil && (try? resolve().id) == device.id }
    func stop() { ring.stop(); input.stop() }
}

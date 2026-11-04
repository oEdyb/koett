import AVFoundation
import Foundation
import Synchronization

final class PreviewPower: Sendable {
    private let bits = Atomic<UInt32>(0)
    var level: Float { Float(bitPattern: bits.load(ordering: .relaxed)) }
    func reset() { bits.store(0, ordering: .relaxed) }

    func append(_ buffer: AVAudioPCMBuffer) {
        guard let channels = buffer.floatChannelData, buffer.frameLength > 0 else { return }
        var rms: Float = 0
        for channel in 0..<Int(buffer.format.channelCount) {
            var sum: Float = 0
            for frame in 0..<Int(buffer.frameLength) { sum += channels[channel][frame] * channels[channel][frame] }
            rms = max(rms, sqrt(sum / Float(buffer.frameLength)))
        }
        bits.store(min(1, rms).bitPattern, ordering: .relaxed)
    }
}

@MainActor
final class InputLevelPreview {
    private let makeInput: (MicrophoneDevice) throws -> any MicrophoneCapturing
    private var input: (any MicrophoneCapturing)?
    let power = PreviewPower()
    var error: Error? { input?.error }
    var isConnected: Bool { input?.isConnected ?? false }

    init(makeInput: @escaping (MicrophoneDevice) throws -> any MicrophoneCapturing = { try MicrophoneInput(device: $0) }) {
        self.makeInput = makeInput
    }

    func start(device: MicrophoneDevice) throws {
        stop()
        let next = try makeInput(device)
        do {
            let power = power
            try next.start { power.append($0) }
            input = next
        } catch {
            next.stop()
            throw error
        }
    }

    func stop() {
        input?.stop()
        input = nil
        power.reset()
    }

    isolated deinit { input?.stop() }
}

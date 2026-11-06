import Foundation

struct RecordingSoundPreferences {
    static let enabledKey = "recordingSounds"
    static let volumeKey = "recordingSoundVolume"
    static let defaultVolume = 0.5
    var enabled: Bool
    var volume: Double

    init(defaults: UserDefaults) {
        enabled = defaults.bool(forKey: Self.enabledKey)
        let saved = (defaults.object(forKey: Self.volumeKey) as? NSNumber)?.doubleValue ?? Self.defaultVolume
        volume = saved.isFinite ? min(1, max(0, saved)) : Self.defaultVolume
    }

    var playbackVolume: Float? { enabled && volume > 0 ? Float(volume) : nil }

    func save(to defaults: UserDefaults) {
        defaults.set(enabled, forKey: Self.enabledKey)
        defaults.set(volume.isFinite ? min(1, max(0, volume)) : Self.defaultVolume, forKey: Self.volumeKey)
    }
}

enum RecordingStartCue {
    static let sampleRate = 44_100
    static let frequency = 440.0
    static let duration = 0.16

    static func samples() -> [Int16] {
        let count = Int(Double(sampleRate) * duration)
        return (0..<count).map { index in
            let time = Double(index) / Double(sampleRate)
            let attack = min(1, time / 0.008)
            let release = min(1, Double(count - 1 - index) / (0.02 * Double(sampleRate)))
            let phase = 2 * Double.pi * frequency * time
            let wave = sin(phase) + 0.15 * sin(2 * phase)
            return Int16(0.24 / 1.15 * wave * exp(-time / 0.055) * attack * release * Double(Int16.max))
        }
    }

    static func waveData() -> Data {
        let samples = samples()
        var data = Data()
        func word<T: FixedWidthInteger>(_ value: T) {
            var value = value.littleEndian
            withUnsafeBytes(of: &value) { data.append(contentsOf: $0) }
        }
        data.append(contentsOf: "RIFF".utf8); word(UInt32(36 + samples.count * 2))
        data.append(contentsOf: "WAVEfmt ".utf8); word(UInt32(16))
        word(UInt16(1)); word(UInt16(1)); word(UInt32(sampleRate)); word(UInt32(sampleRate * 2))
        word(UInt16(2)); word(UInt16(16)); data.append(contentsOf: "data".utf8)
        word(UInt32(samples.count * 2))
        for sample in samples { word(sample) }
        return data
    }
}

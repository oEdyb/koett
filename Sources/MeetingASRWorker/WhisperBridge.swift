import Darwin
import Foundation
import MeetingASRProtocol

final class WhisperBridge {
    typealias Open = @convention(c) (UnsafePointer<CChar>) -> OpaquePointer?
    typealias Close = @convention(c) (OpaquePointer) -> Void
    typealias Token = @convention(c) (UnsafeMutableRawPointer, UnsafePointer<CChar>, Int32, Int32) -> Void
    typealias Span = @convention(c) (UnsafeMutableRawPointer, Int32, Int32) -> Void
    typealias Detect = @convention(c) (OpaquePointer, UnsafePointer<Float>, Int32, UnsafeMutablePointer<Float>) -> Int32
    typealias Decode = @convention(c) (OpaquePointer, UnsafePointer<Float>, Int32, UnsafePointer<CChar>, UnsafeMutableRawPointer, Token) -> Int32
    typealias VAD = @convention(c) (OpaquePointer, UnsafePointer<Float>, Int32, UnsafeMutableRawPointer, Span) -> Int32

    private let library: UnsafeMutableRawPointer
    private let openModel: Open
    private let closeModel: Close
    private let openVAD: Open
    private let closeVAD: Close
    private let detect: Detect
    private let decode: Decode
    private let vad: VAD
    private var models: [String: OpaquePointer] = [:]
    private var vadModel: OpaquePointer?

    init(library path: String) throws {
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            throw MeetingASRError.model("Metal runtime")
        }
        func symbol<T>(_ name: String, _: T.Type) throws -> T {
            guard let pointer = dlsym(library, name) else { throw MeetingASRError.model("Metal runtime") }
            return unsafeBitCast(pointer, to: T.self)
        }
        do {
            openModel = try symbol("koett_whisper_open", Open.self)
            closeModel = try symbol("koett_whisper_close", Close.self)
            openVAD = try symbol("koett_vad_open", Open.self)
            closeVAD = try symbol("koett_vad_close", Close.self)
            detect = try symbol("koett_whisper_language", Detect.self)
            decode = try symbol("koett_whisper_decode", Decode.self)
            vad = try symbol("koett_vad_find", VAD.self)
            self.library = library
        } catch {
            dlclose(library)
            throw error
        }
    }

    deinit {
        if let vadModel { closeVAD(vadModel) }
        for model in models.values { closeModel(model) }
        dlclose(library)
    }

    func model(_ name: String, path: () throws -> URL) throws -> OpaquePointer {
        if let model = models[name] { return model }
        let location = try path()
        guard let model = openModel(location.path) else { throw MeetingASRError.model(name) }
        models[name] = model
        return model
    }

    func language(_ model: OpaquePointer, _ samples: [Float]) throws -> (String, Float) {
        var scores: [Float] = [0, 0]
        guard detect(model, samples, Int32(samples.count), &scores) == 0 else {
            throw MeetingASRError.inference("Whisper could not identify the spoken language.")
        }
        let total = scores[0] + scores[1]
        return (scores[1] > scores[0] ? "sv" : "en", total > 0 ? max(scores[0], scores[1]) / total : 0)
    }

    func words(_ model: OpaquePointer, _ samples: [Float], _ language: String) throws -> [Word] {
        let sink = WordSink()
        let status = decode(model, samples, Int32(samples.count), language, Unmanaged.passUnretained(sink).toOpaque()) {
            context, text, first, end in
            let sink = Unmanaged<WordSink>.fromOpaque(context).takeUnretainedValue()
            let bytes = UnsafeRawBufferPointer(start: text, count: strlen(text))
            if bytes.first == 32 { sink.flush() }
            if sink.text.isEmpty { sink.first = Int(first) }
            sink.text.append(contentsOf: bytes)
            sink.end = Int(end)
        }
        guard status == 0 else { throw MeetingASRError.inference("Whisper could not decode this audio window.") }
        sink.flush()
        return sink.words
    }

    func speech(_ samples: [Float], path: () throws -> URL) throws -> [Range<Int>] {
        if vadModel == nil { vadModel = openVAD(try path().path) }
        guard let vadModel else { throw MeetingASRError.model(MeetingModelPins.vad) }
        let sink = SpanSink()
        let status = vad(vadModel, samples, Int32(samples.count), Unmanaged.passUnretained(sink).toOpaque()) { context, first, end in
            let sink = Unmanaged<SpanSink>.fromOpaque(context).takeUnretainedValue()
            if first >= 0, end > first { sink.spans.append(Int(first)..<Int(end)) }
        }
        guard status == 0 else { throw MeetingASRError.inference("Speech detection failed.") }
        return sink.spans.map { $0.clamped(to: 0..<samples.count) }.filter { !$0.isEmpty }
    }

    private final class SpanSink { var spans: [Range<Int>] = [] }

    private final class WordSink {
        var text = Data()
        var first = 0
        var end = 0
        var words: [Word] = []

        func flush() {
            let value = String(decoding: text, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            if !value.isEmpty { words.append(.init(text: value, first: first, end: end)) }
            text.removeAll(keepingCapacity: true)
        }
    }
}

struct Word {
    let text: String
    let first: Int
    let end: Int
}

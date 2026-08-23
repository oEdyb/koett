import AVFoundation
import Foundation

enum AssistantVoice: String, CaseIterable, Sendable {
    case katie
    case skylar
    case jameson
    case gemma
    case archie

    var displayName: String {
        rawValue.capitalized
    }

    var id: String {
        switch self {
        case .katie: "f786b574-daa5-4673-aa0c-cbe3e8534c02"
        case .skylar: "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"
        case .jameson: "a5136bf9-224c-4d76-b823-52bd5efcffcc"
        case .gemma: "62ae83ad-4f6a-430b-af41-a9bede9286ca"
        case .archie: "ef191366-f52f-447a-a398-ed8c0f2943a1"
        }
    }
}

struct SpokenLeadBuffer {
    private static let maximumSentences = 3

    private var pending = ""
    private var sentenceCount = 0
    private var stopped = false

    mutating func append(_ text: String) -> [String] {
        guard !stopped else { return [] }
        pending += text

        var reachedDetailBoundary = false
        if let boundary = detailBoundary(in: pending) {
            pending = String(pending[..<boundary])
            stopped = true
            reachedDetailBoundary = true
        }

        var sentences = removeCompleteSentences()
        if reachedDetailBoundary, !pending.isEmpty {
            if let finalText = Self.clean(pending) {
                sentences.append(finalText)
            }
            pending = ""
        }
        return limit(sentences)
    }

    mutating func finish() -> [String] {
        guard !pending.isEmpty else { return [] }
        defer { pending = "" }
        guard let text = Self.clean(pending) else { return [] }
        return limit([text])
    }

    private mutating func removeCompleteSentences() -> [String] {
        var boundary: String.Index?
        var index = pending.startIndex
        while index < pending.endIndex {
            let character = pending[index]
            let next = pending.index(after: index)
            if ".!?".contains(character),
               next == pending.endIndex || pending[next].isWhitespace {
                boundary = next
            }
            index = next
        }

        guard let boundary else { return [] }
        let complete = String(pending[..<boundary])
        pending = String(pending[boundary...])
        return complete
            .splitSentences()
            .compactMap(Self.clean)
    }

    private mutating func limit(_ sentences: [String]) -> [String] {
        guard sentenceCount < Self.maximumSentences else {
            stopped = true
            pending = ""
            return []
        }
        let remaining = Self.maximumSentences - sentenceCount
        let accepted = Array(sentences.prefix(remaining))
        sentenceCount += accepted.count
        if sentenceCount == Self.maximumSentences {
            stopped = true
            pending = ""
        }
        return accepted
    }

    private func detailBoundary(in text: String) -> String.Index? {
        ["\r\n\r\n", "\n\n", "```"].compactMap {
            text.range(of: $0)?.lowerBound
        }.min()
    }

    private static func clean(_ source: String) -> String? {
        var text = source
        text = text.replacingOccurrences(
            of: #"\[([^\]]+)\]\([^\)]+\)"#,
            with: "$1",
            options: .regularExpression
        )
        text = text.replacingOccurrences(
            of: #"https?://\S+"#,
            with: "",
            options: .regularExpression
        )
        text = text.replacingOccurrences(
            of: #"(?m)^\s*(?:#{1,6}|[-+*>])\s*"#,
            with: "",
            options: .regularExpression
        )
        text = text.replacingOccurrences(
            of: #"[*_~`$]"#,
            with: "",
            options: .regularExpression
        )
        text = text.replacingOccurrences(
            of: #"\s+"#,
            with: " ",
            options: .regularExpression
        )
        text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }
}

private extension String {
    func splitSentences() -> [String] {
        var result: [String] = []
        var start = startIndex
        var index = startIndex

        while index < endIndex {
            let character = self[index]
            let next = self.index(after: index)
            if ".!?".contains(character),
               next == endIndex || self[next].isWhitespace {
                result.append(String(self[start..<next]))
                start = next
            }
            index = next
        }

        if start < endIndex {
            result.append(String(self[start...]))
        }
        return result
    }
}

@MainActor
final class CartesiaSpeechOutput {
    private struct ServerEvent: Decodable {
        let type: String
        let data: String?
        let message: String?
        let contextID: String?

        enum CodingKeys: String, CodingKey {
            case type
            case data
            case message
            case contextID = "context_id"
        }
    }

    private static let endpoint = URL(
        string: "wss://api.cartesia.ai/tts/websocket?cartesia_version=2026-03-01"
    )!
    private static let model = "sonic-3.5-2026-05-04"
    private static let sampleRate = 44_100.0

    private let session: URLSession
    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let format: AVAudioFormat
    private var socket: URLSessionWebSocketTask?
    private var receiveTask: Task<Void, Never>?
    private var connectionID = UUID()
    private var contextID = UUID().uuidString
    private var voice = AssistantVoice.katie
    private var spokenLead = SpokenLeadBuffer()
    private var sentInput = false
    private var firstSendNanoseconds: UInt64?
    private var loggedFirstAudio = false

    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.urlCache = nil
        configuration.httpCookieStorage = nil
        session = URLSession(configuration: configuration)
        format = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: Self.sampleRate,
            channels: 1,
            interleaved: false
        )!
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)
        engine.prepare()
    }

    func prepare(apiKey: String, voice: AssistantVoice) {
        stop()
        self.voice = voice
        spokenLead = SpokenLeadBuffer()
        contextID = UUID().uuidString
        connectionID = UUID()
        sentInput = false
        firstSendNanoseconds = nil
        loggedFirstAudio = false

        var request = URLRequest(url: Self.endpoint)
        request.timeoutInterval = 30
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        request.setValue("2026-03-01", forHTTPHeaderField: "Cartesia-Version")
        let newSocket = session.webSocketTask(with: request)
        socket = newSocket
        let currentConnectionID = connectionID
        receiveTask = Task { @MainActor [weak self] in
            await self?.receive(
                from: newSocket,
                connectionID: currentConnectionID
            )
        }
        newSocket.resume()
    }

    func appendResponseDelta(_ delta: String) async {
        for sentence in spokenLead.append(delta) {
            await send(sentence, continueGeneration: true)
        }
    }

    func finishResponse() async {
        let remainder = spokenLead.finish()
        if remainder.isEmpty {
            if sentInput {
                await send("", continueGeneration: false)
            }
            return
        }

        for (index, text) in remainder.enumerated() {
            let isLast = index == remainder.index(before: remainder.endIndex)
            await send(text, continueGeneration: !isLast)
        }
    }

    func stop() {
        receiveTask?.cancel()
        receiveTask = nil
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
        player.stop()
        player.reset()
        engine.stop()
        spokenLead = SpokenLeadBuffer()
        sentInput = false
    }

    private func send(_ text: String, continueGeneration: Bool) async {
        guard let socket else { return }
        let body: [String: Any] = [
            "model_id": Self.model,
            "transcript": text,
            "voice": ["mode": "id", "id": voice.id],
            "language": "en",
            "context_id": contextID,
            "output_format": [
                "container": "raw",
                "encoding": "pcm_f32le",
                "sample_rate": Int(Self.sampleRate),
            ],
            "continue": continueGeneration,
        ]

        do {
            let data = try JSONSerialization.data(withJSONObject: body)
            guard let message = String(data: data, encoding: .utf8) else { return }
            if firstSendNanoseconds == nil {
                firstSendNanoseconds = DispatchTime.now().uptimeNanoseconds
            }
            try await socket.send(.string(message))
            sentInput = true
        } catch {
            fail(error)
        }
    }

    private func receive(
        from socket: URLSessionWebSocketTask,
        connectionID: UUID
    ) async {
        do {
            while !Task.isCancelled, connectionID == self.connectionID {
                let message = try await socket.receive()
                let data: Data
                switch message {
                case let .data(value):
                    data = value
                case let .string(value):
                    guard let value = value.data(using: .utf8) else { continue }
                    data = value
                @unknown default:
                    continue
                }

                let event = try JSONDecoder().decode(ServerEvent.self, from: data)
                guard event.contextID == nil || event.contextID == contextID else {
                    continue
                }
                if event.type == "error" {
                    throw failure(event.message ?? "Cartesia could not generate speech.")
                }
                if event.type == "chunk",
                   let encodedAudio = event.data,
                   let audio = Data(base64Encoded: encodedAudio) {
                    try play(audio)
                }
            }
        } catch is CancellationError {
            return
        } catch {
            guard connectionID == self.connectionID else { return }
            fail(error)
        }
    }

    private func play(_ data: Data) throws {
        let bytesPerFrame = MemoryLayout<Float>.size
        guard !data.isEmpty, data.count.isMultiple(of: bytesPerFrame) else {
            throw failure("Cartesia returned invalid audio data.")
        }
        let frameCount = AVAudioFrameCount(data.count / bytesPerFrame)
        guard let buffer = AVAudioPCMBuffer(
            pcmFormat: format,
            frameCapacity: frameCount
        ), let channel = buffer.floatChannelData?.pointee else {
            throw failure("Koett could not prepare Cartesia audio.")
        }
        buffer.frameLength = frameCount
        data.copyBytes(
            to: UnsafeMutableRawPointer(channel).assumingMemoryBound(to: UInt8.self),
            count: data.count
        )

        if !engine.isRunning {
            try engine.start()
        }
        player.scheduleBuffer(buffer)
        if !player.isPlaying {
            player.play()
        }

        if !loggedFirstAudio, let firstSendNanoseconds {
            loggedFirstAudio = true
            let elapsed = Double(
                DispatchTime.now().uptimeNanoseconds - firstSendNanoseconds
            ) / 1_000_000
            print(String(format: "CARTESIA FIRST AUDIO %.1f ms", elapsed))
        }
    }

    private func fail(_ error: Error) {
        fputs("Warning: Cartesia speech stopped: \(error.localizedDescription)\n", stderr)
        stop()
    }

    private func failure(_ message: String) -> NSError {
        NSError(
            domain: "Koett.Cartesia",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}

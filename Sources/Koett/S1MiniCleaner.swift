import AppKit
import CryptoKit
import Foundation

private final class S1MiniDownloadProgressDelegate: NSObject,
    URLSessionDownloadDelegate,
    @unchecked Sendable
{
    private let progressHandler: @Sendable (Double) -> Void

    init(progressHandler: @escaping @Sendable (Double) -> Void) {
        self.progressHandler = progressHandler
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {}

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        guard totalBytesExpectedToWrite > 0 else { return }
        progressHandler(
            Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)
        )
    }
}

enum S1MiniStyle: String, CaseIterable, Sendable {
    case casual
    case semiCasual = "semi-casual"
    case semiFormal = "semi-formal"
    case formal

    var displayName: String {
        switch self {
        case .casual: "Casual"
        case .semiCasual: "Semi-casual"
        case .semiFormal: "Semi-formal"
        case .formal: "Formal"
        }
    }
}

enum S1MiniStructure: String, CaseIterable, Sendable {
    case prose
    case lists

    var displayName: String {
        switch self {
        case .prose: "Prose"
        case .lists: "Lists"
        }
    }
}

enum S1MiniContext: String, CaseIterable, Sendable {
    case general
    case email

    var displayName: String {
        switch self {
        case .general: "General"
        case .email: "Email"
        }
    }
}

struct S1MiniCompletionRequest: Encodable, Sendable {
    struct Message: Encodable, Sendable {
        let role: String
        let content: String
    }

    struct ChatTemplateArguments: Encodable, Sendable {
        let enableThinking = false

        enum CodingKeys: String, CodingKey {
            case enableThinking = "enable_thinking"
        }
    }

    static let systemPrompt =
        "You are a text normalizer for speech-to-text transcripts. The input begins "
        + "with a control line specifying the styling, structure, and context settings; "
        + "clean the transcript to match those settings and output only the cleaned text."

    let model = "s1-mini"
    let messages: [Message]
    let temperature = 0
    let maxTokens: Int
    let cachePrompt = true
    let chatTemplateArguments = ChatTemplateArguments()

    init(
        transcript: String,
        style: S1MiniStyle,
        structure: S1MiniStructure,
        context: S1MiniContext,
        inputTokens: Int
    ) {
        let control = "[Styling: \(style.rawValue)] "
            + "[Structure: \(structure.rawValue)] "
            + "[Context: \(context.rawValue)]"
        messages = [
            Message(role: "system", content: Self.systemPrompt),
            Message(role: "user", content: "\(control)\n\(transcript)"),
        ]
        maxTokens = Int(ceil(Double(inputTokens) * 1.3)) + 32
    }

    enum CodingKeys: String, CodingKey {
        case model
        case messages
        case temperature
        case maxTokens = "max_tokens"
        case cachePrompt = "cache_prompt"
        case chatTemplateArguments = "chat_template_kwargs"
    }
}

@MainActor
final class S1MiniCleaner {
    private struct TokenizeRequest: Encodable {
        let content: String
        let addSpecial = false

        enum CodingKeys: String, CodingKey {
            case content
            case addSpecial = "add_special"
        }
    }

    private struct TokenizeResponse: Decodable {
        let tokens: [Int]
    }

    private struct CompletionResponse: Decodable {
        struct Choice: Decodable {
            struct Message: Decodable {
                let content: String
            }

            let message: Message
        }

        let choices: [Choice]
    }

    private static let modelRevision = "ee2c0f56e56345f475749a44ff2893e21c3cb292"
    nonisolated private static let modelBytes = 484_219_808
    nonisolated private static let modelSHA256 =
        "3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634"
    private static let downloadURL = URL(
        string: "https://huggingface.co/superwhisper/s1-mini-GGUF/resolve/"
            + modelRevision
            + "/s1-mini-q4_k_m.gguf?download=true"
    )!

    private let port: Int
    private let session: URLSession
    private var serverProcess: Process?

    init(port: Int = 18_181, session: URLSession = .shared) {
        self.port = port
        self.session = session
    }

    func prepare(
        progressHandler: (@Sendable (Double) -> Void)? = nil
    ) async throws {
        if serverProcess?.isRunning == true { return }

        let executable = try Self.serverExecutableURL()
        let model = try await prepareModel(progressHandler: progressHandler)
        let process = Process()
        process.executableURL = executable
        process.arguments = [
            "--model", model.path,
            "--host", "127.0.0.1",
            "--port", String(port),
            "--jinja",
            "--chat-template-kwargs", #"{"enable_thinking":false}"#,
            "--temp", "0",
            "--parallel", "1",
            "--ctx-size", "3072",
            "--batch-size", "1024",
            "--ubatch-size", "1024",
            "--threads", "1",
            "--threads-batch", "1",
            "--poll", "0",
            "--flash-attn", "on",
            "--n-gpu-layers", "99",
            "--cache-prompt",
            "--spec-type", "ngram-simple",
            "--spec-ngram-simple-size-n", "7",
            "--spec-ngram-simple-size-m", "48",
            "--spec-ngram-simple-min-hits", "1",
            "--no-ui",
            "--log-disable",
        ]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.standardError
        try process.run()
        serverProcess = process

        do {
            try await waitUntilReady()
        } catch {
            stop()
            throw error
        }
    }

    func clean(
        _ transcript: String,
        style: S1MiniStyle,
        structure: S1MiniStructure,
        context: S1MiniContext
    ) async throws -> String {
        guard !transcript.isEmpty else { return "" }
        try await prepare()

        let tokenRequest = TokenizeRequest(content: transcript)
        let tokenData = try await request(path: "tokenize", body: tokenRequest)
        let inputTokens = try JSONDecoder().decode(TokenizeResponse.self, from: tokenData)
            .tokens.count
        let completionRequest = S1MiniCompletionRequest(
            transcript: transcript,
            style: style,
            structure: structure,
            context: context,
            inputTokens: inputTokens
        )
        let completionData = try await request(
            path: "v1/chat/completions",
            body: completionRequest,
            timeout: 180
        )
        let response = try JSONDecoder().decode(
            CompletionResponse.self,
            from: completionData
        )
        guard let content = response.choices.first?.message.content else {
            throw Self.failure("S1-mini returned no text result.")
        }
        return content.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func stop() {
        guard let serverProcess else { return }
        if serverProcess.isRunning {
            serverProcess.terminate()
        }
        self.serverProcess = nil
    }

    private func prepareModel(
        progressHandler: (@Sendable (Double) -> Void)?
    ) async throws -> URL {
        let directory = URL.cachesDirectory
            .appendingPathComponent("Koett", isDirectory: true)
            .appendingPathComponent("Models", isDirectory: true)
            .appendingPathComponent("S1-mini", isDirectory: true)
        let model = directory.appendingPathComponent("s1-mini-q4_k_m.gguf")
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true
        )

        if FileManager.default.fileExists(atPath: model.path) {
            if try await Self.modelIsValid(model) {
                progressHandler?(1)
                return model
            }
            try FileManager.default.removeItem(at: model)
        }

        print("Downloading S1-mini by Superwhisper (462 MiB)...")
        let delegate = progressHandler.map {
            S1MiniDownloadProgressDelegate(progressHandler: $0)
        }
        let (temporary, response) = try await session.download(
            from: Self.downloadURL,
            delegate: delegate
        )
        defer { try? FileManager.default.removeItem(at: temporary) }
        guard let http = response as? HTTPURLResponse,
              (200..<300).contains(http.statusCode) else {
            throw Self.failure("S1-mini could not be downloaded.")
        }
        guard try await Self.modelIsValid(temporary) else {
            throw Self.failure("The S1-mini download failed its checksum check.")
        }
        try FileManager.default.moveItem(at: temporary, to: model)
        return model
    }

    private func waitUntilReady() async throws {
        for _ in 0..<1_200 {
            guard serverProcess?.isRunning == true else {
                throw Self.failure("llama-server stopped while loading S1-mini.")
            }
            var request = URLRequest(url: endpoint("health"))
            request.timeoutInterval = 0.25
            if let (data, response) = try? await session.data(for: request),
               let http = response as? HTTPURLResponse,
               http.statusCode == 200,
               let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               object["status"] as? String == "ok" {
                print("S1-mini by Superwhisper is ready.")
                return
            }
            try await Task.sleep(for: .milliseconds(25))
        }
        throw Self.failure("S1-mini did not finish loading within 30 seconds.")
    }

    private func request<Body: Encodable>(
        path: String,
        body: Body,
        timeout: TimeInterval = 30
    ) async throws -> Data {
        var request = URLRequest(url: endpoint(path))
        request.httpMethod = "POST"
        request.timeoutInterval = timeout
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(body)
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse,
              (200..<300).contains(http.statusCode) else {
            throw Self.failure("S1-mini's local server returned an error.")
        }
        return data
    }

    private func endpoint(_ path: String) -> URL {
        URL(string: "http://127.0.0.1:\(port)/\(path)")!
    }

    private static func serverExecutableURL() throws -> URL {
        var candidates = [
            "/opt/homebrew/bin/llama-server",
            "/usr/local/bin/llama-server",
        ]
        if let path = ProcessInfo.processInfo.environment["PATH"] {
            candidates += path.split(separator: ":").map {
                URL(fileURLWithPath: String($0))
                    .appendingPathComponent("llama-server")
                    .path
            }
        }
        guard let path = candidates.first(where: {
            FileManager.default.isExecutableFile(atPath: $0)
        }) else {
            throw failure(
                "S1-mini needs llama.cpp. Install it with: brew install llama.cpp"
            )
        }
        return URL(fileURLWithPath: path)
    }

    nonisolated private static func modelIsValid(_ url: URL) async throws -> Bool {
        try await Task.detached(priority: .utility) {
            let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
            guard let size = attributes[.size] as? NSNumber,
                  size.intValue == Self.modelBytes else {
                return false
            }
            let handle = try FileHandle(forReadingFrom: url)
            defer { try? handle.close() }
            var hasher = SHA256()
            while let data = try handle.read(upToCount: 1_048_576), !data.isEmpty {
                hasher.update(data: data)
            }
            let digest = hasher.finalize().map { String(format: "%02x", $0) }.joined()
            return digest == Self.modelSHA256
        }.value
    }

    nonisolated private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "Koett.S1Mini",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}

extension KoettController {
    func useRawDictationText() {
        guard state == .ready, !isPreparing else { return }
        stopS1Mini()
        s1MiniEnabled = false
        UserDefaults.standard.set(false, forKey: "s1MiniEnabled")
        rebuildMenu()
        formattingPopover.refresh()
    }

    func useS1MiniText() {
        enableS1Mini()
    }

    func stopS1Mini() {
        s1MiniCleaner?.stop()
        s1MiniCleaner = nil
    }

    func setS1MiniStyle(_ style: S1MiniStyle) {
        guard state == .ready, !isPreparing else { return }
        s1MiniStyle = style
        UserDefaults.standard.set(style.rawValue, forKey: "s1MiniStyle")
        rebuildMenu()
        formattingPopover.refresh()
    }

    func setS1MiniStructure(_ structure: S1MiniStructure) {
        guard state == .ready, !isPreparing else { return }
        s1MiniStructure = structure
        UserDefaults.standard.set(structure.rawValue, forKey: "s1MiniStructure")
        rebuildMenu()
        formattingPopover.refresh()
    }

    func setS1MiniContext(_ context: S1MiniContext) {
        guard state == .ready, !isPreparing else { return }
        s1MiniContext = context
        UserDefaults.standard.set(context.rawValue, forKey: "s1MiniContext")
        rebuildMenu()
        formattingPopover.refresh()
    }

    private func enableS1Mini() {
        guard state == .ready, !isPreparing else { return }
        if s1MiniEnabled, s1MiniCleaner != nil {
            rebuildMenu()
            formattingPopover.refresh()
            return
        }

        s1MiniEnabled = true
        UserDefaults.standard.set(true, forKey: "s1MiniEnabled")
        state = .loading
        isPreparing = true
        setSetupStatus(.checkingModel("S1-mini"))
        formattingPopover.refresh()
        Task { @MainActor [weak self] in
            guard let self else { return }
            let cleaner = S1MiniCleaner()
            do {
                try await cleaner.prepare { [weak self] fraction in
                    Task { @MainActor [weak self] in
                        self?.setSetupStatus(
                            .download(model: "S1-mini", fraction: fraction)
                        )
                    }
                }
                self.s1MiniCleaner = cleaner
                self.s1MiniEnabled = true
                UserDefaults.standard.set(true, forKey: "s1MiniEnabled")
                self.recordingOverlay.showTransientStatus("S1-mini is ready")
            } catch {
                cleaner.stop()
                self.s1MiniEnabled = false
                self.showS1MiniError(error)
            }
            self.state = .ready
            self.isPreparing = false
            self.setSetupStatus(.ready, showOverlay: false)
            self.formattingPopover.refresh()
        }
    }

    private func showS1MiniError(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = "S1-mini could not start"
        alert.informativeText = error.localizedDescription
            + "\n\nRaw text is active. Koett will retry S1-mini next launch."
        alert.addButton(withTitle: "OK")
        recordingOverlay.showError("S1-mini unavailable · Raw active")
        _ = NSRunningApplication.current.activate(options: [.activateAllWindows])
        NSApp.activate(ignoringOtherApps: true)
        _ = alert.runModal()
    }
}

import AppKit
import Darwin
import Foundation

struct BrowserMediaSource: Equatable, Sendable {
    let url: URL

    var platformName: String {
        let host = url.host?.lowercased() ?? "Media"
        if Self.matches(host, domain: "youtu.be")
            || Self.matches(host, domain: "youtube.com") {
            return "YouTube"
        }
        if Self.matches(host, domain: "tiktok.com") {
            return "TikTok"
        }
        if Self.matches(host, domain: "instagram.com") {
            return "Instagram"
        }
        if Self.matches(host, domain: "x.com")
            || Self.matches(host, domain: "twitter.com") {
            return "X"
        }
        if Self.matches(host, domain: "vimeo.com") {
            return "Vimeo"
        }
        if Self.matches(host, domain: "facebook.com") {
            return "Facebook"
        }
        if Self.matches(host, domain: "twitch.tv") {
            return "Twitch"
        }
        if Self.matches(host, domain: "soundcloud.com") {
            return "SoundCloud"
        }
        return host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
    }

    init?(url: URL) {
        guard ["http", "https"].contains(url.scheme?.lowercased()),
              url.host?.isEmpty == false else {
            return nil
        }
        self.url = url
    }

    private static func matches(_ host: String, domain: String) -> Bool {
        host == domain || host.hasSuffix(".\(domain)")
    }
}

enum BrowserTabTarget: Equatable, Sendable {
    case safari
    case chromium(applicationName: String)

    static func target(
        bundleIdentifier: String?,
        localizedName: String?
    ) -> BrowserTabTarget? {
        switch bundleIdentifier {
        case "com.apple.Safari":
            return .safari
        case "com.google.Chrome":
            return .chromium(applicationName: "Google Chrome")
        case "com.google.Chrome.beta":
            return .chromium(applicationName: "Google Chrome Beta")
        case "com.google.Chrome.canary":
            return .chromium(applicationName: "Google Chrome Canary")
        case "com.brave.Browser":
            return .chromium(applicationName: "Brave Browser")
        case "com.microsoft.edgemac":
            return .chromium(applicationName: "Microsoft Edge")
        default:
            guard let localizedName,
                  ["Google Chrome", "Safari"].contains(localizedName) else {
                return nil
            }
            return localizedName == "Safari"
                ? .safari
                : .chromium(applicationName: localizedName)
        }
    }

    var appleScript: String {
        switch self {
        case .safari:
            return """
            tell application "Safari"
                if (count of windows) is 0 then error "Safari has no open window."
                return URL of current tab of front window
            end tell
            """
        case .chromium(let applicationName):
            return """
            tell application "\(applicationName)"
                if (count of windows) is 0 then error "\(applicationName) has no open window."
                return URL of active tab of front window
            end tell
            """
        }
    }
}

struct DownloadedMediaAudio: Sendable {
    let source: BrowserMediaSource
    let title: String
    let audioURL: URL
    let temporaryDirectoryURL: URL
}

enum BrowserMedia {
    @concurrent
    static func currentURL(for target: BrowserTabTarget) async throws -> URL {
        let output = try await run(
            executableURL: URL(fileURLWithPath: "/usr/bin/osascript"),
            arguments: ["-e", target.appleScript],
            timeout: 15,
            timeoutMessage: "The browser did not respond."
        )
        let value = output.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: value) else {
            throw failure("Koett could not read the browser tab URL.")
        }
        return url
    }

    @concurrent
    static func fetchAudio(for source: BrowserMediaSource) async throws -> DownloadedMediaAudio {
        guard let ytDLP = executable(named: "yt-dlp") else {
            throw failure("Media support needs yt-dlp. Run: brew install yt-dlp ffmpeg")
        }
        guard let ffmpeg = executable(named: "ffmpeg") else {
            throw failure("Media support needs FFmpeg. Run: brew install yt-dlp ffmpeg")
        }

        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-media-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true
        )

        do {
            let outputTemplate = directoryURL
                .appendingPathComponent("audio.%(ext)s")
                .path
            let title = try await run(
                executableURL: ytDLP,
                arguments: downloadArguments(
                    for: source,
                    ffmpegDirectory: ffmpeg.deletingLastPathComponent().path,
                    outputTemplate: outputTemplate
                ),
                timeout: 30 * 60,
                timeoutMessage: "The media fetch timed out."
            )
            .trimmingCharacters(in: .whitespacesAndNewlines)

            let audioURL = directoryURL.appendingPathComponent("audio.wav")
            let attributes = try FileManager.default.attributesOfItem(atPath: audioURL.path)
            guard let size = attributes[.size] as? NSNumber, size.int64Value > 0 else {
                throw failure("The media audio download was empty.")
            }

            return DownloadedMediaAudio(
                source: source,
                title: title.isEmpty ? "Media Transcript" : title,
                audioURL: audioURL,
                temporaryDirectoryURL: directoryURL
            )
        } catch {
            do {
                try FileManager.default.removeItem(at: directoryURL)
            } catch let cleanupError {
                fputs(
                    "Warning: temporary media audio remains at \(directoryURL.path): \(cleanupError.localizedDescription)\n",
                    stderr
                )
            }
            throw error
        }
    }

    @concurrent
    static func removeTemporaryAudio(at directoryURL: URL) async {
        do {
            try FileManager.default.removeItem(at: directoryURL)
        } catch {
            fputs("Warning: temporary media audio remains at \(directoryURL.path)\n", stderr)
        }
    }

    static func downloadArguments(
        for source: BrowserMediaSource,
        ffmpegDirectory: String,
        outputTemplate: String
    ) -> [String] {
        [
            "--ignore-config",
            "--no-cookies",
            "--no-cookies-from-browser",
            "--no-playlist",
            "--playlist-end", "1",
            "--match-filters", "!is_live & duration<=?10800",
            "--max-filesize", "2G",
            "--socket-timeout", "30",
            "--no-simulate",
            "--no-progress",
            "--no-warnings",
            "--format", "bestaudio/best",
            "--extract-audio",
            "--audio-format", "wav",
            "--audio-quality", "0",
            "--ffmpeg-location", ffmpegDirectory,
            "--output", outputTemplate,
            "--print", "%(title)s",
            source.url.absoluteString,
        ]
    }

    private static func executable(named name: String) -> URL? {
        let candidates = [
            "/opt/homebrew/bin/\(name)",
            "/usr/local/bin/\(name)",
            "/usr/bin/\(name)",
        ]
        return candidates
            .map { URL(fileURLWithPath: $0) }
            .first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }

    static func run(
        executableURL: URL,
        arguments: [String],
        timeout: TimeInterval,
        timeoutMessage: String
    ) async throws -> String {
        let outputURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-process-\(UUID().uuidString).log")
        guard FileManager.default.createFile(atPath: outputURL.path, contents: nil),
              let output = try? FileHandle(forWritingTo: outputURL) else {
            throw failure("Koett could not create temporary process output.")
        }
        defer {
            try? output.close()
            try? FileManager.default.removeItem(at: outputURL)
        }

        var environment = ProcessInfo.processInfo.environment
        environment["PATH"] = [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            environment["PATH"] ?? "/usr/bin:/bin",
        ].joined(separator: ":")
        let processID = try spawnProcessGroup(
            executableURL: executableURL,
            arguments: arguments,
            environment: environment,
            outputFileDescriptor: output.fileDescriptor
        )

        let deadline = Date().addingTimeInterval(timeout)
        var waitStatus: Int32 = 0
        do {
            while true {
                let result = Darwin.waitpid(processID, &waitStatus, WNOHANG)
                if result == processID {
                    break
                }
                if result == -1, errno != EINTR {
                    throw failure("Koett could not wait for the media command.")
                }
                try Task.checkCancellation()
                guard Date() < deadline else {
                    throw failure(timeoutMessage)
                }
                try await Task.sleep(for: .milliseconds(50))
            }
        } catch {
            stopProcessGroup(processID)
            throw error
        }

        try output.synchronize()
        let data = try Data(contentsOf: outputURL)

        let message = String(decoding: data, as: UTF8.self)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard exitCode(from: waitStatus) == 0 else {
            throw failure(message.isEmpty ? "The media command failed." : message)
        }
        return message
    }

    private static func spawnProcessGroup(
        executableURL: URL,
        arguments: [String],
        environment: [String: String],
        outputFileDescriptor: Int32
    ) throws -> pid_t {
        var fileActions: posix_spawn_file_actions_t?
        guard posix_spawn_file_actions_init(&fileActions) == 0 else {
            throw failure("Koett could not prepare the media command.")
        }
        defer { posix_spawn_file_actions_destroy(&fileActions) }
        guard posix_spawn_file_actions_adddup2(
            &fileActions,
            outputFileDescriptor,
            STDOUT_FILENO
        ) == 0,
        posix_spawn_file_actions_adddup2(
            &fileActions,
            outputFileDescriptor,
            STDERR_FILENO
        ) == 0,
        posix_spawn_file_actions_addclose(
            &fileActions,
            outputFileDescriptor
        ) == 0 else {
            throw failure("Koett could not capture media command output.")
        }

        var attributes: posix_spawnattr_t?
        guard posix_spawnattr_init(&attributes) == 0 else {
            throw failure("Koett could not prepare the media command.")
        }
        defer { posix_spawnattr_destroy(&attributes) }
        let flags = Int16(POSIX_SPAWN_SETPGROUP)
        guard posix_spawnattr_setflags(&attributes, flags) == 0,
              posix_spawnattr_setpgroup(&attributes, 0) == 0 else {
            throw failure("Koett could not isolate the media command.")
        }

        let command = [executableURL.path] + arguments
        let environmentStrings = environment
            .map { "\($0.key)=\($0.value)" }
            .sorted()
        var processID: pid_t = 0
        let result = withCStringArray(command) { argumentPointers in
            withCStringArray(environmentStrings) { environmentPointers in
                executableURL.path.withCString { executablePath in
                    posix_spawn(
                        &processID,
                        executablePath,
                        &fileActions,
                        &attributes,
                        argumentPointers,
                        environmentPointers
                    )
                }
            }
        }
        guard result == 0 else {
            throw failure(String(cString: strerror(result)))
        }
        return processID
    }

    private static func withCStringArray<Result>(
        _ strings: [String],
        body: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Result
    ) -> Result {
        let pointers = strings.map { strdup($0) }
        defer { pointers.forEach { free($0) } }
        var nullTerminated = pointers + [nil]
        return nullTerminated.withUnsafeMutableBufferPointer { buffer in
            body(buffer.baseAddress!)
        }
    }

    private static func stopProcessGroup(_ processID: pid_t) {
        _ = Darwin.kill(-processID, SIGTERM)
        let deadline = Date().addingTimeInterval(1)
        var didReapLeader = false
        while processGroupExists(processID), Date() < deadline {
            var waitStatus: Int32 = 0
            if !didReapLeader,
               Darwin.waitpid(processID, &waitStatus, WNOHANG) == processID {
                didReapLeader = true
            }
            Thread.sleep(forTimeInterval: 0.01)
        }
        if processGroupExists(processID) {
            _ = Darwin.kill(-processID, SIGKILL)
        }
        if !didReapLeader {
            var waitStatus: Int32 = 0
            while Darwin.waitpid(processID, &waitStatus, 0) == -1, errno == EINTR {
                continue
            }
        }
    }

    private static func processGroupExists(_ processID: pid_t) -> Bool {
        Darwin.kill(-processID, 0) == 0 || errno == EPERM
    }

    private static func exitCode(from waitStatus: Int32) -> Int32 {
        let signal = waitStatus & 0x7f
        return signal == 0 ? (waitStatus >> 8) & 0xff : 128 + signal
    }

    private static func failure(_ message: String) -> NSError {
        NSError(
            domain: "KoettMedia",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}

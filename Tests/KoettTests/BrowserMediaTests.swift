@testable import Koett
import Darwin
import XCTest

final class BrowserMediaTests: XCTestCase {
    func testAcceptsWebMediaURLsAndNamesKnownPlatforms() throws {
        let examples = [
            ("https://www.youtube.com/watch?v=aZ2q7JeDqmA", "YouTube"),
            ("https://www.tiktok.com/@creator/video/123", "TikTok"),
            ("https://www.instagram.com/reel/ABC123/", "Instagram"),
            ("https://x.com/creator/status/123", "X"),
            ("https://vimeo.com/123", "Vimeo"),
            ("https://media.example.com/video/123", "media.example.com"),
        ]

        for (value, expectedPlatform) in examples {
            let url = try XCTUnwrap(URL(string: value))
            let source = try XCTUnwrap(BrowserMediaSource(url: url))
            XCTAssertEqual(source.url, url)
            XCTAssertEqual(source.platformName, expectedPlatform)
        }
    }

    func testPlatformNamesRequireTheRealHostBoundary() throws {
        let examples = [
            ("https://youtube.com.example.org/watch?v=123", "youtube.com.example.org"),
            ("https://notyoutube.com/watch?v=123", "notyoutube.com"),
            ("https://tiktok.com@media.example.org/video/123", "media.example.org"),
            ("https://fakeinstagram.com/reel/123", "fakeinstagram.com"),
            ("https://x.com.example.org/status/123", "x.com.example.org"),
        ]

        for (value, expectedPlatform) in examples {
            let source = try XCTUnwrap(BrowserMediaSource(
                url: try XCTUnwrap(URL(string: value))
            ))
            XCTAssertEqual(source.platformName, expectedPlatform)
        }
    }

    func testRejectsNonWebURLs() throws {
        XCTAssertNil(BrowserMediaSource(
            url: try XCTUnwrap(URL(string: "file:///tmp/video.mp4"))
        ))
        XCTAssertNil(BrowserMediaSource(
            url: try XCTUnwrap(URL(string: "mailto:hello@example.com"))
        ))
    }

    func testBuildsOneItemNoCookieDownloadCommand() throws {
        let url = try XCTUnwrap(URL(string: "https://www.tiktok.com/@creator/video/123"))
        let source = try XCTUnwrap(BrowserMediaSource(url: url))
        let arguments = BrowserMedia.downloadArguments(
            for: source,
            ffmpegDirectory: "/opt/homebrew/bin",
            outputTemplate: "/tmp/audio.%(ext)s"
        )

        XCTAssertTrue(arguments.contains("--ignore-config"))
        XCTAssertTrue(arguments.contains("--no-cookies"))
        XCTAssertTrue(arguments.contains("--no-cookies-from-browser"))
        XCTAssertTrue(arguments.contains("--no-playlist"))
        XCTAssertTrue(Self.hasPair("--playlist-end", "1", in: arguments))
        XCTAssertTrue(Self.hasPair(
            "--match-filters",
            "!is_live & duration<=?10800",
            in: arguments
        ))
        XCTAssertTrue(Self.hasPair("--max-filesize", "2G", in: arguments))
        XCTAssertEqual(arguments.last, url.absoluteString)
    }

    func testStopsACommandAfterItsTimeoutAndRemovesProcessOutput() async throws {
        let temporaryDirectory = FileManager.default.temporaryDirectory
        let before = try Self.processOutputFiles(in: temporaryDirectory)
        let processIDsURL = temporaryDirectory
            .appendingPathComponent("koett-process-ids-\(UUID().uuidString).txt")
        defer { try? FileManager.default.removeItem(at: processIDsURL) }
        let startedAt = Date()

        do {
            _ = try await BrowserMedia.run(
                executableURL: URL(fileURLWithPath: "/bin/sh"),
                arguments: [
                    "-c",
                    "sleep 5 & echo \"$$ $!\" > \"$1\"; wait",
                    "koett-process-test",
                    processIDsURL.path,
                ],
                timeout: 0.2,
                timeoutMessage: "Test timeout."
            )
            XCTFail("The process should time out.")
        } catch {
            XCTAssertEqual(error.localizedDescription, "Test timeout.")
        }

        XCTAssertLessThan(Date().timeIntervalSince(startedAt), 2)
        XCTAssertEqual(
            try Self.processOutputFiles(in: temporaryDirectory),
            before
        )
        let processIDs = try String(contentsOf: processIDsURL, encoding: .utf8)
            .split(separator: " ")
            .compactMap { pid_t($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
        XCTAssertEqual(processIDs.count, 2)
        for processID in processIDs {
            XCTAssertTrue(Self.waitUntilProcessStops(processID))
        }
    }

    func testClassifiesCommonMediaFailures() {
        let failures = [
            ("ERROR: Unsupported URL", "Page not supported"),
            ("Sign in to confirm. Use --cookies-from-browser", "Media needs login"),
            ("Your IP address is blocked", "Site blocked the download"),
            ("unable to obtain file audio codec", "Media has no audio"),
            ("does not pass filter (!is_live)", "Live or long media isn't supported"),
            ("The media fetch timed out.", "Media fetch timed out"),
        ]

        for (message, expected) in failures {
            let error = NSError(
                domain: "KoettMediaTests",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: message]
            )
            XCTAssertEqual(KoettController.shortMediaError(error), expected)
        }
    }

    func testMapsInstalledBrowserBundleIdentifiers() {
        XCTAssertEqual(
            BrowserTabTarget.target(
                bundleIdentifier: "com.apple.Safari",
                localizedName: "Safari"
            ),
            .safari
        )
        XCTAssertEqual(
            BrowserTabTarget.target(
                bundleIdentifier: "com.google.Chrome",
                localizedName: "Google Chrome"
            ),
            .chromium(applicationName: "Google Chrome")
        )
        XCTAssertNil(BrowserTabTarget.target(
            bundleIdentifier: "com.apple.TextEdit",
            localizedName: "TextEdit"
        ))
    }

    private static func hasPair(
        _ flag: String,
        _ value: String,
        in arguments: [String]
    ) -> Bool {
        arguments.indices.dropLast().contains { index in
            arguments[index] == flag && arguments[index + 1] == value
        }
    }

    private static func processOutputFiles(in directory: URL) throws -> Set<String> {
        Set(try FileManager.default.contentsOfDirectory(atPath: directory.path)
            .filter { $0.hasPrefix("koett-process-") && $0.hasSuffix(".log") })
    }

    private static func waitUntilProcessStops(_ processID: pid_t) -> Bool {
        let deadline = Date().addingTimeInterval(1)
        while Date() < deadline {
            if Darwin.kill(processID, 0) == -1, errno == ESRCH {
                return true
            }
            Thread.sleep(forTimeInterval: 0.01)
        }
        return false
    }
}

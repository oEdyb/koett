import Darwin
import Foundation
@testable import Koett
import XCTest

@MainActor
final class LocalControlTests: XCTestCase {
    private func socketURL() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("ktt-\(UUID().uuidString.prefix(8))/control.sock")
    }

    func testUserOnlySocketReturnsJSONAndRejectsSecondOwner() async throws {
        let url = socketURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let diagnostics = PasteDiagnostics()
        let coordinator = PasteCoordinator(snapshot: { PasteSnapshot(access: false) },
            postEvent: { _ in false }, record: diagnostics.record)
        _ = await coordinator.prepare(stage: .dictation)
        let server = try LocalControlServer(url: url) { arguments in
            .init(ok: arguments == ["status"], phase: "ready", model: "test", paste: diagnostics.latest)
        }
        defer { server.stop() }
        let reply = try await Task.detached { try LocalControlSocket.request(["status"], at: url) }.value
        XCTAssertTrue(reply.ok)
        XCTAssertEqual(reply.phase, "ready")
        XCTAssertEqual(reply.model, "test")
        XCTAssertEqual(reply.paste?.reason, .accessibilityDenied)
        XCTAssertNotNil(reply.paste?.at)
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        XCTAssertEqual((attributes[.posixPermissions] as? NSNumber)?.intValue, 0o600)
        XCTAssertThrowsError(try LocalControlServer(url: url) { _ in .failure("test", "test") })
        server.stop()
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
    }

    func testExistingFileIsNeverRemoved() throws {
        let url = socketURL()
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let data = Data("User file".utf8)
        try data.write(to: url)
        XCTAssertThrowsError(try LocalControlServer(url: url) { _ in .failure("test", "test") })
        XCTAssertEqual(try Data(contentsOf: url), data)
    }

    func testOversizedCommandCannotReachTheHandler() async throws {
        let url = socketURL()
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        var calls = 0
        let server = try LocalControlServer(url: url) { _ in
            calls += 1
            return .init(ok: true, phase: "ready", model: "test")
        }
        defer { server.stop() }
        do {
            _ = try await Task.detached { try LocalControlSocket.request([String(repeating: "x", count: 3_000)], at: url) }.value
            XCTFail("Oversized request must fail")
        } catch {}
        XCTAssertEqual(calls, 0)
    }

    func testControlCommandsReturnFixableErrorsWithoutStartingCapture() throws {
        let root = socketURL().deletingLastPathComponent()
        defer { try? FileManager.default.removeItem(at: root) }
        let suite = "koett-control-\(UUID())"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let controller = try KoettController(defaults: defaults, speechEngine: .parakeet,
            transcriptPersistence: TranscriptPersistence(
                history: TranscriptStore(fileURL: root.appendingPathComponent("history.md")),
                recovery: FailedTranscriptStore(directoryURL: root.appendingPathComponent("recovery"))))
        XCTAssertEqual(controller.handleControl(["status"]).phase, "loading")
        XCTAssertEqual(controller.handleControl(["start"]).error?.code, "busy")
        XCTAssertEqual(controller.handleControl(["pause"]).error?.code, "not_recording")
        XCTAssertEqual(controller.handleControl(["wat"]).error?.code, "unknown_command")
        controller.state = .ready
        XCTAssertEqual(controller.handleControl(["copy-last"]).error?.code, "no_transcript")
        XCTAssertEqual(controller.handleControl(["transcribe-url", "file:///tmp/a.wav"]).error?.code, "invalid_url")
        XCTAssertEqual(controller.handleControl(["transcribe-file", "relative.wav"]).error?.code, "invalid_path")
        XCTAssertNil(controller.controlServer, "Test controllers must never open the production endpoint")
    }
}

import Darwin
import Foundation
import MeetingASRProtocol

final class MeetingWorkerClient: Sendable {
    private let process: Process
    private let input: FileHandle
    private let output: FileHandle

    init(executable: URL, models: URL, library: URL, mode: MeetingASRMode, networkDenied: Bool = false) throws {
        let process = Process()
        let commands = Pipe(), replies = Pipe()
        process.standardInput = commands
        process.standardOutput = replies
        process.standardError = FileHandle.standardError
        process.qualityOfService = .background
        let args = [mode.rawValue, models.path, library.path, String(getpid())]
        if networkDenied {
            process.executableURL = URL(fileURLWithPath: "/usr/bin/sandbox-exec")
            process.arguments = ["-p", "(version 1)(allow default)(deny network*)", executable.path] + args
        } else {
            process.executableURL = executable
            process.arguments = args
        }
        try process.run()
        self.process = process
        self.input = commands.fileHandleForWriting
        self.output = replies.fileHandleForReading
        commands.fileHandleForReading.closeFile()
        replies.fileHandleForWriting.closeFile()
        let noPipeSignal: Int32 = 1
        _ = fcntl(input.fileDescriptor, F_SETNOSIGPIPE, noPipeSignal)
    }

    deinit { try? input.close(); try? output.close() }

    func decode(_ request: MeetingASRRequest) throws -> MeetingASRResponse {
        try MeetingASRWire.write(request, to: input)
        guard let reply = try MeetingASRWire.read(MeetingASRResponse.self, from: output) else {
            throw MeetingASRError.inference("Meeting worker exited before returning text. Check the worker and model cache, then recover the meeting.")
        }
        if let issue = reply.issue { throw MeetingASRError.inference(issue) }
        return reply
    }

    func cancel() {
        if process.isRunning { process.terminate() }
    }
}

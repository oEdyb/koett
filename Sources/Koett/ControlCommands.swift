import AppKit

extension KoettController {
    func startLocalControl() {
        controlRequested = true
        guard controlServer == nil else { return }
        do {
            controlServer = try LocalControlServer { [weak self] arguments in
                self?.handleControl(arguments) ?? .failure("not_running", "Start Koett and retry.")
            }
        } catch { fputs("Local control: \(error.localizedDescription)\n", stderr) }
    }

    func handleControl(_ arguments: [String]) -> ControlReply {
        let command = arguments.first ?? "status"
        func result(_ failure: ControlReply.Failure? = nil) -> ControlReply {
            .init(ok: failure == nil, phase: String(describing: state), model: speechEngine.rawValue,
                  paused: recorder?.isPaused, error: failure, paste: pasteDiagnostics.latest, pasteLogError: pasteDiagnostics.writeError, pasteAccessGranted: pasteAccessGranted,
                  drawLocked: sketchOverlay.isLocked)
        }
        func fail(_ code: String, _ message: String) -> ControlReply {
            result(.init(code: code, message: message))
        }
        if command == "status" { return result() }
        if ["stop", "pause", "resume", "undo", "draw-lock"].contains(command) {
            guard state == .recording else { return fail("not_recording", "Start dictation before this command.") }
            switch command {
            case "stop": stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
            case "pause": recordingOverlay.setPaused(true)
            case "resume": recordingOverlay.setPaused(false)
            case "draw-lock":
                guard sketchOverlay.isActive else { return fail("drawing_disabled", "Enable Draw & Pin While Dictating before recording.") }
                toggleDrawLock()
            default:
                guard sketchOverlay.canUndo else { return fail("no_drawing", "Draw or pin before undoing.") }
                undoLastDrawing()
            }
            return result()
        }
        if command == "cancel" {
            guard activeDictationID != nil else { return fail("not_cancellable", "No active dictation can be cancelled.") }
            cancelDictation()
            return result()
        }
        let idleCommands = ["start", "history", "dictionary", "copy-last", "paste-last", "retry-save", "transcribe-file", "transcribe-url"]
        guard idleCommands.contains(command) else {
            return fail("unknown_command", "Use status, start, stop, pause, resume, cancel, undo, draw-lock, history, dictionary, copy-last, paste-last, retry-save, transcribe-file PATH, or transcribe-url URL.")
        }
        guard canStartSession else { return fail("busy", "Wait until phase is ready, then retry.") }
        switch command {
        case "start":
            startRecordingIfReady()
            guard state == .recording else { return fail("capture_failed", "Check Koett microphone and permission status.") }
        case "history": openTranscripts()
        case "dictionary": openDictionary()
        case "copy-last", "paste-last":
            guard let lastTranscript else { return fail("no_transcript", "Complete a dictation first.") }
            performLastTranscriptAction(command == "copy-last" ? .copy(lastTranscript) : .paste(lastTranscript))
        case "retry-save": retrySaveTranscripts()
        case "transcribe-file":
            guard arguments.count == 2, arguments[1].hasPrefix("/") else { return fail("invalid_path", "Pass an absolute local media file path.") }
            let url = URL(fileURLWithPath: arguments[1])
            beginMediaTranscription { try await BrowserMedia.localAudio(at: url) }
        case "transcribe-url":
            guard arguments.count == 2, let url = URL(string: arguments[1]), let source = BrowserMediaSource(url: url) else {
                return fail("invalid_url", "Pass an HTTP or HTTPS media URL.")
            }
            beginMediaTranscription { try await BrowserMedia.fetchAudio(for: source) }
        default: break
        }
        return result()
    }
}

import AVFoundation
import Foundation
import MeetingASRProtocol
import OSLog

struct MeetingSnapshot: Codable, Sendable {
    var phase: MeetingState.Phase
    var id: UUID?
    var confirmation: MeetingStopKey?
    var elapsedFrames: UInt64
    var tracks: [String: MeetingTrackStatus]
    var issue: String?
    var progress: Double?
}

enum MeetingCommand: Sendable {
    case start(UUID), stop(UUID), finish(MeetingStopKey), cancel(MeetingStopKey), resume(MeetingStopKey)
    case recover(UUID), toggle
}

struct MeetingCommandError: LocalizedError {
    let code: String
    let message: String
    var errorDescription: String? { message }

    static let busy = Self(code: "busy", message: "Wait until Koett is ready.")
    static let stale = Self(code: "stale_stop_token", message: "Use the current meeting ID and stop token.")
    static let wrongState = Self(code: "wrong_state", message: "This action is unavailable in the current meeting state.")
}

@MainActor
final class MeetingController {
    let root: URL
    private let canStart: () -> Bool
    private let reserve: (Bool) -> Void
    private let makeSource: (MeetingTrack) throws -> any MeetingAudioSource
    private let manifest: (UUID) -> MeetingManifest
    private(set) var state = MeetingState()
    private var capture: Capture?
    private var timer: Timer?
    private var task: Task<Void, Never>?
    private var pending: [UUID] = []
    private var status: [MeetingTrack: MeetingTrackStatus] = [:]
    private var issue: String?
    private var progress: Double?
    var onSnapshot: ((MeetingSnapshot) -> Void)?
    var process: ((UUID, @escaping @MainActor @Sendable (Double) -> Void) async throws -> Void)?

    init(
        root: URL, canStart: @escaping () -> Bool, reserve: @escaping (Bool) -> Void,
        makeSource: @escaping (MeetingTrack) throws -> any MeetingAudioSource,
        manifest: @escaping (UUID) -> MeetingManifest = { .init(id: $0, startedAt: .now) }
    ) {
        self.root = root
        self.canStart = canStart
        self.reserve = reserve
        self.makeSource = makeSource
        self.manifest = manifest
    }

    var snapshot: MeetingSnapshot {
        .init(phase: state.phase, id: state.id, confirmation: state.confirmation,
              elapsedFrames: state.cutoff ?? capture?.clock.offset() ?? 0,
              tracks: Dictionary(uniqueKeysWithValues: status.map { ($0.key.rawValue, $0.value) }),
              issue: issue, progress: progress)
    }

    func offset(atHostTime hostTime: UInt64) -> UInt64? {
        guard state.phase == .recording || state.phase == .confirmStop else { return nil }
        return capture?.clock.offset(hostTime)
    }

    @discardableResult
    func send(_ command: MeetingCommand) throws -> MeetingSnapshot {
        defer { publish() }
        switch command {
        case let .start(id):
            let directory = MeetingBundle.directory(in: root, id: id)
            if state.id == id { return snapshot }
            if FileManager.default.fileExists(atPath: directory.path) {
                throw MeetingCommandError(code: "already_exists", message: "This meeting ID already has a bundle. Check its status.")
            }
            try start(id)
        case let .stop(id):
            guard state.transition(.stop(id)) != .wrongState else { throw MeetingCommandError.wrongState }
        case let .resume(key):
            guard state.transition(.resume(key)) == .changed else { throw MeetingCommandError.stale }
            issue = nil
        case let .finish(key):
            try close(key, goal: .finish)
        case let .cancel(key):
            try close(key, goal: .hold)
        case let .recover(id):
            guard process != nil, state.phase == .idle else { throw MeetingCommandError.busy }
            _ = try MeetingBundle.readManifest(at: MeetingBundle.directory(in: root, id: id), id: id)
            _ = state.transition(.recover(id))
            launch(id)
        case .toggle:
            switch state.phase {
            case .idle: try start(UUID())
            case .recording:
                if let id = state.id { _ = state.transition(.stop(id)) }
            case .confirmStop: break
            default: throw MeetingCommandError.busy
            }
        }
        return snapshot
    }

    static func shortcutEnabled(in defaults: UserDefaults) -> Bool {
        defaults.bool(forKey: "meetingShortcutEnabled")
    }

    func sendShortcut() {
        do { _ = try send(.toggle) }
        catch {
            issue = error.localizedDescription
            Logger(subsystem: "com.olledyberg.Koett", category: "meeting").error("Meeting shortcut: \(error.localizedDescription, privacy: .public)")
            publish()
        }
    }

    func enqueuePending(_ ids: [UUID]) {
        for id in ids where id != state.id && !pending.contains(id) { pending.append(id) }
        launchPending()
        publish()
    }

    func shutdown() {
        pending.removeAll()
        interrupt("Koett quit before finishing the meeting.")
        task?.cancel()
    }

    func poll() {
        guard let capture, state.phase == .recording || state.phase == .confirmStop else { return }
        let reading = capture.writer.reading
        if let failure = reading.failure {
            interrupt("Meeting audio could not be saved. \(failure)")
            return
        }
        let now = capture.clock.offset()
        for track in MeetingTrack.allCases {
            let source = capture.sources[track]
            let hasData = source?.ring.received.load(ordering: .acquiring) == true
            if hasData { capture.received.insert(track) }
            if now >= 48_000 && !capture.received.contains(track) {
                let message = track == .other
                    ? "No system audio arrived. Check System Audio Recording access and retry."
                    : "No microphone audio arrived. Check Microphone access and retry."
                interrupt(message)
                return
            }
            var sourceUnavailable = false
            if let source {
                let opened = capture.openedAt[track, default: 0]
                let arrival = source.ring.lastCallback.load(ordering: .acquiring)
                let last = arrival == 0 ? opened : capture.clock.offset(arrival)
                sourceUnavailable = now > last && now - last >= 48_000
                if now >= capture.nextRouteCheck[track, default: 0] {
                    capture.nextRouteCheck[track] = now + 4_000
                    sourceUnavailable = sourceUnavailable || !source.isConnected
                }
            }
            if sourceUnavailable {
                source?.stop()
                capture.sources[track] = nil
                capture.writer.setSource(nil, for: track)
                capture.nextRetry[track] = now + 8_000
            }
            if capture.sources[track] == nil, now >= (capture.nextRetry[track] ?? 0) {
                if let source = try? makeSource(track) {
                    capture.sources[track] = source
                    capture.writer.setSource(source.ring, for: track)
                }
                capture.openedAt[track] = now
                capture.nextRouteCheck[track] = now + 4_000
                capture.nextRetry[track] = now + 16_000
            }
            let receiving = capture.sources[track]?.ring.received.load(ordering: .acquiring) == true
            let silence = track == .other && receiving && reading.silentSystemAudio
            status[track] = .init(health: receiving ? .receiving : capture.received.contains(track) ? .reconnecting : .waiting,
                                  level: receiving ? reading.levels[track, default: 0] : 0,
                                  issue: silence ? "System audio is silent. If sound is playing, check System Audio Recording access." : nil)
        }
        publish()
    }

    private func start(_ id: UUID) throws {
        guard state.phase == .idle, canStart() else { throw MeetingCommandError.busy }
        _ = state.transition(.start(id))
        issue = nil
        progress = nil
        status = Dictionary(uniqueKeysWithValues: MeetingTrack.allCases.map { ($0, .init()) })
        reserve(true)
        publish()
        do {
            let bundle = try MeetingBundle.create(in: root, manifest: manifest(id))
            let opened = try Capture(bundle)
            capture = opened
            opened.activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .idleSystemSleepDisabled],
                                                                    reason: "Recording a meeting")
            for track in MeetingTrack.allCases {
                let source = try makeSource(track)
                opened.sources[track] = source
                opened.writer.setSource(source.ring, for: track)
            }
            _ = state.transition(.opened(state.attempt))
            let timer = Timer(timeInterval: 0.05, repeats: true) { [weak self] timer in
                guard let self else { timer.invalidate(); return }
                MainActor.assumeIsolated { self.poll() }
            }
            self.timer = timer
            RunLoop.main.add(timer, forMode: .common)
        } catch {
            if capture != nil {
                interrupt(error.localizedDescription)
            } else {
                _ = state.transition(.startFailed(state.attempt))
                issue = error.localizedDescription
                reserve(false)
                launchPending()
            }
            throw error
        }
    }

    private func close(_ key: MeetingStopKey, goal: MeetingCloseGoal) throws {
        guard key == state.confirmation, let capture else { throw MeetingCommandError.stale }
        let intent = MeetingIntent(cutoff: capture.clock.offset(), goal: goal)
        do { try capture.bundle.write(intent, "intent.json") }
        catch MeetingBundleError.beforeCommit(let error) {
            issue = error.localizedDescription
            throw MeetingCommandError(code: "intent_write_failed", message: "Could not save the stop request. Recording is still active.")
        } catch {
            interrupt("Stop request durability is uncertain. \(error.localizedDescription)")
            throw error
        }

        issue = nil
        _ = state.transition(.closing(key, goal, intent.cutoff))
        drain(capture, intent: intent)
    }

    private func interrupt(_ message: String) {
        guard state.reservesForeground, state.phase != .saving, let capture else { return }
        let intent = MeetingIntent(cutoff: capture.clock.offset(), goal: .interrupted, issue: message)
        _ = state.transition(.interrupted(state.attempt, intent.cutoff))
        issue = message
        try? capture.bundle.write(intent, "intent.json")
        drain(capture, intent: intent)
    }

    private func drain(_ capture: Capture, intent: MeetingIntent) {
        publish()
        timer?.invalidate()
        timer = nil
        capture.sources.values.forEach { $0.stop() }
        capture.sources.removeAll()
        if let activity = capture.activity {
            ProcessInfo.processInfo.endActivity(activity)
            capture.activity = nil
        }
        let id = capture.bundle.manifest.id
        let writer = capture.writer
        Task { [weak self] in
            var saved = false
            do {
                try await writer.finish(intent)
                saved = true
            } catch {
                self?.issue = "Could not finish saving. Use meeting recovery. \(error.localizedDescription)"
            }
            guard let self else { return }
            self.capture = nil
            status.removeAll()
            let enqueue = saved && intent.goal == .finish && process != nil
            _ = state.transition(.sealed(state.attempt, process: enqueue))
            reserve(false)
            if enqueue { launch(id) } else { launchPending() }
            publish()
        }
    }

    private func launchPending() {
        guard process != nil, state.phase == .idle, !pending.isEmpty else { return }
        let id = pending.removeFirst()
        _ = state.transition(.recover(id))
        launch(id)
    }

    private func launch(_ id: UUID) {
        pending.removeAll { $0 == id }
        let attempt = state.attempt
        issue = nil
        progress = 0
        task = Task { [weak self] in
            guard let self else { return }
            do {
                try await process?(id) { [weak self] value in
                    guard let self, state.attempt == attempt, state.phase == .processing else { return }
                    progress = min(1, max(0, value))
                    publish()
                }
            } catch { issue = error.localizedDescription }
            if state.transition(.processed(attempt)) == .changed {
                progress = nil
                task = nil
                launchPending()
                publish()
            }
        }
    }

    private func publish() { onSnapshot?(snapshot) }

    private final class Capture {
        let bundle: MeetingBundle
        let clock = MeetingAudioClock()
        let writer: MeetingAudioWriter
        var activity: (any NSObjectProtocol)?
        var sources: [MeetingTrack: any MeetingAudioSource] = [:]
        var received: Set<MeetingTrack> = []
        var nextRetry: [MeetingTrack: UInt64] = [:]
        var nextRouteCheck: [MeetingTrack: UInt64] = [:]
        var openedAt: [MeetingTrack: UInt64] = [:]

        init(_ bundle: MeetingBundle) throws {
            self.bundle = bundle
            writer = try MeetingAudioWriter(bundle: bundle, clock: clock)
        }
    }
}

extension KoettController {
    func makeMeetingController() -> MeetingController {
        let root = transcriptStore.databaseURL.deletingLastPathComponent().appending(path: "Meetings")
        let controller = MeetingController(root: root, canStart: { [weak self] in self?.canStartSession == true },
                                 reserve: { [weak self] reserved in
            guard let self else { return }
            if reserved {
                microphonePreview?.suspend()
                if AssistantAvailability.isEnabled { assistantSpeech.stop() }
            } else {
                microphonePreview?.refresh()
            }
            appUpdater?.stateDidChange()
        }, makeSource: { [weak self] track in
            switch track {
            case .you:
                guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
                    throw MicrophoneDevices.failure("Allow Microphone access in System Settings before recording a meeting.")
                }
                return try MeetingMicrophone { [weak self] in
                    guard let self else { throw MeetingCommandError.wrongState }
                    return try microphoneSelection.resolve(in: availableMicrophones(), defaultID: (try? defaultMicrophoneID()) ?? 0)
                }
            case .other: return try MeetingSystemAudio()
            }
        }, manifest: { [weak self] id in
            let path = self?.microphoneDefaults.string(forKey: "meetingExportDirectory")
            let export = path.flatMap { $0.hasPrefix("/") ? URL(fileURLWithPath: $0, isDirectory: true) : nil }
            return .init(id: id, startedAt: .now, exportDirectory: export,
                         keepAudio: self?.microphoneDefaults.bool(forKey: "meetingKeepAudio") ?? false)
        })
        let history = transcriptStore
        controller.process = { [weak self] id, progress in
            let mode: MeetingASRMode = self?.microphoneDefaults.bool(forKey: "meetingUseRoutedMetal") == true ? .routed : .parakeet
            try await MeetingTranscription(root: root, history: history, mode: mode).run(id, progress: progress)
        }
        return controller
    }

    func replayMeetingPublications() {
        let transcription = MeetingTranscription(root: meeting.root, history: transcriptStore)
        Task(priority: .background) {
            let pending = await transcription.replayAtLaunch()
            guard !Task.isCancelled else { return }
            meeting.enqueuePending(pending)
        }
    }
}

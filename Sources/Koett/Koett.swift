import AppKit
import ApplicationServices
import AVFoundation
import CoreGraphics
import Darwin
import FluidAudio
import Foundation
import ServiceManagement

private enum SpeechEngine: String {
    case parakeet
    case nemotron

    var displayName: String {
        switch self {
        case .parakeet: "Parakeet v2"
        case .nemotron: "Nemotron 560 ms"
        }
    }
}

@MainActor
private final class KoettController: NSObject {
    private enum StartupErrorCode: Int {
        case microphone = 2
        case accessibility = 3
    }

    private enum State {
        case loading
        case ready
        case recording
        case transcribing
    }

    private enum RecordingMode: String {
        case hold
        case toggle
    }

    private enum Shortcut: String {
        case eitherOption
        case rightOption
        case rightCommand

        var keyCodes: [UInt16] {
            switch self {
            case .eitherOption: [58, 61]
            case .rightOption: [61]
            case .rightCommand: [54]
            }
        }

        var modifier: NSEvent.ModifierFlags {
            switch self {
            case .eitherOption, .rightOption: .option
            case .rightCommand: .command
            }
        }
    }

    private let speechEngine: SpeechEngine
    private let transcriptStore = TranscriptStore()
    private let recordingOverlay = RecordingOverlayController()
    private let manager = AsrManager(config: .default)
    private let nemotronAdapter: NemotronStreamingAdapter?
    private let startSound: AVAudioPlayer
    private let stopSound: AVAudioPlayer
    private var recordingMode: RecordingMode
    private var shortcut: Shortcut
    private var state = State.loading
    private var recorder: AVAudioRecorder?
    private var recordingURL: URL?
    private var nemotronRecorder: NemotronLiveRecorder?
    private var monitor: Any?
    private var statusItem: NSStatusItem?
    private var isPreparing = false
    private var isRelaunching = false
    private var startupErrorMessage: String?
    private var startupErrorCode: StartupErrorCode?

    init(defaults: UserDefaults, speechEngine: SpeechEngine) throws {
        self.speechEngine = speechEngine
        nemotronAdapter = speechEngine == .nemotron
            ? NemotronStreamingAdapter()
            : nil
        recordingMode = RecordingMode(
            rawValue: defaults.string(forKey: "recordingMode") ?? ""
        ) ?? .toggle
        shortcut = Shortcut(
            rawValue: defaults.string(forKey: "shortcut") ?? ""
        ) ?? .eitherOption
        startSound = try Self.soundPlayer(named: "Tink")
        stopSound = try Self.soundPlayer(named: "Basso")
        super.init()
    }

    func prepare() async throws {
        guard await Self.microphonePermission() else {
            throw Self.failure(
                "Microphone access is not allowed. Enable it in System Settings > Privacy & Security > Microphone.",
                code: StartupErrorCode.microphone.rawValue
            )
        }

        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        guard AXIsProcessTrustedWithOptions(options) else {
            throw Self.failure(
                "Allow Accessibility access in System Settings > Privacy & Security > Accessibility, then run this command again.",
                code: StartupErrorCode.accessibility.rawValue
            )
        }

        print("Loading Parakeet v2...")
        let models = try await AsrModels.downloadAndLoad(version: .v2)
        try await manager.loadModels(models)
        print("Warming Parakeet v2...")
        let decoderLayers = await manager.decoderLayerCount
        var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
        let silence = [Float](repeating: 0, count: 4_800)
        _ = try await manager.transcribe(silence, decoderState: &decoderState)

        if let nemotronAdapter {
            print("Loading Nemotron 560 ms...")
            try await nemotronAdapter.prepare()
            nemotronRecorder = try NemotronLiveRecorder(adapter: nemotronAdapter)
        }
    }

    func start() async {
        guard state == .loading, !isPreparing else { return }
        isPreparing = true
        startupErrorMessage = nil
        startupErrorCode = nil
        rebuildMenu()

        do {
            try await prepare()
            try installHotkey()
            state = .ready
            let engineName = speechEngine == .nemotron
                ? "Nemotron test"
                : "Parakeet"
            print("Ready with \(engineName). Use the menu-bar icon to change the mode or shortcut.")
        } catch {
            startupErrorMessage = error.localizedDescription
            let nsError = error as NSError
            startupErrorCode = nsError.domain == "Koett"
                ? StartupErrorCode(rawValue: nsError.code)
                : nil
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }

        isPreparing = false
        rebuildMenu()
    }

    func installHotkey() throws {
        guard monitor == nil else { return }
        monitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            let keyCode = event.keyCode
            let modifierFlags = event.modifierFlags.rawValue
            let eventTimestamp = event.timestamp
            Task { @MainActor [weak self] in
                self?.handleShortcut(
                    keyCode: keyCode,
                    modifierFlags: modifierFlags,
                    eventTimestamp: eventTimestamp
                )
            }
        }

        guard monitor != nil else {
            throw Self.failure("macOS could not install the global Option-key monitor.")
        }
    }

    func installMenu() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(
            systemSymbolName: "waveform",
            accessibilityDescription: "Koett"
        )
        item.button?.toolTip = "Koett"
        statusItem = item
        rebuildMenu()
    }

    private func handleShortcut(
        keyCode: UInt16,
        modifierFlags: UInt,
        eventTimestamp: TimeInterval
    ) {
        guard shortcut.keyCodes.contains(keyCode) else { return }
        let flags = NSEvent.ModifierFlags(rawValue: modifierFlags)
            .intersection(.deviceIndependentFlagsMask)
        let isDown = flags.contains(shortcut.modifier)

        switch recordingMode {
        case .hold:
            handleHold(isDown: isDown, eventTimestamp: eventTimestamp)
        case .toggle:
            if isDown {
                toggleRecording(eventTimestamp: eventTimestamp)
            }
        }
    }

    private func handleHold(isDown: Bool, eventTimestamp: TimeInterval) {
        if isDown {
            startRecordingIfReady()
        } else {
            guard state == .recording else { return }
            stopRecording(releaseEventTimestamp: eventTimestamp)
        }
    }

    private func toggleRecording(eventTimestamp: TimeInterval) {
        switch state {
        case .ready:
            startRecordingIfReady()
        case .recording:
            stopRecording(releaseEventTimestamp: eventTimestamp)
        case .loading, .transcribing:
            return
        }
    }

    private func startRecordingIfReady() {
        guard state == .ready else { return }
        do {
            try startRecording()
        } catch {
            state = .ready
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func startRecording() throws {
        switch speechEngine {
        case .parakeet:
            try startParakeetRecording()
        case .nemotron:
            try startNemotronRecording()
        }

        state = .recording
        if let recorder {
            recordingOverlay.start(recorder: recorder)
        }
        play(startSound)
        print("RECORDING")
    }

    private func startParakeetRecording() throws {
        let (newRecorder, url) = try startFileRecording()
        recorder = newRecorder
        recordingURL = url
    }

    private func startNemotronRecording() throws {
        guard let nemotronRecorder else {
            throw Self.failure("Nemotron is not ready.")
        }

        let (recoveryRecorder, recoveryURL) = try startFileRecording()
        do {
            try nemotronRecorder.start()
        } catch {
            recoveryRecorder.stop()
            cleanup(recoveryURL)
            throw error
        }

        recorder = recoveryRecorder
        recordingURL = recoveryURL
    }

    private func startFileRecording() throws -> (AVAudioRecorder, URL) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("koett-\(UUID().uuidString).wav")
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000.0,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 32,
            AVLinearPCMIsFloatKey: true,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ]

        let newRecorder = try AVAudioRecorder(url: url, settings: settings)
        newRecorder.isMeteringEnabled = true
        guard newRecorder.prepareToRecord(), newRecorder.record() else {
            throw Self.failure("The microphone recorder could not start.")
        }

        return (newRecorder, url)
    }

    private func stopRecording(releaseEventTimestamp: TimeInterval) {
        recordingOverlay.stop()
        switch speechEngine {
        case .parakeet:
            stopParakeetRecording(releaseEventTimestamp: releaseEventTimestamp)
        case .nemotron:
            stopNemotronRecording(releaseEventTimestamp: releaseEventTimestamp)
        }
    }

    private func stopParakeetRecording(releaseEventTimestamp: TimeInterval) {
        guard let recorder, let url = recordingURL else { return }
        let duration = recorder.currentTime
        recorder.stop()
        self.recorder = nil
        recordingURL = nil
        state = .transcribing
        play(stopSound)
        print("TRANSCRIBING")

        guard duration >= 0.25 else {
            cleanup(url)
            state = .ready
            print("Ignored: recording was too short.")
            return
        }

        Task { @MainActor [weak self] in
            await self?.transcribe(url, releaseEventTimestamp: releaseEventTimestamp)
        }
    }

    private func stopNemotronRecording(releaseEventTimestamp: TimeInterval) {
        guard let nemotronRecorder,
              let recoveryRecorder = recorder,
              let recoveryURL = recordingURL else { return }

        let duration = recoveryRecorder.currentTime
        recoveryRecorder.stop()
        recorder = nil
        recordingURL = nil

        do {
            let pendingCapture = try nemotronRecorder.stop()
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")

            Task { @MainActor [weak self] in
                await self?.finishNemotron(
                    pendingCapture,
                    recoveryURL: recoveryURL,
                    duration: duration,
                    releaseEventTimestamp: releaseEventTimestamp
                )
            }
        } catch {
            state = .transcribing
            play(stopSound)
            print("TRANSCRIBING")
            fputs(
                "Warning: Nemotron stop failed; using Parakeet recovery: \(error.localizedDescription)\n",
                stderr
            )
            Task { @MainActor [weak self] in
                guard let self else { return }
                defer {
                    self.cleanup(recoveryURL)
                    self.state = .ready
                }
                await self.transcribeRecoveryFile(
                    recoveryURL,
                    releaseEventTimestamp: releaseEventTimestamp
                )
            }
        }
    }

    private func transcribe(_ url: URL, releaseEventTimestamp: TimeInterval) async {
        defer {
            cleanup(url)
            state = .ready
        }

        do {
            let transcriptionStartedAt = ProcessInfo.processInfo.systemUptime
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let transcriptReadyAt = ProcessInfo.processInfo.systemUptime
            let text = result.text.trimmingCharacters(in: .whitespacesAndNewlines)

            guard !text.isEmpty else {
                print("No speech detected. Clipboard unchanged.")
                return
            }

            let pastePostedAt = try deliver(text, model: speechEngine.displayName)
            print("PASTE POSTED: \(text)")
            print(String(
                format: "LATENCY release-to-ASR-start %.1fms | ASR %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (transcriptionStartedAt - releaseEventTimestamp) * 1_000,
                (transcriptReadyAt - transcriptionStartedAt) * 1_000,
                (pastePostedAt - transcriptReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func finishNemotron(
        _ pendingCapture: NemotronLiveRecorder.PendingCapture,
        recoveryURL: URL,
        duration: TimeInterval,
        releaseEventTimestamp: TimeInterval
    ) async {
        defer {
            cleanup(recoveryURL)
            state = .ready
        }
        guard let nemotronAdapter else { return }

        if duration < 0.3 {
            do {
                try await pendingCapture.processingTask.value
            } catch {
                fputs("Warning: Nemotron stopped with: \(error.localizedDescription)\n", stderr)
            }
            await nemotronAdapter.cancel()
            print("Ignored: recording was too short.")
            return
        }

        let finalizationStartedAt = ProcessInfo.processInfo.systemUptime
        let text: String
        do {
            try await pendingCapture.processingTask.value
            guard pendingCapture.store.droppedFrames == 0 else {
                throw Self.failure(
                    "Nemotron dropped \(pendingCapture.store.droppedFrames) microphone frames."
                )
            }

            text = try await nemotronAdapter.finish()
                .trimmingCharacters(in: .whitespacesAndNewlines)
        } catch {
            fputs(
                "Warning: Nemotron failed; using Parakeet recovery: \(error.localizedDescription)\n",
                stderr
            )
            await nemotronAdapter.cancel()
            await transcribeRecoveryFile(
                recoveryURL,
                releaseEventTimestamp: releaseEventTimestamp
            )
            return
        }

        let transcriptReadyAt = ProcessInfo.processInfo.systemUptime
        guard !text.isEmpty else {
            print("No speech detected. Clipboard unchanged.")
            return
        }

        do {
            let pastePostedAt = try deliver(text, model: speechEngine.displayName)
            print("PASTE POSTED: \(text)")
            print(String(
                format: "LATENCY Nemotron release-to-finish-start %.1fms | finish %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (finalizationStartedAt - releaseEventTimestamp) * 1_000,
                (transcriptReadyAt - finalizationStartedAt) * 1_000,
                (pastePostedAt - transcriptReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    private func transcribeRecoveryFile(
        _ url: URL,
        releaseEventTimestamp: TimeInterval
    ) async {
        do {
            let fallbackStartedAt = ProcessInfo.processInfo.systemUptime
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let transcriptReadyAt = ProcessInfo.processInfo.systemUptime
            let text = result.text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else {
                print("No speech detected. Clipboard unchanged.")
                return
            }

            let pastePostedAt = try deliver(
                text,
                model: "Parakeet v2 (Nemotron recovery)"
            )
            print("PASTE POSTED (Parakeet recovery): \(text)")
            print(String(
                format: "LATENCY recovery-start %.1fms | recovery-ASR %.1fms | delivery %.1fms | release-to-paste-post %.1fms",
                (fallbackStartedAt - releaseEventTimestamp) * 1_000,
                (transcriptReadyAt - fallbackStartedAt) * 1_000,
                (pastePostedAt - transcriptReadyAt) * 1_000,
                (pastePostedAt - releaseEventTimestamp) * 1_000
            ))
        } catch {
            fputs("Error: Parakeet recovery failed: \(error.localizedDescription)\n", stderr)
        }
    }

    private func deliver(_ text: String, model: String) throws -> TimeInterval {
        defer {
            do {
                try transcriptStore.append(text, model: model)
            } catch {
                fputs("Warning: transcript was not saved: \(error.localizedDescription)\n", stderr)
            }
        }

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard pasteboard.setString(text, forType: .string) else {
            throw Self.failure("The transcript could not be copied to the clipboard.")
        }

        guard pasteAtCursor() else {
            throw Self.failure("The transcript is on the clipboard, but Command-V could not be sent.")
        }
        let pastePostedAt = ProcessInfo.processInfo.systemUptime
        return pastePostedAt
    }

    private func play(_ player: AVAudioPlayer) {
        player.stop()
        player.currentTime = 0
        _ = player.play()
    }

    private func pasteAtCursor() -> Bool {
        guard let source = CGEventSource(stateID: .hidSystemState),
              let keyDown = CGEvent(
                  keyboardEventSource: source,
                  virtualKey: 9,
                  keyDown: true
              ),
              let keyUp = CGEvent(
                  keyboardEventSource: source,
                  virtualKey: 9,
                  keyDown: false
              ) else {
            return false
        }

        keyDown.flags = .maskCommand
        keyUp.flags = .maskCommand
        keyDown.post(tap: .cghidEventTap)
        keyUp.post(tap: .cghidEventTap)
        return true
    }

    private func cleanup(_ url: URL) {
        do {
            try FileManager.default.removeItem(at: url)
        } catch {
            fputs("Warning: temporary audio remains at \(url.path)\n", stderr)
        }
    }

    private func rebuildMenu() {
        let menu = NSMenu()
        if isPreparing {
            let item = NSMenuItem(title: "Starting Koett…", action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
            menu.addItem(.separator())
        } else if let startupErrorMessage {
            let item = NSMenuItem(title: "Setup Required", action: nil, keyEquivalent: "")
            item.isEnabled = false
            item.toolTip = startupErrorMessage
            menu.addItem(item)
            if startupErrorCode == .accessibility {
                menu.addItem(menuItem(
                    title: "Open Accessibility Settings",
                    action: #selector(openAccessibilitySettings),
                    selected: false
                ))
            }
            menu.addItem(menuItem(
                title: "Retry Setup",
                action: #selector(retrySetup),
                selected: false
            ))
            menu.addItem(.separator())
        }
        let canChangeModel = !isPreparing
            && !isRelaunching
            && state != .recording
            && state != .transcribing
        let parakeetItem = menuItem(
            title: "Model: Parakeet v2",
            action: #selector(useParakeetModel),
            selected: speechEngine == .parakeet
        )
        parakeetItem.isEnabled = canChangeModel
        menu.addItem(parakeetItem)
        let nemotronItem = menuItem(
            title: "Model: Nemotron 560 ms (Test)",
            action: #selector(useNemotronModel),
            selected: speechEngine == .nemotron
        )
        nemotronItem.isEnabled = canChangeModel
        menu.addItem(nemotronItem)
        menu.addItem(.separator())
        menu.addItem(menuItem(
            title: "Mode: Toggle",
            action: #selector(useToggleMode),
            selected: recordingMode == .toggle
        ))
        menu.addItem(menuItem(
            title: "Mode: Hold",
            action: #selector(useHoldMode),
            selected: recordingMode == .hold
        ))
        menu.addItem(.separator())
        menu.addItem(menuItem(
            title: "Key: Either Option",
            action: #selector(useEitherOption),
            selected: shortcut == .eitherOption
        ))
        menu.addItem(menuItem(
            title: "Key: Right Option",
            action: #selector(useRightOption),
            selected: shortcut == .rightOption
        ))
        menu.addItem(menuItem(
            title: "Key: Right Command",
            action: #selector(useRightCommand),
            selected: shortcut == .rightCommand
        ))
        menu.addItem(.separator())
        menu.addItem(menuItem(
            title: "Open Transcripts",
            action: #selector(openTranscripts),
            selected: false
        ))
        menu.addItem(NSMenuItem(
            title: "Quit Koett",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        ))
        statusItem?.menu = menu
    }

    private func menuItem(title: String, action: Selector, selected: Bool) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        item.state = selected ? .on : .off
        return item
    }

    @objc private func useToggleMode() {
        setRecordingMode(.toggle)
    }

    @objc private func useParakeetModel() {
        setSpeechEngine(.parakeet)
    }

    @objc private func useNemotronModel() {
        setSpeechEngine(.nemotron)
    }

    @objc private func useHoldMode() {
        setRecordingMode(.hold)
    }

    @objc private func useEitherOption() {
        setShortcut(.eitherOption)
    }

    @objc private func useRightOption() {
        setShortcut(.rightOption)
    }

    @objc private func useRightCommand() {
        setShortcut(.rightCommand)
    }

    @objc private func openAccessibilitySettings() {
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        guard let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        ), NSWorkspace.shared.open(url) else {
            fputs("Error: Accessibility settings could not open.\n", stderr)
            return
        }
    }

    @objc private func openTranscripts() {
        do {
            try transcriptStore.prepare()
            guard NSWorkspace.shared.open(transcriptStore.fileURL) else {
                throw Self.failure("The transcript file could not open.")
            }
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc private func retrySetup() {
        Task { @MainActor [weak self] in
            await self?.start()
        }
    }

    private func setRecordingMode(_ mode: RecordingMode) {
        recordingMode = mode
        UserDefaults.standard.set(mode.rawValue, forKey: "recordingMode")
        rebuildMenu()
    }

    private func setShortcut(_ newShortcut: Shortcut) {
        shortcut = newShortcut
        UserDefaults.standard.set(newShortcut.rawValue, forKey: "shortcut")
        rebuildMenu()
    }

    private func setSpeechEngine(_ newEngine: SpeechEngine) {
        guard newEngine != speechEngine,
              !isPreparing,
              !isRelaunching,
              state != .recording,
              state != .transcribing else { return }

        isRelaunching = true
        rebuildMenu()
        relaunch(with: newEngine)
    }

    private func relaunch(with engine: SpeechEngine) {
        let argument = engine == .nemotron ? "--nemotron" : "--parakeet"
        let bundleURL = Bundle.main.bundleURL
        if bundleURL.pathExtension == "app" {
            let configuration = NSWorkspace.OpenConfiguration()
            configuration.activates = false
            configuration.arguments = [argument]
            configuration.createsNewApplicationInstance = true
            NSWorkspace.shared.openApplication(
                at: bundleURL,
                configuration: configuration
            ) { [weak self] _, error in
                Task { @MainActor in
                    guard let self else { return }
                    if let error {
                        self.isRelaunching = false
                        self.rebuildMenu()
                        fputs(
                            "Error: Koett could not restart: \(error.localizedDescription)\n",
                            stderr
                        )
                        return
                    }
                    UserDefaults.standard.set(engine.rawValue, forKey: "speechEngine")
                    NSApplication.shared.terminate(nil)
                }
            }
            return
        }

        do {
            let process = Process()
            process.executableURL = URL(
                fileURLWithPath: CommandLine.arguments[0]
            ).standardizedFileURL
            process.arguments = [argument]
            try process.run()
            UserDefaults.standard.set(engine.rawValue, forKey: "speechEngine")
            NSApplication.shared.terminate(nil)
        } catch {
            isRelaunching = false
            rebuildMenu()
            fputs("Error: Koett could not restart: \(error.localizedDescription)\n", stderr)
        }
    }

    private static func microphonePermission() async -> Bool {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return true
        case .notDetermined:
            return await AVCaptureDevice.requestAccess(for: .audio)
        default:
            return false
        }
    }

    private static func soundPlayer(named name: String) throws -> AVAudioPlayer {
        let url = URL(fileURLWithPath: "/System/Library/Sounds/\(name).aiff")
        let player = try AVAudioPlayer(contentsOf: url)
        player.volume = 1
        guard player.prepareToPlay() else {
            throw failure("The \(name) sound could not prepare.")
        }
        return player
    }

    private static func failure(_ message: String, code: Int = 1) -> NSError {
        NSError(domain: "Koett", code: code, userInfo: [NSLocalizedDescriptionKey: message])
    }
}

@main
private struct Koett {
    @MainActor
    static func main() {
        let arguments = CommandLine.arguments.dropFirst()

        if arguments.contains("--register-login") {
            do {
                try registerLoginItem()
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(EXIT_FAILURE)
            }
            return
        }

        if arguments.contains("--unregister-login") {
            do {
                try unregisterLoginItem()
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(EXIT_FAILURE)
            }
            return
        }

        if arguments.contains("--help") {
            print("Usage: koett [--parakeet | --nemotron]")
            print("Command-line model flags override the saved model for this launch.")
            print("Use the menu-bar icon to choose the model, mode, and shortcut.")
            return
        }

        do {
            let savedEngine = SpeechEngine(
                rawValue: UserDefaults.standard.string(forKey: "speechEngine") ?? ""
            ) ?? .parakeet
            let speechEngine: SpeechEngine
            if arguments.contains("--nemotron") {
                speechEngine = .nemotron
            } else if arguments.contains("--parakeet") {
                speechEngine = .parakeet
            } else {
                speechEngine = savedEngine
            }
            let controller = try KoettController(
                defaults: .standard,
                speechEngine: speechEngine
            )
            let delegate = KoettDelegate(controller: controller)
            let application = NSApplication.shared
            application.setActivationPolicy(.accessory)
            application.delegate = delegate
            application.run()
            withExtendedLifetime(delegate) {}
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    private static func registerLoginItem() throws {
        let service = SMAppService.mainApp
        if service.status != .enabled {
            try service.register()
        }

        guard service.status == .enabled else {
            SMAppService.openSystemSettingsLoginItems()
            throw NSError(
                domain: "Koett",
                code: 2,
                userInfo: [
                    NSLocalizedDescriptionKey: "Enable Koett in System Settings > General > Login Items."
                ]
            )
        }

        print("Login Item enabled.")
    }

    private static func unregisterLoginItem() throws {
        let service = SMAppService.mainApp
        if service.status != .notRegistered {
            try service.unregister()
        }
        print("Login Item disabled.")
    }
}

@MainActor
private final class KoettDelegate: NSObject, NSApplicationDelegate {
    private let controller: KoettController

    init(controller: KoettController) {
        self.controller = controller
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        controller.installMenu()
        print("Checking microphone and Accessibility access...")
        Task { @MainActor in
            await controller.start()
        }
    }
}

import AppKit
import ApplicationServices
import AVFoundation
import CoreGraphics
import Darwin
import FluidAudio
import Foundation

@MainActor
final class KoettController: NSObject {
    enum State {
        case loading
        case ready
        case recording
        case transcribing
        case media
        case assistantRecording
        case assistantWorking
    }

    enum RecordingMode: String {
        case hold
        case toggle
    }

    let speechEngine: SpeechEngine
    let transcriptStore = TranscriptStore()
    let mediaTranscriptStore = MediaTranscriptStore()
    let failedRecordingStore = FailedRecordingStore()
    let failedTranscriptStore = FailedTranscriptStore()
    let recordingOverlay = RecordingOverlayController()
    let assistantPanel = AssistantPanelController()
    let assistantClient = AssistantClient()
    let assistantSpeech = CartesiaSpeechOutput()
    let manager: AsrManager
    let nemotronAdapter: NemotronStreamingAdapter?
    let startSound: AVAudioPlayer
    let stopSound: AVAudioPlayer
    var recordingMode: RecordingMode
    var dictationShortcut: ShortcutBinding
    var mediaShortcut: ShortcutBinding
    var assistantShortcut: ShortcutBinding
    var assistantProvider: AssistantProvider
    var assistantVoice: AssistantVoice
    var assistantSpeechEnabled: Bool
    var s1MiniEnabled: Bool
    var s1MiniStyle: S1MiniStyle
    var s1MiniStructure: S1MiniStructure
    var s1MiniContext: S1MiniContext
    var s1MiniCleaner: S1MiniCleaner?
    lazy var formattingPopover = FormattingPopoverController(controller: self)
    private var dictationModifierState = ModifierShortcutState()
    private var mediaModifierState = ModifierShortcutState()
    private var assistantModifierState = ModifierShortcutState()
    var state = State.loading
    var recorder: AVAudioRecorder?
    var recordingURL: URL?
    var screenCaptureTask: Task<Data, Error>?
    var assistantWorkTask: Task<Void, Never>?
    var nemotronRecorder: NemotronLiveRecorder?
    private var modifierEventTap: CFMachPort?
    private var modifierEventSource: CFRunLoopSource?
    private var keyMonitor: Any?
    var statusItem: NSStatusItem?
    var isPreparing = false
    var isRelaunching = false
    private var isCapturingShortcut = false
    var startupErrorMessage: String?
    var startupErrorCode: StartupErrorCode?
    var setupStatus = SetupStatus.checkingMicrophone
    var startupNoticeMessage: String?

    init(defaults: UserDefaults, speechEngine: SpeechEngine) throws {
        self.speechEngine = speechEngine
        manager = AsrManager(config: speechEngine.asrConfiguration)
        nemotronAdapter = speechEngine == .nemotron
            ? NemotronStreamingAdapter()
            : nil
        recordingMode = RecordingMode(
            rawValue: defaults.string(forKey: "recordingMode") ?? ""
        ) ?? .toggle
        dictationShortcut = Self.loadShortcut(
            key: "dictationShortcut",
            defaults: defaults
        ) ?? ShortcutBinding.migratedDictation(
            defaults.string(forKey: "shortcut")
        )
        mediaShortcut = Self.loadShortcut(
            key: "mediaShortcut",
            defaults: defaults
        ) ?? .defaultMedia
        assistantShortcut = Self.loadShortcut(
            key: "assistantShortcut",
            defaults: defaults
        ) ?? .defaultAssistant
        assistantProvider = AssistantProvider(
            rawValue: defaults.string(forKey: "assistantProvider") ?? ""
        ) ?? .groq
        assistantVoice = AssistantVoice(
            rawValue: defaults.string(forKey: "assistantVoice") ?? ""
        ) ?? .katie
        assistantSpeechEnabled = defaults.bool(forKey: "assistantSpeechEnabled")
        let savedS1MiniStyle = defaults.string(forKey: "s1MiniStyle")
            .flatMap(S1MiniStyle.init(rawValue:))
        s1MiniEnabled = defaults.object(forKey: "s1MiniEnabled") as? Bool
            ?? (savedS1MiniStyle != nil)
        s1MiniStyle = savedS1MiniStyle ?? .semiCasual
        s1MiniStructure = defaults.string(forKey: "s1MiniStructure")
            .flatMap(S1MiniStructure.init(rawValue:)) ?? .prose
        s1MiniContext = defaults.string(forKey: "s1MiniContext")
            .flatMap(S1MiniContext.init(rawValue:)) ?? .general
        startSound = try Self.soundPlayer(named: "Tink")
        stopSound = try Self.soundPlayer(named: "Basso")
        super.init()
    }

    func prepare() async throws {
        setSetupStatus(.checkingMicrophone, showOverlay: false)
        guard await Self.microphonePermission() else {
            throw Self.failure(
                "Microphone access is not allowed. Enable it in System Settings > Privacy & Security > Microphone.",
                code: StartupErrorCode.microphone.rawValue
            )
        }

        setSetupStatus(.checkingAccessibility, showOverlay: false)
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        guard AXIsProcessTrustedWithOptions(options) else {
            throw Self.failure(
                "Allow Accessibility access in System Settings > Privacy & Security > Accessibility, then choose Retry Setup from the Koett menu.",
                code: StartupErrorCode.accessibility.rawValue
            )
        }

        let parakeetModelName = speechEngine.parakeetModelName
        setSetupStatus(.checkingModel(parakeetModelName))
        print("Loading \(parakeetModelName)...")
        let models = try await AsrModels.downloadAndLoad(
            version: speechEngine.parakeetVersion,
            encoderPrecision: .int8,
            progressHandler: fluidAudioProgressHandler(model: parakeetModelName)
        )
        setSetupStatus(.loadingModel(parakeetModelName))
        try await manager.loadModels(models)
        setSetupStatus(.warmingModel(parakeetModelName))
        print("Warming \(parakeetModelName)...")
        let decoderLayers = await manager.decoderLayerCount
        var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
        let silence = [Float](repeating: 0, count: 4_800)
        _ = try await manager.transcribe(silence, decoderState: &decoderState)

        if s1MiniEnabled {
            setSetupStatus(.checkingModel("S1-mini"))
            print("Loading S1-mini by Superwhisper...")
            let cleaner = S1MiniCleaner()
            do {
                try await cleaner.prepare { [weak self] fraction in
                    Task { @MainActor [weak self] in
                        self?.setSetupStatus(
                            .download(model: "S1-mini", fraction: fraction)
                        )
                    }
                }
                s1MiniCleaner = cleaner
            } catch {
                s1MiniEnabled = false
                startupNoticeMessage = "S1-mini unavailable · Raw text active"
                fputs(
                    "Warning: S1-mini is unavailable for this launch: \(error.localizedDescription)\n",
                    stderr
                )
            }
        }

        if let nemotronAdapter {
            setSetupStatus(.checkingModel("Nemotron 560 ms"))
            print("Loading Nemotron 560 ms...")
            try await nemotronAdapter.prepare(
                progressHandler: fluidAudioProgressHandler(model: "Nemotron 560 ms")
            )
            nemotronRecorder = try NemotronLiveRecorder(adapter: nemotronAdapter)
        }
    }

    func start() async {
        guard state == .loading, !isPreparing else { return }
        isPreparing = true
        startupErrorMessage = nil
        startupErrorCode = nil
        startupNoticeMessage = nil
        rebuildMenu()

        do {
            try await prepare()
            try installHotkey()
            state = .ready
            print(
                "Ready with \(speechEngine.displayName). "
                    + "Use the menu-bar icon to change the mode or shortcut."
            )
        } catch {
            startupErrorMessage = error.localizedDescription
            let nsError = error as NSError
            startupErrorCode = nsError.domain == "Koett"
                ? StartupErrorCode(rawValue: nsError.code)
                : nil
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }

        isPreparing = false
        if startupErrorMessage == nil {
            setSetupStatus(.ready, showOverlay: startupNoticeMessage == nil)
            if let startupNoticeMessage {
                recordingOverlay.showError(startupNoticeMessage)
            }
        } else {
            let message = SetupStatus.shortError(
                code: startupErrorCode,
                message: startupErrorMessage ?? ""
            )
            recordingOverlay.showError(message)
            rebuildMenu()
        }
    }

    func setSetupStatus(_ status: SetupStatus, showOverlay: Bool = true) {
        guard status != setupStatus else { return }
        setupStatus = status
        statusItem?.button?.toolTip = status.menuTitle
        rebuildMenu()
        guard showOverlay else { return }
        if let fraction = status.progressFraction {
            recordingOverlay.showProgress(status.overlayTitle, fraction: fraction)
        } else if status == .ready {
            recordingOverlay.showTransientStatus(status.overlayTitle)
        } else {
            recordingOverlay.showStatus(status.overlayTitle)
        }
    }

    private func fluidAudioProgressHandler(model: String) -> ProgressHandler {
        { [weak self] progress in
            Task { @MainActor [weak self] in
                guard let self else { return }
                switch progress.phase {
                case .listing:
                    self.setSetupStatus(.checkingModel(model))
                case .downloading(_, let totalFiles):
                    if totalFiles == 0 {
                        self.setSetupStatus(.loadingModel(model))
                    } else {
                        self.setSetupStatus(
                            .fluidAudioDownload(
                                model: model,
                                fraction: progress.fractionCompleted
                            )
                        )
                    }
                case .compiling:
                    self.setSetupStatus(.loadingModel(model))
                }
            }
        }
    }

    func installHotkey() throws {
        guard modifierEventTap == nil, keyMonitor == nil else { return }

        let eventMask = CGEventMask(1) << CGEventType.flagsChanged.rawValue
        guard let eventTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: { _, type, event, userInfo in
                guard let userInfo else {
                    return Unmanaged.passUnretained(event)
                }
                let controller = Unmanaged<KoettController>
                    .fromOpaque(userInfo)
                    .takeUnretainedValue()

                if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
                    DispatchQueue.main.async {
                        controller.recoverModifierEventTap()
                    }
                    return Unmanaged.passUnretained(event)
                }

                guard type == .flagsChanged else {
                    return Unmanaged.passUnretained(event)
                }
                let keyCode = UInt16(
                    event.getIntegerValueField(.keyboardEventKeycode)
                )
                let modifierKeyIsDown = CGEventSource.keyState(
                    .combinedSessionState,
                    key: CGKeyCode(keyCode)
                )
                let modifierFlags = UInt(event.flags.rawValue)
                let eventTimestamp = TimeInterval(event.timestamp) / 1_000_000_000
                DispatchQueue.main.async {
                    controller.handleShortcutEvent(
                        type: .flagsChanged,
                        keyCode: keyCode,
                        modifierFlags: modifierFlags,
                        eventTimestamp: eventTimestamp,
                        isRepeat: false,
                        modifierKeyIsDown: modifierKeyIsDown
                    )
                }
                return Unmanaged.passUnretained(event)
            },
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            throw Self.failure("macOS could not install the modifier shortcut monitor.")
        }
        guard let eventSource = CFMachPortCreateRunLoopSource(
            kCFAllocatorDefault,
            eventTap,
            0
        ) else {
            throw Self.failure("macOS could not start the modifier shortcut monitor.")
        }

        let newKeyMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: [.keyDown, .keyUp]
        ) { [weak self] event in
            let type = event.type
            let keyCode = event.keyCode
            let modifierFlags = event.modifierFlags.rawValue
            let eventTimestamp = event.timestamp
            let isRepeat = event.isARepeat
            DispatchQueue.main.async { [weak self] in
                self?.handleShortcutEvent(
                    type: type,
                    keyCode: keyCode,
                    modifierFlags: modifierFlags,
                    eventTimestamp: eventTimestamp,
                    isRepeat: isRepeat
                )
            }
        }

        guard let newKeyMonitor else {
            throw Self.failure("macOS could not install the key shortcut monitor.")
        }

        CFRunLoopAddSource(CFRunLoopGetMain(), eventSource, .commonModes)
        modifierEventTap = eventTap
        modifierEventSource = eventSource
        keyMonitor = newKeyMonitor
        enableModifierEventTap()
    }

    private func enableModifierEventTap() {
        if let modifierEventTap {
            CGEvent.tapEnable(tap: modifierEventTap, enable: true)
        }
    }

    private func recoverModifierEventTap() {
        enableModifierEventTap()

        let keyState: (UInt16) -> Bool = { keyCode in
            CGEventSource.keyState(
                .combinedSessionState,
                key: CGKeyCode(keyCode)
            )
        }
        let dictationIsHeld = dictationModifierState.resync(
            binding: dictationShortcut,
            isKeyDown: keyState
        )
        mediaModifierState.resync(binding: mediaShortcut, isKeyDown: keyState)
        assistantModifierState.resync(binding: assistantShortcut, isKeyDown: keyState)

        if recordingMode == .hold,
           dictationShortcut.modifierOnly,
           state == .recording,
           !dictationIsHeld {
            stopRecording(releaseEventTimestamp: ProcessInfo.processInfo.systemUptime)
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

    private func handleShortcutEvent(
        type: NSEvent.EventType,
        keyCode: UInt16,
        modifierFlags: UInt,
        eventTimestamp: TimeInterval,
        isRepeat: Bool,
        modifierKeyIsDown: Bool? = nil
    ) {
        guard !isCapturingShortcut else { return }

        if type == .keyDown, !isRepeat {
            dictationModifierState.noteKeyDown()
            mediaModifierState.noteKeyDown()
            assistantModifierState.noteKeyDown()
        }

        let assistantModifierTransition = assistantModifierState.update(
            binding: assistantShortcut,
            type: type,
            keyCode: keyCode,
            modifierFlags: modifierFlags,
            keyIsDown: modifierKeyIsDown
        )
        let mediaModifierTransition = mediaModifierState.update(
            binding: mediaShortcut,
            type: type,
            keyCode: keyCode,
            modifierFlags: modifierFlags,
            keyIsDown: modifierKeyIsDown
        )
        let dictationModifierTransition = dictationModifierState.update(
            binding: dictationShortcut,
            type: type,
            keyCode: keyCode,
            modifierFlags: modifierFlags,
            keyIsDown: modifierKeyIsDown
        )
        let assistantIsDown = assistantShortcut.modifierOnly
            ? assistantModifierTransition.releasedWithoutChord
            : assistantShortcut.matchesDown(
                type: type,
                keyCode: keyCode,
                modifierFlags: modifierFlags,
                isRepeat: isRepeat
            )
        if assistantIsDown {
            toggleAssistant()
            return
        }

        let mediaIsDown = mediaShortcut.modifierOnly
            ? mediaModifierTransition.down
            : mediaShortcut.matchesDown(
                type: type,
                keyCode: keyCode,
                modifierFlags: modifierFlags,
                isRepeat: isRepeat
            )
        if mediaIsDown {
            startMediaTranscription()
            return
        }

        let isDown = dictationShortcut.modifierOnly
            ? dictationModifierTransition.down
            : dictationShortcut.matchesDown(
                type: type,
                keyCode: keyCode,
                modifierFlags: modifierFlags,
                isRepeat: isRepeat
            )
        let isUp = dictationShortcut.modifierOnly
            ? dictationModifierTransition.up
            : dictationShortcut.matchesUp(
                type: type,
                keyCode: keyCode,
                modifierFlags: modifierFlags
            )

        switch recordingMode {
        case .hold:
            if isDown {
                startRecordingIfReady()
            } else if isUp, state == .recording {
                stopRecording(releaseEventTimestamp: eventTimestamp)
            }
        case .toggle:
            let shouldToggle = dictationShortcut.modifierOnly
                ? dictationModifierTransition.releasedWithoutChord
                : isDown
            if shouldToggle {
                toggleRecording(eventTimestamp: eventTimestamp)
            }
        }
    }

    @objc func useToggleMode() {
        setRecordingMode(.toggle)
    }

    @objc func useParakeetModel() {
        setSpeechEngine(.parakeet)
    }

    @objc func useParakeetV3Model() {
        setSpeechEngine(.parakeetV3)
    }

    @objc func useNemotronModel() {
        setSpeechEngine(.nemotron)
    }

    @objc func useHoldMode() {
        setRecordingMode(.hold)
    }

    @objc func openAccessibilitySettings() {
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        guard let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        ), NSWorkspace.shared.open(url) else {
            fputs("Error: Accessibility settings could not open.\n", stderr)
            return
        }
    }

    @objc func openMicrophoneSettings() {
        guard let url = URL(
            string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        ), NSWorkspace.shared.open(url) else {
            fputs("Error: Microphone settings could not open.\n", stderr)
            return
        }
    }

    @objc func showSetupError() {
        guard let startupErrorMessage else { return }
        let alert = NSAlert()
        alert.messageText = "Koett setup failed"
        alert.informativeText = startupErrorMessage
        alert.addButton(withTitle: "OK")
        _ = NSRunningApplication.current.activate(options: [.activateAllWindows])
        NSApp.activate(ignoringOtherApps: true)
        _ = alert.runModal()
    }

    @objc func openTranscripts() {
        do {
            try transcriptStore.prepare()
            guard NSWorkspace.shared.open(transcriptStore.fileURL) else {
                throw Self.failure("The transcript file could not open.")
            }
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
    }

    @objc func bindDictationShortcut() {
        guard state == .ready else { return }
        DispatchQueue.main.async { [weak self] in
            self?.captureDictationShortcut()
        }
    }

    private func captureDictationShortcut() {
        guard state == .ready else { return }
        isCapturingShortcut = true
        defer {
            DispatchQueue.main.async { [weak self] in
                self?.isCapturingShortcut = false
            }
        }
        guard let binding = ShortcutCapture.run(title: "Set Dictation Shortcut") else {
            return
        }
        guard !ShortcutBinding.shortcutsConflict(
            dictation: binding,
            media: mediaShortcut,
            dictationUsesToggle: recordingMode == .toggle
        ) else {
            showShortcutConflict(
                with: "Media Shortcut",
                binding: mediaShortcut
            )
            return
        }
        guard !ShortcutBinding.shortcutsConflict(
            dictation: binding,
            media: assistantShortcut,
            dictationUsesToggle: recordingMode == .toggle
        ) else {
            showShortcutConflict(
                with: "Ask Shortcut",
                binding: assistantShortcut
            )
            return
        }
        dictationShortcut = binding
        dictationModifierState.reset()
        saveShortcut(binding, key: "dictationShortcut")
        rebuildMenu()
    }

    @objc func bindMediaShortcut() {
        guard state == .ready else { return }
        DispatchQueue.main.async { [weak self] in
            self?.captureMediaShortcut()
        }
    }

    @objc func bindAssistantShortcut() {
        guard state == .ready else { return }
        DispatchQueue.main.async { [weak self] in
            self?.captureAssistantShortcut()
        }
    }

    private func captureMediaShortcut() {
        guard state == .ready else { return }
        isCapturingShortcut = true
        defer {
            DispatchQueue.main.async { [weak self] in
                self?.isCapturingShortcut = false
            }
        }
        guard let binding = ShortcutCapture.run(title: "Set Media Shortcut") else {
            return
        }
        guard !ShortcutBinding.shortcutsConflict(
            dictation: dictationShortcut,
            media: binding,
            dictationUsesToggle: recordingMode == .toggle
        ) else {
            showShortcutConflict(
                with: "Dictation Shortcut",
                binding: dictationShortcut
            )
            return
        }
        guard !binding.conflicts(with: assistantShortcut) else {
            showShortcutConflict(
                with: "Ask Shortcut",
                binding: assistantShortcut
            )
            return
        }
        mediaShortcut = binding
        mediaModifierState.reset()
        saveShortcut(binding, key: "mediaShortcut")
        rebuildMenu()
    }

    private func captureAssistantShortcut() {
        guard state == .ready else { return }
        isCapturingShortcut = true
        defer {
            DispatchQueue.main.async { [weak self] in
                self?.isCapturingShortcut = false
            }
        }
        guard let binding = ShortcutCapture.run(title: "Set Ask Shortcut") else {
            return
        }
        guard !ShortcutBinding.shortcutsConflict(
            dictation: dictationShortcut,
            media: binding,
            dictationUsesToggle: recordingMode == .toggle
        ) else {
            showShortcutConflict(
                with: "Dictation Shortcut",
                binding: dictationShortcut
            )
            return
        }
        guard !binding.conflicts(with: mediaShortcut) else {
            showShortcutConflict(
                with: "Media Shortcut",
                binding: mediaShortcut
            )
            return
        }
        assistantShortcut = binding
        assistantModifierState.reset()
        saveShortcut(binding, key: "assistantShortcut")
        rebuildMenu()
    }

    @objc func resetShortcuts() {
        guard state == .ready else { return }
        dictationShortcut = .defaultDictation
        mediaShortcut = .defaultMedia
        assistantShortcut = .defaultAssistant
        dictationModifierState.reset()
        mediaModifierState.reset()
        assistantModifierState.reset()
        saveShortcut(dictationShortcut, key: "dictationShortcut")
        saveShortcut(mediaShortcut, key: "mediaShortcut")
        saveShortcut(assistantShortcut, key: "assistantShortcut")
        rebuildMenu()
    }

    @objc func retrySetup() {
        Task { @MainActor [weak self] in
            await self?.start()
        }
    }

    private func setRecordingMode(_ mode: RecordingMode) {
        guard state == .ready else { return }
        if mode == .hold {
            if ShortcutBinding.shortcutsConflict(
                dictation: dictationShortcut,
                media: mediaShortcut,
                dictationUsesToggle: false
            ) {
                showShortcutConflict(
                    with: "Media Shortcut",
                    binding: mediaShortcut
                )
                return
            }
            if ShortcutBinding.shortcutsConflict(
                dictation: dictationShortcut,
                media: assistantShortcut,
                dictationUsesToggle: false
            ) {
                showShortcutConflict(
                    with: "Ask Shortcut",
                    binding: assistantShortcut
                )
                return
            }
        }
        recordingMode = mode
        UserDefaults.standard.set(mode.rawValue, forKey: "recordingMode")
        rebuildMenu()
    }

    private func setSpeechEngine(_ newEngine: SpeechEngine) {
        guard newEngine != speechEngine,
              !isPreparing,
              !isRelaunching,
              state != .recording,
              state != .transcribing,
              state != .media,
              state != .assistantRecording,
              state != .assistantWorking else { return }

        isRelaunching = true
        rebuildMenu()
        relaunch(with: newEngine)
    }

    private func relaunch(with engine: SpeechEngine) {
        let argument = engine.launchArgument
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

    static func failure(_ message: String, code: Int = 1) -> NSError {
        NSError(domain: "Koett", code: code, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private static func loadShortcut(
        key: String,
        defaults: UserDefaults
    ) -> ShortcutBinding? {
        guard let data = defaults.data(forKey: key) else { return nil }
        guard let binding = try? JSONDecoder().decode(ShortcutBinding.self, from: data),
              binding.isValid else {
            return nil
        }
        return binding
    }

    private func saveShortcut(_ binding: ShortcutBinding, key: String) {
        do {
            UserDefaults.standard.set(try JSONEncoder().encode(binding), forKey: key)
        } catch {
            fputs("Error: The shortcut could not be saved.\n", stderr)
        }
    }

    private func showShortcutConflict(
        with name: String,
        binding: ShortcutBinding
    ) {
        let alert = NSAlert()
        alert.messageText = "That conflicts with \(name)."
        alert.informativeText = "Current shortcut: \(binding.displayName)"
        alert.addButton(withTitle: "OK")
        _ = alert.runModal()
    }

    func promptForText(
        title: String,
        message: String,
        current: String,
        secure: Bool
    ) -> String? {
        let previousApplication = NSWorkspace.shared.frontmostApplication
        isCapturingShortcut = true
        defer {
            previousApplication?.activate(options: [])
            DispatchQueue.main.async { [weak self] in
                self?.isCapturingShortcut = false
            }
        }

        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")

        let field: NSTextField = secure
            ? NSSecureTextField(frame: .zero)
            : NSTextField(frame: .zero)
        field.stringValue = current
        field.placeholderString = secure ? "API key" : nil
        field.frame = NSRect(x: 0, y: 0, width: 420, height: 24)
        alert.accessoryView = field

        _ = NSRunningApplication.current.activate(options: [.activateAllWindows])
        NSApp.activate(ignoringOtherApps: true)
        alert.window.initialFirstResponder = field
        return alert.runModal() == .alertFirstButtonReturn
            ? field.stringValue
            : nil
    }

}

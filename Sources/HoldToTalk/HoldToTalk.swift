import AppKit
import ApplicationServices
import AVFoundation
import CoreGraphics
import Darwin
import FluidAudio
import Foundation
import ServiceManagement

@MainActor
private final class HoldToTalkController: NSObject {
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

    private let manager = AsrManager(config: .default)
    private let startSound: AVAudioPlayer
    private let stopSound: AVAudioPlayer
    private var recordingMode: RecordingMode
    private var shortcut: Shortcut
    private var state = State.loading
    private var recorder: AVAudioRecorder?
    private var recordingURL: URL?
    private var monitor: Any?
    private var statusItem: NSStatusItem?

    init(defaults: UserDefaults) throws {
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
                "Microphone access is not allowed. Enable it in System Settings > Privacy & Security > Microphone."
            )
        }

        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        guard AXIsProcessTrustedWithOptions(options) else {
            throw Self.failure(
                "Allow Accessibility access in System Settings > Privacy & Security > Accessibility, then run this command again."
            )
        }

        print("Loading Parakeet v2...")
        let models = try await AsrModels.downloadAndLoad(version: .v2)
        try await manager.loadModels(models)
        state = .ready
        print("Ready. Use the menu-bar icon to change the mode or shortcut.")
    }

    func installHotkey() throws {
        monitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            let keyCode = event.keyCode
            let modifierFlags = event.modifierFlags.rawValue
            Task { @MainActor [weak self] in
                self?.handleShortcut(keyCode: keyCode, modifierFlags: modifierFlags)
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
            accessibilityDescription: "Local Voice Input"
        )
        item.button?.toolTip = "Local Voice Input"
        statusItem = item
        rebuildMenu()
    }

    private func handleShortcut(keyCode: UInt16, modifierFlags: UInt) {
        guard shortcut.keyCodes.contains(keyCode) else { return }
        let flags = NSEvent.ModifierFlags(rawValue: modifierFlags)
            .intersection(.deviceIndependentFlagsMask)
        let isDown = flags.contains(shortcut.modifier)

        switch recordingMode {
        case .hold:
            handleHold(isDown: isDown)
        case .toggle:
            if isDown {
                toggleRecording()
            }
        }
    }

    private func handleHold(isDown: Bool) {
        if isDown {
            startRecordingIfReady()
        } else {
            guard state == .recording else { return }
            stopRecording()
        }
    }

    private func toggleRecording() {
        switch state {
        case .ready:
            startRecordingIfReady()
        case .recording:
            stopRecording()
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
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("local-voice-input-\(UUID().uuidString).wav")
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
        guard newRecorder.prepareToRecord(), newRecorder.record() else {
            throw Self.failure("The microphone recorder could not start.")
        }

        recorder = newRecorder
        recordingURL = url
        state = .recording
        play(startSound)
        print("RECORDING")
    }

    private func stopRecording() {
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
            await self?.transcribe(url)
        }
    }

    private func transcribe(_ url: URL) async {
        defer {
            cleanup(url)
            state = .ready
        }

        do {
            let decoderLayers = await manager.decoderLayerCount
            var decoderState = TdtDecoderState.make(decoderLayers: decoderLayers)
            let result = try await manager.transcribe(url, decoderState: &decoderState)
            let text = result.text.trimmingCharacters(in: .whitespacesAndNewlines)

            guard !text.isEmpty else {
                print("No speech detected. Clipboard unchanged.")
                return
            }

            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            guard pasteboard.setString(text, forType: .string) else {
                throw Self.failure("The transcript could not be copied to the clipboard.")
            }

            guard pasteAtCursor() else {
                throw Self.failure("The transcript is on the clipboard, but Command-V could not be sent.")
            }

            print("PASTED: \(text)")
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
        }
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
        menu.addItem(NSMenuItem(
            title: "Quit Local Voice Input",
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

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "HoldToTalk", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
}

@main
private struct HoldToTalk {
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
            print("Usage: hold-to-talk")
            print("Hold either Option key to record. Release it to transcribe and paste.")
            return
        }

        do {
            let controller = try HoldToTalkController(defaults: .standard)
            let delegate = HoldToTalkDelegate(controller: controller)
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
                domain: "HoldToTalk",
                code: 2,
                userInfo: [
                    NSLocalizedDescriptionKey: "Enable Local Voice Input in System Settings > General > Login Items."
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
private final class HoldToTalkDelegate: NSObject, NSApplicationDelegate {
    private let controller: HoldToTalkController

    init(controller: HoldToTalkController) {
        self.controller = controller
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        controller.installMenu()
        print("Checking microphone and Accessibility access...")
        Task { @MainActor in
            do {
                try await controller.prepare()
                try controller.installHotkey()
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                NSApplication.shared.terminate(nil)
            }
        }
    }
}

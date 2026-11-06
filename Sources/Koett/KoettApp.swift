import AppKit
import Foundation
import ServiceManagement

@main
private struct Koett {
    @MainActor
    private static func importDictionary(path: String?) throws {
        guard let path else { throw NSError(domain: "Koett", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "--import-dictionary needs a file.",
        ]) }
        let own = ProcessInfo.processInfo.processIdentifier
        let running = NSWorkspace.shared.runningApplications.contains {
            $0.bundleIdentifier == Bundle.main.bundleIdentifier && $0.processIdentifier != own
        }
        guard !running else { throw NSError(domain: "Koett", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "Quit Koett first; the running app would overwrite the dictionary.",
        ]) }
        let store = DictionaryStore(defaults: .standard)
        let text = try String(contentsOfFile: path, encoding: .utf8)
        try store.save(KoettDictionary.merge(store.entries, KoettDictionary.parseText(text)))
        print(KoettDictionary.toText(store.entries))
    }

    @MainActor
    static func main() {
        let arguments = CommandLine.arguments.dropFirst()

        if let flag = arguments.firstIndex(of: "--control") {
            let commands = Array(arguments.dropFirst(flag - arguments.startIndex + 1))
            let reply: ControlReply
            do { reply = try LocalControlSocket.request(commands) }
            catch { reply = .failure("unavailable", error.localizedDescription) }
            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            if let data = try? encoder.encode(reply), let json = String(data: data, encoding: .utf8) { print(json) }
            exit(reply.ok ? EXIT_SUCCESS : EXIT_FAILURE)
        }

        if arguments.contains("--register-login") {
            do {
                try registerLoginItem()
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(EXIT_FAILURE)
            }
            return
        }

        if let flag = arguments.firstIndex(of: "--import-dictionary") {
            do {
                try importDictionary(path: arguments.dropFirst(flag - arguments.startIndex + 1).first)
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

        if arguments.contains("--import-groq-key-if-missing") {
            guard AssistantAvailability.isEnabled else {
                fputs("Unsupported command. Run koett --help.\n", stderr)
                exit(EXIT_FAILURE)
            }
            do {
                if try AssistantAPIKeyStore.load(for: .groq) != nil {
                    print("Groq API key is already in Keychain.")
                    return
                }
                guard let key = ProcessInfo.processInfo.environment[
                    "GROQ_API_KEY"
                ] else {
                    throw NSError(
                        domain: "Koett",
                        code: 1,
                        userInfo: [
                            NSLocalizedDescriptionKey: "GROQ_API_KEY is not set."
                        ]
                    )
                }
                try AssistantAPIKeyStore.save(key, for: .groq)
                print("Groq API key saved in Keychain.")
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(EXIT_FAILURE)
            }
            return
        }

        if arguments.contains("--import-cartesia-key") {
            guard AssistantAvailability.isEnabled else {
                fputs("Unsupported command. Run koett --help.\n", stderr)
                exit(EXIT_FAILURE)
            }
            do {
                guard let key = ProcessInfo.processInfo.environment[
                    "CARTESIA_API_KEY"
                ] else {
                    throw NSError(
                        domain: "Koett",
                        code: 1,
                        userInfo: [
                            NSLocalizedDescriptionKey: "CARTESIA_API_KEY is not set."
                        ]
                    )
                }
                try AssistantAPIKeyStore.saveCartesia(key)
                print("Cartesia API key saved in Keychain.")
            } catch {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(EXIT_FAILURE)
            }
            return
        }

        if arguments.contains("--help") {
            print("Usage: koett [--parakeet | --parakeet-v3 | --parakeet-110m | --nemotron]")
            print("       koett --control COMMAND [ARG]   (local JSON control; status by default)")
            print("       koett --import-dictionary FILE   (while Koett is quit)")
            print("Command-line model flags override the saved model for this launch.")
            print("Use the menu-bar icon to choose the model, mode, and shortcut.")
            return
        }

        do {
            let speechEngine = SpeechEngine.selected(arguments: Array(arguments), defaults: .standard)
            let controller = try KoettController(
                defaults: .standard,
                speechEngine: speechEngine,
                pasteDiagnostics: PasteDiagnostics(directory: URL.applicationSupportDirectory
                    .appendingPathComponent("Koett/Diagnostics", isDirectory: true))
            )
            let delegate = KoettDelegate(controller: controller)
            let application = NSApplication.shared
            application.setActivationPolicy(.accessory)
            EditMenu.install(into: application)
            application.delegate = delegate
            application.run()
            withExtendedLifetime(delegate) {}
        } catch {
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(EXIT_FAILURE)
        }
    }

    private static func registerLoginItem() throws {
        guard LoginPreference.shouldRegister(in: .standard) else {
            print("Login Item remains disabled by preference.")
            return
        }
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
        try LoginPreference.unregisterIfRegistered(status: service.status) {
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
        controller.startLocalControl()
        print("Checking microphone and Accessibility access...")
        Task { @MainActor in
            await controller.start()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        controller.controlServer?.stop()
        controller.stopS1Mini()
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        controller.prepareTermination()
    }
}

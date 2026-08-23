import AppKit
import Foundation
import ServiceManagement

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

        if arguments.contains("--import-groq-key-if-missing") {
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

    func applicationWillTerminate(_ notification: Notification) {
        controller.stopS1Mini()
    }
}

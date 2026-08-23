import AppKit
import Foundation

extension KoettController {
    func rebuildMenu() {
        let menu = NSMenu()
        if isPreparing {
            let item = NSMenuItem(
                title: setupStatus.menuTitle,
                action: nil,
                keyEquivalent: ""
            )
            item.isEnabled = false
            menu.addItem(item)
            menu.addItem(.separator())
        } else if let startupErrorMessage {
            let item = NSMenuItem(title: "Setup Required", action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
            let reason = NSMenuItem(
                title: SetupStatus.shortError(
                    code: startupErrorCode,
                    message: startupErrorMessage
                ),
                action: nil,
                keyEquivalent: ""
            )
            reason.isEnabled = false
            reason.toolTip = startupErrorMessage
            menu.addItem(reason)
            if startupErrorCode == .microphone {
                menu.addItem(menuItem(
                    title: "Open Microphone Settings",
                    action: #selector(openMicrophoneSettings),
                    selected: false
                ))
            }
            if startupErrorCode == .accessibility {
                menu.addItem(menuItem(
                    title: "Open Accessibility Settings",
                    action: #selector(openAccessibilitySettings),
                    selected: false
                ))
            }
            menu.addItem(menuItem(
                title: "Show Error…",
                action: #selector(showSetupError),
                selected: false
            ))
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
            && state != .media
            && state != .assistantRecording
            && state != .assistantWorking
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

        let formattingMode = s1MiniEnabled ? "S1-mini" : "Raw"
        let formattingItem = menuItem(
            title: "Formatting: \(formattingMode)…",
            action: #selector(showFormattingPopover),
            selected: false
        )
        formattingItem.isEnabled = canChangeModel
        menu.addItem(formattingItem)
        menu.addItem(.separator())
        let toggleItem = menuItem(
            title: "Mode: Toggle",
            action: #selector(useToggleMode),
            selected: recordingMode == .toggle
        )
        toggleItem.isEnabled = state == .ready
        menu.addItem(toggleItem)
        let holdItem = menuItem(
            title: "Mode: Hold",
            action: #selector(useHoldMode),
            selected: recordingMode == .hold
        )
        holdItem.isEnabled = state == .ready
        menu.addItem(holdItem)
        menu.addItem(.separator())
        let dictationShortcutItem = menuItem(
            title: "Dictation Shortcut: \(dictationShortcut.displayName)",
            action: #selector(bindDictationShortcut),
            selected: false
        )
        dictationShortcutItem.isEnabled = state == .ready
        menu.addItem(dictationShortcutItem)
        let mediaShortcutItem = menuItem(
            title: "Media Shortcut: \(mediaShortcut.displayName)",
            action: #selector(bindMediaShortcut),
            selected: false
        )
        mediaShortcutItem.isEnabled = state == .ready
        menu.addItem(mediaShortcutItem)
        let resetShortcutsItem = menuItem(
            title: "Reset Shortcuts to Defaults",
            action: #selector(resetShortcuts),
            selected: false
        )
        resetShortcutsItem.isEnabled = state == .ready
        menu.addItem(resetShortcutsItem)
        menu.addItem(.separator())

        let assistantMenu = NSMenu(title: "Ask About Screen")
        let askTitle: String
        switch state {
        case .assistantRecording:
            askTitle = "Stop Asking"
        case .assistantWorking:
            askTitle = "Interrupt and Ask"
        default:
            askTitle = "Ask About Screen"
        }
        let askItem = menuItem(
            title: askTitle,
            action: #selector(askAboutScreen),
            selected: false
        )
        askItem.isEnabled = state == .ready
            || state == .assistantRecording
            || state == .assistantWorking
        assistantMenu.addItem(askItem)
        let assistantShortcutItem = menuItem(
            title: "Shortcut: \(assistantShortcut.displayName)",
            action: #selector(bindAssistantShortcut),
            selected: false
        )
        assistantShortcutItem.isEnabled = state == .ready
        assistantMenu.addItem(assistantShortcutItem)
        assistantMenu.addItem(.separator())
        for provider in AssistantProvider.allCases {
            let item = menuItem(
                title: "Provider: \(provider.displayName)",
                action: providerSelector(provider),
                selected: assistantProvider == provider
            )
            item.isEnabled = state == .ready
            assistantMenu.addItem(item)
        }
        let configuration = try? AssistantConfiguration.load(
            provider: assistantProvider
        )
        let modelName = configuration?.model ?? "Not set"
        let modelItem = menuItem(
            title: "Assistant Model: \(modelName)",
            action: #selector(setAssistantModel),
            selected: false
        )
        modelItem.isEnabled = state == .ready
        assistantMenu.addItem(modelItem)
        if assistantProvider == .custom {
            let endpointItem = menuItem(
                title: "Set Custom Endpoint…",
                action: #selector(setCustomAssistantEndpoint),
                selected: false
            )
            endpointItem.isEnabled = state == .ready
            assistantMenu.addItem(endpointItem)
        }
        let keyItem = menuItem(
            title: "Set \(assistantProvider.displayName) API Key…",
            action: #selector(setAssistantAPIKey),
            selected: false
        )
        keyItem.isEnabled = state == .ready
        assistantMenu.addItem(keyItem)
        assistantMenu.addItem(.separator())
        let speakItem = menuItem(
            title: "Speak Replies",
            action: #selector(toggleAssistantSpeech),
            selected: assistantSpeechEnabled
        )
        speakItem.isEnabled = state == .ready
        assistantMenu.addItem(speakItem)

        let voiceMenu = NSMenu(title: "Voice")
        for voice in AssistantVoice.allCases {
            let item = menuItem(
                title: voice.displayName,
                action: voiceSelector(voice),
                selected: assistantVoice == voice
            )
            item.isEnabled = state == .ready
            voiceMenu.addItem(item)
        }
        let voiceItem = NSMenuItem(
            title: "Voice: \(assistantVoice.displayName)",
            action: nil,
            keyEquivalent: ""
        )
        voiceItem.submenu = voiceMenu
        assistantMenu.addItem(voiceItem)

        let cartesiaKeyItem = menuItem(
            title: "Set Cartesia API Key…",
            action: #selector(setCartesiaAPIKey),
            selected: false
        )
        cartesiaKeyItem.isEnabled = state == .ready
        assistantMenu.addItem(cartesiaKeyItem)
        assistantMenu.addItem(menuItem(
            title: "Open Screen Recording Settings",
            action: #selector(openScreenRecordingSettings),
            selected: false
        ))

        let assistantRootItem = NSMenuItem(
            title: "Ask About Screen",
            action: nil,
            keyEquivalent: ""
        )
        assistantRootItem.submenu = assistantMenu
        menu.addItem(assistantRootItem)
        menu.addItem(.separator())
        let transcribeItem = menuItem(
            title: "Transcribe Current Media",
            action: #selector(transcribeCurrentMedia),
            selected: false
        )
        transcribeItem.isEnabled = state == .ready
        transcribeItem.toolTip = "Works with one public media item that yt-dlp can access without login."
        menu.addItem(transcribeItem)
        menu.addItem(menuItem(
            title: "Open Media Transcripts",
            action: #selector(openMediaTranscripts),
            selected: false
        ))
        menu.addItem(.separator())
        menu.addItem(menuItem(
            title: "Open Transcripts",
            action: #selector(openTranscripts),
            selected: false
        ))
        if failedRecordingStore.hasRecordings {
            menu.addItem(menuItem(
                title: "Open Failed Recordings",
                action: #selector(openFailedRecordings),
                selected: false
            ))
        }
        if failedTranscriptStore.hasTranscripts {
            menu.addItem(menuItem(
                title: "Open Failed Transcripts",
                action: #selector(openFailedTranscripts),
                selected: false
            ))
        }
        menu.addItem(NSMenuItem(
            title: "Quit Koett",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        ))
        statusItem?.menu = menu
    }

    private func menuItem(
        title: String,
        action: Selector,
        selected: Bool
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        item.state = selected ? .on : .off
        return item
    }
}

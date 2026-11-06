import AppKit
import Foundation

extension KoettController {
    func rebuildMenu() {
        appUpdater?.stateDidChange()
        let menu = NSMenu()
        menu.delegate = self
        let historyItem = menuItem(title: "History…", action: #selector(openTranscripts), selected: false)
        historyItem.keyEquivalent = "h"
        historyItem.keyEquivalentModifierMask = [.command, .shift]
        menu.addItem(historyItem)
        menu.addItem(menuItem(title: "Dictionary…", action: #selector(openDictionary), selected: false))
        menu.addItem(.separator())
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
        for (engine, action) in [
            (SpeechEngine.parakeetV3, #selector(useParakeetV3Model)),
            (SpeechEngine.parakeet110m, #selector(useParakeet110MModel)),
        ] {
            let item = menuItem(title: "Model: \(engine.displayName)", action: action,
                                selected: speechEngine == engine)
            item.isEnabled = canChangeModel
            menu.addItem(item)
        }
        let nemotronItem = menuItem(
            title: "Model: Nemotron 560 ms (Test)",
            action: #selector(useNemotronModel),
            selected: speechEngine == .nemotron
        )
        nemotronItem.isEnabled = canChangeModel
        menu.addItem(nemotronItem)

        menu.addItem(microphoneMenuItem())
        let pauseMedia = menuItem(title: "Pause Supported Media While Recording", action: #selector(togglePauseMedia),
                                  selected: microphoneDefaults.bool(forKey: "pauseMedia"))
        pauseMedia.isEnabled = state == .ready
        pauseMedia.toolTip = "Pauses visible Music, Spotify, VLC, QuickTime and YouTube controls using existing Accessibility access. Unsupported or paused players stay untouched. Resume playback manually."
        menu.addItem(pauseMedia)

        let formattingMode = s1MiniEnabled ? "S1-mini" : "Raw"
        let formattingItem = menuItem(
            title: "Formatting: \(formattingMode)…",
            action: #selector(showFormattingPopover),
            selected: false
        )
        formattingItem.isEnabled = canChangeModel
        menu.addItem(formattingItem)
        menu.addItem(drawMenuItem())
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
        for (title, action) in [("Transcribe Media File…", #selector(transcribeMediaFile)),
                                ("Transcribe Media URL…", #selector(transcribeMediaURL))] {
            let item = menuItem(title: title, action: action, selected: false)
            item.isEnabled = state == .ready
            menu.addItem(item)
        }
        menu.addItem(recordingLimitMenuItem())
        let resetShortcutsItem = menuItem(
            title: "Reset Shortcuts to Defaults",
            action: #selector(resetShortcuts),
            selected: false
        )
        resetShortcutsItem.isEnabled = state == .ready
        menu.addItem(resetShortcutsItem)
        menu.addItem(.separator())

        addAssistantMenu(to: menu)
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
        if let latestOutcome = recordingOverlay.latestOutcome {
            let latestItem = NSMenuItem(
                title: latestOutcome.menuTitle,
                action: nil,
                keyEquivalent: ""
            )
            latestItem.isEnabled = false
            latestItem.toolTip = latestOutcome.message
            menu.addItem(latestItem)
        }
        let copyLastItem = menuItem(
            title: "Copy Last Transcript",
            action: #selector(copyLastTranscript),
            selected: false
        )
        copyLastItem.isEnabled = state == .ready && lastTranscript != nil
        menu.addItem(copyLastItem)
        let pasteLastItem = menuItem(
            title: "Paste Last Transcript",
            action: #selector(pasteLastTranscript),
            selected: false
        )
        pasteLastItem.isEnabled = state == .ready && lastTranscript != nil
        menu.addItem(pasteLastItem)
        if transcriptPersistence.hasVolatileTranscripts || failedTranscriptStore.hasTranscripts {
            let retry = menuItem(title: "Retry Save Transcripts", action: #selector(retrySaveTranscripts), selected: false)
            retry.isEnabled = state == .ready
            menu.addItem(retry)
        }
        if transcriptPersistence.unsavedTranscriptText != nil {
            let unsavedItem = menuItem(
                title: "Copy Unsaved Transcripts",
                action: #selector(copyUnsavedTranscripts),
                selected: false
            )
            unsavedItem.isEnabled = state == .ready
            menu.addItem(unsavedItem)
        }
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
        menu.addItem(.separator())
        menu.addItem(loginMenuItem())
        menu.addItem(menuItem(title: "Permissions…", action: #selector(showPermissions), selected: false))
        let updates = menuItem(title: "Check for Updates…", action: #selector(checkForUpdates), selected: false)
        updates.target = appUpdater
        updates.isEnabled = appUpdater?.canCheck == true
        updates.toolTip = appUpdater?.lastError ?? (appUpdater?.configuration == nil ? "Updates are not enabled in this build." : nil)
        menu.addItem(updates)
        if let updater = appUpdater, let skipped = updater.skippedBuild {
            let retry = NSMenuItem(title: "Allow Skipped Update (\(skipped))", action: #selector(AppUpdater.allowSkippedUpdate), keyEquivalent: "")
            retry.target = updater
            menu.addItem(retry)
        }
        let versionItem = NSMenuItem(
            title: "Koett \(Self.versionLabel)",
            action: nil,
            keyEquivalent: ""
        )
        versionItem.isEnabled = false
        menu.addItem(versionItem)
        menu.addItem(NSMenuItem(
            title: "Quit Koett",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        ))
        statusItem?.menu = menu
    }

    static var versionLabel: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev"
    }

    func menuItem(
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

from pathlib import Path
root=Path(__file__).resolve().parent/'current-source/Sources/Koett'
p=root/'DictationSession.swift';s=p.read_text();s=s.replace('''    func stopRecording(releaseEventTimestamp: TimeInterval) {
        mediaPauseTask?.cancel()
        recordingOverlay.stop()
        sketchOverlay.finish()
        let session = activeDictationID ?? beginDictationSession()''','''    func stopRecording(releaseEventTimestamp: TimeInterval) {
        let begin = ProcessInfo.processInfo.systemUptime
        mediaPauseTask?.cancel()
        let media = ProcessInfo.processInfo.systemUptime
        recordingOverlay.stop()
        let overlay = ProcessInfo.processInfo.systemUptime
        sketchOverlay.finish()
        let sketch = ProcessInfo.processInfo.systemUptime
        let session = activeDictationID ?? beginDictationSession()
        print("STOP_ENTRY mediaMS=\\((media-begin)*1000) overlayMS=\\((overlay-media)*1000) sketchMS=\\((sketch-overlay)*1000) sessionMS=\\((ProcessInfo.processInfo.systemUptime-sketch)*1000)")''')
s=s.replace('''        guard !handleMicrophoneFailure(recorder, url: url) else { return }
        state = .transcribing
        play(stopSound)
        print("TRANSCRIBING")''','''        let checked = ProcessInfo.processInfo.systemUptime
        let microphoneFailed = handleMicrophoneFailure(recorder, url: url)
        let beforeState = ProcessInfo.processInfo.systemUptime
        guard !microphoneFailed else { return }
        state = .transcribing
        let afterState = ProcessInfo.processInfo.systemUptime
        play(stopSound)
        print("STOP_STATE failureCheckMS=\\((beforeState-checked)*1000) stateMS=\\((afterState-beforeState)*1000) playMS=\\((ProcessInfo.processInfo.systemUptime-afterState)*1000)")
        print("TRANSCRIBING")''',1)
s=s.replace('''        recordingOverlay.showTranscribing()
        let retained''','''        let overlayStart = ProcessInfo.processInfo.systemUptime
        recordingOverlay.showTranscribing()
        print("STOP_SHOW_TRANSCRIBING_MS \\((ProcessInfo.processInfo.systemUptime-overlayStart)*1000)")
        let retained''',1);p.write_text(s)
p=root/'Koett.swift';s=p.read_text().replace('''                rebuildMenu()
                refreshMicrophoneMonitorTarget()''','''                let menuStart = ProcessInfo.processInfo.systemUptime
                rebuildMenu()
                let menuEnd = ProcessInfo.processInfo.systemUptime
                refreshMicrophoneMonitorTarget()
                if state == .transcribing { print("STATE_OBSERVER menuMS=\\((menuEnd-menuStart)*1000) microphoneRefreshMS=\\((ProcessInfo.processInfo.systemUptime-menuEnd)*1000)") }''',1);p.write_text(s)

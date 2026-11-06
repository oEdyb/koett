from pathlib import Path
root=Path(__file__).resolve().parent;repo=root.parents[2];source=root/'current-source/Sources/Koett'
# Match the production cache and menu-refresh implementation; preserve only timing/seam instrumentation.
p=source/'LoginPreference.swift';s=(repo/'Sources/Koett/LoginPreference.swift').read_text().replace('''        let status = loginPreference.menuStatus(refresh: refresh) { SMAppService.mainApp.status }''','''        let began = ProcessInfo.processInfo.systemUptime
        let status = loginPreference.menuStatus(refresh: refresh) { SMAppService.mainApp.status }
        print("LOGIN_STATUS_MS \\((ProcessInfo.processInfo.systemUptime-began)*1000)")''');p.write_text(s)
(source/'MicrophoneMenu.swift').write_text((repo/'Sources/Koett/MicrophoneMenu.swift').read_text())
p=source/'Koett.swift';s=p.read_text().replace('''    var recordingMode: RecordingMode''','''    var loginPreference = LoginPreference()
    var recordingMode: RecordingMode''',1);p.write_text(s)
p=source/'DictationSession.swift';s=p.read_text().replace('detector: ProcessInfo.processInfo.environment["KOETT_PROFILE_NO_VAD"] == "1" ? nil : speechDetector,','detector: speechDetector,');s=s.replace('''        let retained = ProcessInfo.processInfo.environment["KOETT_PROFILE_RETAIN_RECORDER"] == "1" ? recorder : nil
        dictationTask = Task { @MainActor [weak self, retained] in
            defer { withExtendedLifetime(retained) {} }''','''        dictationTask = Task { @MainActor [weak self] in''');p.write_text(s)
s=(root/'run-cache.py').read_text().replace("  order=['baseline','cached'] if repeat%2==0 else ['cached','baseline']", "  order=['production']").replace('cache-', 'production-').replace("==16", "==8").replace('ALL 8 CACHE COMPARISON RECEIPTS PASSED; outputs identical','ALL 4 PRODUCTION FIX RECEIPTS PASSED; outputs identical')
# Three 5-second receipts and one 120-second receipt; each gets a new process/warm-up.
s=s.replace("for seconds in [5,120]:assert", "for seconds in [5,120]:assert")
(root/'run-production.py').write_text(s)

from pathlib import Path
import shutil,subprocess
root=Path(__file__).resolve().parent;host=root.parent/'host'
for kind in ['current','old']:
 source=root/(kind+'-source');source.mkdir(exist_ok=True)
 original=host/(kind+'-source')
 for name in ['Sources','Packaging']:
  shutil.copytree(original/name,source/name,dirs_exist_ok=True)
 for name in ['Package.swift','Package.resolved']:
  if (original/name).exists():shutil.copy2(original/name,source/name)
 p=source/'Sources/Koett/DictationSession.swift';s=p.read_text()
 s=s.replace('    func cleanup(_ url: URL) {','''    func cleanup(_ url: URL) {
        if let root = ProcessInfo.processInfo.environment["KOETT_AUDIT_ROOT"] {
            let folder = URL(fileURLWithPath: root).appendingPathComponent("captured-wavs")
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try? FileManager.default.copyItem(at: url, to: folder.appendingPathComponent(url.lastPathComponent))
        }''')
 if kind=='current':
  s=s.replace('detector: speechDetector,','detector: ProcessInfo.processInfo.environment["KOETT_PROFILE_NO_VAD"] == "1" ? nil : speechDetector,')
  needle='''        recorder.stop()
        self.recorder = nil'''
  s=s.replace(needle,'''        let stopBegan = ProcessInfo.processInfo.systemUptime
        recorder.stop()
        print("RECORDER_STOP_TOTAL_MS \\((ProcessInfo.processInfo.systemUptime-stopBegan)*1000)")
        self.recorder = nil''',1)
  needle='''        dictationTask = Task { @MainActor [weak self] in
            await self?.transcribe(url, releaseEventTimestamp: releaseEventTimestamp, session: session)
        }'''
  replacement='''        let retained = ProcessInfo.processInfo.environment["KOETT_PROFILE_RETAIN_RECORDER"] == "1" ? recorder : nil
        dictationTask = Task { @MainActor [weak self, retained] in
            defer { withExtendedLifetime(retained) {} }
            await self?.transcribe(url, releaseEventTimestamp: releaseEventTimestamp, session: session)
        }'''
  assert needle in s;s=s.replace(needle,replacement,1)
 else:
  s=s.replace('''        recorder.stop()
        self.recorder = nil''','''        let stopBegan = ProcessInfo.processInfo.systemUptime
        recorder.stop()
        print("LEGACY_RECORDER_STOP_MS \\((ProcessInfo.processInfo.systemUptime-stopBegan)*1000)")
        self.recorder = nil''',1)
 p.write_text(s)
 if kind=='current':
  p=source/'Sources/Koett/MicrophoneRecorder.swift';s=p.read_text().replace('''        input.stop()
        sink.close()
        isRecording = false''','''        let began = ProcessInfo.processInfo.systemUptime
        input.stop()
        let stopped = ProcessInfo.processInfo.systemUptime
        sink.close()
        let closed = ProcessInfo.processInfo.systemUptime
        isRecording = false
        print("RECORDER_PARTS inputStopMS=\\((stopped-began)*1000) fileCloseMS=\\((closed-stopped)*1000)")''');p.write_text(s)
  p=source/'Sources/Koett/MicrophoneInput.swift';s=p.read_text().replace('''    isolated deinit {
        AudioOutputUnitStop(unit)
        AudioUnitUninitialize(unit)
        AudioComponentInstanceDispose(unit)
    }''','''    isolated deinit {
        let began = ProcessInfo.processInfo.systemUptime
        AudioOutputUnitStop(unit)
        let stopped = ProcessInfo.processInfo.systemUptime
        AudioUnitUninitialize(unit)
        let uninitialized = ProcessInfo.processInfo.systemUptime
        AudioComponentInstanceDispose(unit)
        print("INPUT_DEINIT stopMS=\\((stopped-began)*1000) uninitializeMS=\\((uninitialized-stopped)*1000) disposeMS=\\((ProcessInfo.processInfo.systemUptime-uninitialized)*1000)")
    }''');p.write_text(s)
print('Private profiling source copies ready')

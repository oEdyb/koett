from pathlib import Path
import shutil
root=Path(__file__).resolve().parent;host=root.parent/'host';out=root/'cache-rebuild';out.mkdir(exist_ok=True)
s=(root/'prepare.py').read_text().replace("root=Path(__file__).resolve().parent;host=root.parent/'host'", "root=Path("+repr(str(out))+ ");host=Path("+repr(str(host))+")").replace("['current','old']", "['current']")
exec(compile(s,str(out/'prepare.py'),'exec'),{'__file__':str(out/'prepare.py')})
shutil.copytree(host/'current-source/Tests',out/'current-source/Tests',dirs_exist_ok=True)
exec(compile((root/'add-stop-stages.py').read_text(),str(out/'add-stop-stages.py'),'exec'),{'__file__':str(out/'add-stop-stages.py')})
source=out/'current-source/Sources/Koett'
p=source/'Koett.swift';s=p.read_text().replace('''        let models = try await AsrModels.downloadAndLoad(''','''        let modelLoadBegan = ProcessInfo.processInfo.systemUptime
        let models = try await AsrModels.downloadAndLoad(''',1).replace('''        setSetupStatus(.warmingModel(modelName))''','''        print("STARTUP_MODEL_LOAD_MS \\((ProcessInfo.processInfo.systemUptime-modelLoadBegan)*1000)")
        let modelWarmBegan = ProcessInfo.processInfo.systemUptime
        setSetupStatus(.warmingModel(modelName))''',1).replace('''        startLoadingSpeechDetector()''','''        print("STARTUP_ASR_WARM_MS \\((ProcessInfo.processInfo.systemUptime-modelWarmBegan)*1000)")
        startLoadingSpeechDetector()''',1);p.write_text(s)
p=source/'LoginPreference.swift';s=p.read_text().replace('''extension KoettController {''','''@MainActor private enum AuditLoginStatus {
    static var cached: SMAppService.Status?
}

extension KoettController {''',1).replace('''        let status = SMAppService.mainApp.status''','''        let began = ProcessInfo.processInfo.systemUptime
        let status: SMAppService.Status
        if ProcessInfo.processInfo.environment["KOETT_PROFILE_CACHE_LOGIN"] == "1",
           state != .ready, let cached = AuditLoginStatus.cached {
            status = cached
        } else {
            status = SMAppService.mainApp.status
            AuditLoginStatus.cached = status
        }
        print("LOGIN_STATUS_MS \\((ProcessInfo.processInfo.systemUptime-began)*1000)")''',1);p.write_text(s)
p=source/'KoettMenu.swift';s=p.read_text().replace('''        menu.addItem(microphoneMenuItem())''','''        let micBegan = ProcessInfo.processInfo.systemUptime
        menu.addItem(microphoneMenuItem())
        print("MIC_MENU_MS \\((ProcessInfo.processInfo.systemUptime-micBegan)*1000)")''');p.write_text(s)
print(out/'current-source')

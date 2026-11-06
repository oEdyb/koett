from pathlib import Path
import subprocess,shutil
root=Path(__file__).resolve().parent;host=root.parent/'host'
(root/'home/Library/Application Support').mkdir(parents=True,exist_ok=True)
cache=root/'home/Library/Application Support/FluidAudio'
if not cache.exists():subprocess.run(['cp','-cR',str(host/'home/Library/Application Support/FluidAudio'),str(cache)],check=True)
app=root/'Current.app'
if not app.exists():subprocess.run(['cp','-cR',str(host/'Current.app'),str(app)],check=True)
shutil.copy2(host/'current-build/release/koett',app/'Contents/MacOS/koett')
subprocess.run(['codesign','--force','--options','runtime','--sign','6D37E71BA097E1FFA9C61A0E55621A835B0D59B3',str(app)],check=True)
driver=root/'Driver.app'
if not driver.exists():subprocess.run(['cp','-cR',str(host/'Driver.app'),str(driver)],check=True)
s=(host/'HostDriver.swift').read_text()
s=s.replace('''   _ = try await tap("start");try await Task.sleep(for:.milliseconds(300))''','''   let requestedStart = ProcessInfo.processInfo.systemUptime
   _ = try await tap("start")
   var captureReadyMilliseconds = 0.0
   let captureReadyDeadline = ProcessInfo.processInfo.systemUptime + 5
   while ProcessInfo.processInfo.systemUptime < captureReadyDeadline {
    _ = try await tap("progress")
    if (lastAppAck["framesInjected"] as? Int ?? 0) > 0 {
     captureReadyMilliseconds = (ProcessInfo.processInfo.systemUptime-requestedStart)*1000
     break
    }
    try await Task.sleep(for:.milliseconds(5))
   }
   guard captureReadyMilliseconds > 0 else {throw NSError(domain:"CaptureNotReady",code:1)}
   try await Task.sleep(for:.milliseconds(300))''')
s=s.replace('''   row["stopUptime"]=stop;''','''   row["captureReadyMilliseconds"]=captureReadyMilliseconds
   row["stopUptime"]=stop;''')
(root/'HostDriver.swift').write_text(s)
subprocess.run(['swiftc','-swift-version','5',str(root/'HostDriver.swift'),'-o',str(driver/'Contents/MacOS/koett')],check=True)
subprocess.run(['codesign','--force','--options','runtime','--entitlements',str(host/'driver.entitlements'),'--sign','6D37E71BA097E1FFA9C61A0E55621A835B0D59B3',str(driver)],check=True)

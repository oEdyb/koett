import AppKit
import AVFoundation
import Foundation
import ApplicationServices
guard NSUserName() == "admin", NSHomeDirectory().hasPrefix("/Users/admin") else { exit(2) }
let args=Array(CommandLine.arguments.dropFirst())
fputs("AX=\(AXIsProcessTrusted()) POST=\(CGPreflightPostEventAccess())\n", stderr)
let source=CGEventSource(stateID:.hidSystemState)!
func tap() -> Double {
 let down=CGEvent(keyboardEventSource:source,virtualKey:40,keyDown:true)!
 down.flags = [.maskControl,.maskShift]
 let start=ProcessInfo.processInfo.systemUptime
 down.post(tap:.cghidEventTap)
 usleep(40000)
 let up=CGEvent(keyboardEventSource:source,virtualKey:40,keyDown:false)!
 up.flags=[]
 up.post(tap:.cghidEventTap)
 return start
}
let url=URL(fileURLWithPath:args[0])
let player=try AVAudioPlayer(contentsOf:url)
player.prepareToPlay()
let receipt=URL(fileURLWithPath:"/tmp/koett-paste-receipt.json")
try? FileManager.default.removeItem(at:receipt)
for code: CGKeyCode in [0, 51] {
 for down in [true, false] {
  let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!
  event.flags = code == 0 ? .maskCommand : []
  event.post(tap:.cghidEventTap)
 }
 usleep(50000)
}
_ = tap()
usleep(300000)
player.play()
while player.isPlaying { usleep(10000) }
usleep(150000)
let stop = tap()
let deadline=ProcessInfo.processInfo.systemUptime+180
while ProcessInfo.processInfo.systemUptime<deadline {
 if let data=try? Data(contentsOf:receipt), var object=try JSONSerialization.jsonObject(with:data) as? [String:Any],let received=object["receivedUptime"] as? Double {
  object["stopUptime"]=stop
  object["milliseconds"]=(received-stop)*1000
  object["fixture"]=url.lastPathComponent
  print(String(data:try JSONSerialization.data(withJSONObject:object,options:[.sortedKeys]),encoding:.utf8)!)
  exit(0)
 }
 usleep(10000)
}
fputs("No target paste receipt\n",stderr)
exit(3)

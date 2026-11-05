import AppKit
import AVFoundation
import Foundation
import ApplicationServices
import CryptoKit
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
guard args.count == 2 else { exit(2) }
let requestID = UUID().uuidString
let expectedNonce = args[1]
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
let board = NSPasteboard.general
board.clearContents()
guard board.setString("AUDIT_SENTINEL_" + requestID, forType: .string) else { exit(4) }
let before = board.changeCount
let request: [String: Any] = ["requestID":requestID,"expectedAudioNonce":expectedNonce,"clipboardBefore":before]
try JSONSerialization.data(withJSONObject: request).write(to: URL(fileURLWithPath:"/tmp/koett-paste-request.json"),options:.atomic)
_ = tap()
usleep(300000)
player.play()
while player.isPlaying { usleep(10000) }
usleep(150000)
let stop = tap()
let deadline=ProcessInfo.processInfo.systemUptime+180
while ProcessInfo.processInfo.systemUptime<deadline {
 if let data=try? Data(contentsOf:receipt), var object=try JSONSerialization.jsonObject(with:data) as? [String:Any],let received=object["receivedUptime"] as? Double {
  let text = object["text"] as? String ?? ""
  let clipboard = object["clipboardText"] as? String ?? ""
  func normalized(_ value:String)->String { value.lowercased().replacingOccurrences(of:"[^a-z0-9]",with:"",options:.regularExpression) }
  guard object["requestID"] as? String == requestID,
        received > stop,
        (object["clipboardChangeCount"] as? Int ?? -1) > before,
        text == clipboard,
        normalized(text).contains(normalized(expectedNonce)),
        !text.contains("AUDIT_SENTINEL_") else {
   fputs("Guard failed: " + String(data:data,encoding:.utf8)! + "\n",stderr)
   exit(5)
  }
  object["stopUptime"]=stop
  object["clipboardBefore"]=before
  object["expectedAudioNonce"]=expectedNonce
  object["guarded"]=true
  object["fixtureSHA256"]=SHA256.hash(data:try Data(contentsOf:url)).map { String(format:"%02x",$0) }.joined()
  object["audioSeconds"]=player.duration
  object["milliseconds"]=(received-stop)*1000
  object["fixture"]=url.lastPathComponent
  print(String(data:try JSONSerialization.data(withJSONObject:object,options:[.sortedKeys]),encoding:.utf8)!)
  exit(0)
 }
 usleep(10000)
}
fputs("No target paste receipt\n",stderr)
exit(3)

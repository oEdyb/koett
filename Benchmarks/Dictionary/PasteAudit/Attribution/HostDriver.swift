import AppKit
import AVFoundation
import ApplicationServices
import CryptoKit
import Foundation
import IOKit
let root=URL(fileURLWithPath:CommandLine.arguments[1])
let hid=IOServiceGetMatchingService(kIOMainPortDefault,IOServiceMatching("IOHIDSystem"))
func idleSeconds()->Double {
 guard hid != 0,let value=IORegistryEntryCreateCFProperty(hid,"HIDIdleTime" as CFString,kCFAllocatorDefault,0)?.takeRetainedValue() as? NSNumber else{return -1}
 return value.doubleValue/1e9
}
guard ProcessInfo.processInfo.environment["KOETT_AUDIT_HOST"] == "1",NSHomeDirectory()==root.path+"/home" else{exit(2)}
let ownPID=ProcessInfo.processInfo.processIdentifier
let board=NSPasteboard.general
var original:[[NSPasteboard.PasteboardType:Data]]=[]
var ownedGeneration:Int?
var previousApp:NSRunningApplication?
var pasteRow:[String:Any]?
var requestID=""
var expectedAppPID:Int32=0
var activeFixturePath=""
var lastAppAck:[String:Any]=[:]
func emit(_ row:[String:Any]) {try! JSONSerialization.data(withJSONObject:row,options:.sortedKeys).write(to:root.appendingPathComponent("driver-state.json"),options:.atomic)}
func restore()->Bool {
 guard let generation=ownedGeneration,board.changeCount==generation else{return false}
 let items=original.map { entries in let item=NSPasteboardItem();for(type,data) in entries {item.setData(data,forType:type)};return item }
 board.clearContents();if !items.isEmpty {guard board.writeObjects(items) else{return false}}
 ownedGeneration=board.changeCount
 let current=(board.pasteboardItems ?? []).map { item in Dictionary(uniqueKeysWithValues:item.types.compactMap { type in item.data(forType:type).map {(type,$0)} }) }
 return current == original
}
func foreground()->Bool {NSWorkspace.shared.frontmostApplication?.processIdentifier==ownPID}
@MainActor @Sendable func tap(_ command: String) async throws -> Double {
 guard command == "progress" || command == "abort" || foreground() else {throw NSError(domain:"OwnedTargetLostFocus",code:1)}
 let id=UUID().uuidString
 try JSONSerialization.data(withJSONObject:["command":command,"requestID":id,"targetPID":String(ownPID),"fixture":activeFixturePath]).write(to:root.appendingPathComponent("app-command.json"),options:.atomic)
 let deadline=ProcessInfo.processInfo.systemUptime+5
 while ProcessInfo.processInfo.systemUptime<deadline {
  if let data=try? Data(contentsOf:root.appendingPathComponent("app-ack.json")),let ack=try? JSONSerialization.jsonObject(with:data) as? [String:Any],ack["requestID"] as? String==id {
   guard ack["ok"] as? Bool == true,let now=ack["uptime"] as? Double else{throw NSError(domain:"AppNotReady",code:1)}
   lastAppAck=ack
   return now
  }
  try await Task.sleep(for:.milliseconds(5))
 }
 throw NSError(domain:"AppControlTimeout",code:1)
}
final class OffscreenWindow:NSWindow {override func constrainFrameRect(_ frameRect:NSRect,to screen:NSScreen?)->NSRect {frameRect}}
final class Receiver:NSTextView {
 override func paste(_ sender:Any?) {
  super.paste(sender)
  let received=ProcessInfo.processInfo.systemUptime
  pasteRow=["receivedUptime":received,"text":string,"clipboardText":board.string(forType:.string) ?? "","clipboardChangeCount":board.changeCount,"requestID":requestID,"eventSourcePID":NSApp.currentEvent?.cgEvent?.getIntegerValueField(.eventSourceUnixProcessID) ?? -1,"appClipboardGeneration":NSApp.currentEvent?.cgEvent?.getIntegerValueField(.eventSourceUserData) ?? -1]
 }
}
let app=NSApplication.shared
app.setActivationPolicy(.regular)
let menu=NSMenu();let editItem=NSMenuItem();let edit=NSMenu(title:"Edit")
edit.addItem(withTitle:"Paste",action:#selector(NSText.paste(_:)),keyEquivalent:"v");editItem.submenu=edit;menu.addItem(editItem);app.mainMenu=menu
let window=OffscreenWindow(contentRect:NSRect(x:-20000,y:-20000,width:600,height:300),styleMask:.titled,backing:.buffered,defer:false)
window.alphaValue=0
let text=Receiver(frame:window.contentView!.bounds);window.contentView=text
original=(board.pasteboardItems ?? []).map{item in Dictionary(uniqueKeysWithValues:item.types.compactMap{type in item.data(forType:type).map{(type,$0)}})}
ownedGeneration=board.changeCount
previousApp=NSWorkspace.shared.frontmostApplication
window.makeKeyAndOrderFront(nil);window.makeFirstResponder(text);app.activate(ignoringOtherApps:true)
emit(["ready":true,"ax":AXIsProcessTrusted(),"post":CGPreflightPostEventAccess(),"pid":ownPID,"home":NSHomeDirectory(),"windowAlpha":window.alphaValue,"windowX":window.frame.minX])
DispatchQueue.main.asyncAfter(deadline:.now()+0.5) {
 window.makeKeyAndOrderFront(nil);window.makeFirstResponder(text);app.activate(ignoringOtherApps:true)
 emit(["ready":true,"pid":ownPID,"foreground":foreground(),"windowAlpha":window.alphaValue,"originalItemCount":original.count])
}
var busy=false
var cancelRequested=false
let timer=Timer.scheduledTimer(withTimeInterval:0.05,repeats:true){_ in
 guard let data=try? Data(contentsOf:root.appendingPathComponent("driver-command.json")),let command=try? JSONSerialization.jsonObject(with:data) as? [String:String] else{return}
 if busy {
  if command["command"]=="cancel" {cancelRequested=true;try? FileManager.default.removeItem(at:root.appendingPathComponent("driver-command.json"))}
  return
 }
 try? FileManager.default.removeItem(at:root.appendingPathComponent("driver-command.json"))
 if command["command"]=="focus" {
  let owned=NSWorkspace.shared.frontmostApplication?.bundleURL?.path.hasPrefix(root.path) == true
  guard foreground() || owned else {emit(["error":"Existing user app has focus"]);return}
  window.makeKeyAndOrderFront(nil);window.makeFirstResponder(text);app.activate(ignoringOtherApps:true)
  DispatchQueue.main.asyncAfter(deadline:.now()+0.2) {emit(["focused":foreground(),"pid":ownPID])}
  return
 }
 if command["command"]=="quit" {
  let restored=restore();if foreground() {previousApp?.activate(options:[])};emit(["stopped":true,"clipboardRestored":restored]);app.terminate(nil);return
 }
 busy=true;cancelRequested=false
 Task { @MainActor in
  defer {busy=false}
  var diagnostic:[String:Any]=[:]
  do {
   guard foreground(),board.changeCount==ownedGeneration else {throw NSError(domain:"UserStateChanged",code:1)}
   guard let path=command["path"],let nonce=command["nonce"] else{throw NSError(domain:"Arguments",code:1)}
   expectedAppPID=Int32(command["appPID"] ?? "0") ?? 0
   activeFixturePath=path
   text.string="";pasteRow=nil;requestID=UUID().uuidString
   board.clearContents();guard board.setString("AUDIT_SENTINEL_"+requestID,forType:.string) else{throw NSError(domain:"Clipboard",code:1)}
   let before=board.changeCount;ownedGeneration=before
   let requestedStart = ProcessInfo.processInfo.systemUptime
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
   try await Task.sleep(for:.milliseconds(300))
   let expectedSHA=SHA256.hash(data:try Data(contentsOf:URL(fileURLWithPath:path))).map{String(format:"%02x",$0)}.joined()
   guard lastAppAck["fixtureSHA256"] as? String==expectedSHA else {throw NSError(domain:"LoadedFixtureMismatch",code:1)}
   let playbackDeadline=ProcessInfo.processInfo.systemUptime+650
   var previousIdle=idleSeconds()
   var nextIdleCheck=ProcessInfo.processInfo.systemUptime+0.5
   while true {
    guard !cancelRequested else {throw NSError(domain:"AuditCancelled",code:1)}
    if ProcessInfo.processInfo.systemUptime>=nextIdleCheck {
     let idle=idleSeconds()
     guard idle>=0,idle+0.1>=previousIdle else {throw NSError(domain:"UnexpectedHostInput",code:1)}
     previousIdle=idle;nextIdleCheck=ProcessInfo.processInfo.systemUptime+0.5
    }
    guard foreground() else {throw NSError(domain:"OwnedTargetLostFocus",code:1)}
    _ = try await tap("progress")
    guard lastAppAck["priorPhase"] as? String == "recording" else {throw NSError(domain:"RecorderDidNotStart",code:1)}
    if lastAppAck["fixtureFinished"] as? Bool == true {break}
    guard ProcessInfo.processInfo.systemUptime<playbackDeadline else {throw NSError(domain:"FixtureInjectionTimeout",code:1)}
    try await Task.sleep(for:.milliseconds(20))
   }
   try await Task.sleep(for:.milliseconds(150))
   let stop=try await tap("stop");let deadline=ProcessInfo.processInfo.systemUptime+180
   while !cancelRequested,pasteRow==nil,ProcessInfo.processInfo.systemUptime<deadline {try await Task.sleep(for:.milliseconds(5))}
   guard var row=pasteRow,let received=row["receivedUptime"] as? Double,let result=row["text"] as? String else{throw NSError(domain:"ReceiptTimeout",code:1)}
   diagnostic=["sourcePID":row["eventSourcePID"] ?? -1,"expectedPID":expectedAppPID,"appGeneration":row["appClipboardGeneration"] ?? -1,"boardGeneration":row["clipboardChangeCount"] ?? -1,"receivedCharacters":result.count]
   if row["eventSourcePID"] as? Int64 == Int64(expectedAppPID),
      row["appClipboardGeneration"] as? Int64 == Int64(row["clipboardChangeCount"] as? Int ?? -1) {
    ownedGeneration=row["clipboardChangeCount"] as? Int
   }
   func normalize(_ s:String)->String {s.lowercased().replacingOccurrences(of:"[^a-z0-9]",with:"",options:.regularExpression)}
   guard row["eventSourcePID"] as? Int64 == Int64(expectedAppPID),
         row["appClipboardGeneration"] as? Int64 == Int64(row["clipboardChangeCount"] as? Int ?? -1),
         received>stop,result.count >= Int(command["minimumCharacters"] ?? "50")!,(row["clipboardChangeCount"] as? Int ?? -1)>before,result==row["clipboardText"] as? String,normalize(result).contains(normalize(nonce)),!result.contains("AUDIT_SENTINEL_") else{throw NSError(domain:"GenerationGuard",code:1)}
   ownedGeneration=row["clipboardChangeCount"] as? Int
   row["loadedFixtureSHA256"]=lastAppAck["fixtureSHA256"]
   row["expectedAppPID"]=expectedAppPID
   row["captureGeneration"]=lastAppAck["captureGeneration"];row["inputSampleRate"]=lastAppAck["inputSampleRate"];row["framesInjected"]=lastAppAck["framesInjected"]
   row["captureReadyMilliseconds"]=captureReadyMilliseconds
   row["stopUptime"]=stop;row["milliseconds"]=(received-stop)*1000;row["clipboardBefore"]=before;row["expectedAudioNonce"]=nonce;row["guarded"]=true
   row["fixtureSHA256"]=SHA256.hash(data:try Data(contentsOf:URL(fileURLWithPath:path))).map{String(format:"%02x",$0)}.joined()
   row["clipboardRestored"]=restore();row["done"]=true
   emit(row)
  } catch {
   // On blocked delivery, obtain the app's exact write generation without reading any text.
   _ = try? await tap("abort")
   for _ in 0..<3_600 {
    guard let _ = try? await tap("progress") else {break}
    if lastAppAck["priorPhase"] as? String == "ready" {break}
    try? await Task.sleep(for:.milliseconds(50))
   }
   if let generation=lastAppAck["appClipboardGeneration"] as? Int, generation==board.changeCount {
    ownedGeneration=generation
   }
   // Restore only a generation owned by this fixture; never replay over a newer copy.
   let restored=restore();emit(["error":String(describing:error),"clipboardRestored":restored,"diagnostic":diagnostic])
  }
 }
}
app.run()
withExtendedLifetime(timer){}

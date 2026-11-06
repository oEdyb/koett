import AppKit
import ApplicationServices
import AVFoundation
import CoreAudio
import Darwin

@main struct GuestInput {
 static func main() throws {
  guard ProcessInfo.processInfo.environment["KOETT_AUDIT_GUEST"] == "1", NSUserName() == "admin", NSHomeDirectory().hasPrefix("/Users/admin") else {
   fputs("Guest-only tool: refused to run outside the disposable admin test account.\n",stderr);exit(2)
  }
  let args=Array(CommandLine.arguments.dropFirst())
  guard let command=args.first else { exit(2) }
  func output(_ value:Any) throws {print(String(decoding:try JSONSerialization.data(withJSONObject:value,options:[.sortedKeys]),as:UTF8.self))}
  let source=CGEventSource(stateID:.combinedSessionState)
  func key(_ code:CGKeyCode,_ down:Bool,_ flags:CGEventFlags,_ modifier:Bool=false) {
   let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!
   event.flags=flags
   if modifier {event.type = .flagsChanged}
   event.post(tap:.cghidEventTap)
  }
  switch command {
  case "windows":
   let app=NSRunningApplication.runningApplications(withBundleIdentifier:args[1]).first
   let windows=CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements],kCGNullWindowID) as? [[String:Any]] ?? []
   try output(windows.filter { ($0[kCGWindowOwnerPID as String] as? Int32)==app?.processIdentifier }.map { ["name":$0[kCGWindowName as String] ?? "","layer":$0[kCGWindowLayer as String] ?? -1,"bounds":$0[kCGWindowBounds as String] ?? [:]] })
  case "screens":
   try output(NSScreen.screens.map { ["x":$0.frame.minX,"y":$0.frame.minY,"width":$0.frame.width,"height":$0.frame.height,"scale":$0.backingScaleFactor] })
  case "ui", "press":
   guard let app=NSRunningApplication.runningApplications(withBundleIdentifier:args[1]).first else { exit(4) }
   let root=AXUIElementCreateApplication(app.processIdentifier)
   AXUIElementSetMessagingTimeout(root,0.5)
   var rows:[[String:Any]]=[];var pressed=false
   func attribute(_ element:AXUIElement,_ name:String)->CFTypeRef? {
    var value:CFTypeRef?;guard AXUIElementCopyAttributeValue(element,name as CFString,&value) == .success else{return nil};return value
   }
   func visit(_ element:AXUIElement,_ depth:Int) {
    guard depth<12,rows.count<400 else{return}
    let title=attribute(element,kAXTitleAttribute) as? String ?? ""
    let description=attribute(element,kAXDescriptionAttribute) as? String ?? ""
    let role=attribute(element,kAXRoleAttribute) as? String ?? ""
    rows.append(["depth":depth,"role":role,"title":title,"description":description])
    if command=="press",!pressed,(title==args[2] || description==args[2]) {
     pressed=AXUIElementPerformAction(element,kAXPressAction as CFString) == .success
    }
    for child in attribute(element,kAXChildrenAttribute) as? [AXUIElement] ?? [] {visit(child,depth+1)}
   }
   visit(root,0)
   if command=="press" {try output(["pressed":pressed]);if !pressed {exit(5)}} else {try output(rows)}
  case "permissions":
   try output(["accessibility":AXIsProcessTrusted(),"postEvent":CGPreflightPostEventAccess(),"screen":CGPreflightScreenCaptureAccess(),"microphone":AVCaptureDevice.authorizationStatus(for:.audio).rawValue])
  case "key":
   let code=CGKeyCode(args[1])!;let down=args[2]=="down";let flags=CGEventFlags(rawValue:UInt64(args[3])!)
   key(code,down,flags,[55,56,58,59,60,61,62].contains(Int(code)))
  case "stroke":
   let code=CGKeyCode(args[1])!;let flags=CGEventFlags(rawValue:UInt64(args[2])!)
   key(code,true,flags);usleep(40000);key(code,false,flags)
  case "scroll":
   let event=CGEvent(scrollWheelEvent2Source:source,units:.pixel,wheelCount:1,wheel1:Int32(args[1])!,wheel2:0,wheel3:0)!
   event.post(tap:.cghidEventTap)
  case "tap-modifier":
   let code=CGKeyCode(args[1])!,flags=CGEventFlags(rawValue:UInt64(args[2])!)
   key(code,true,flags,true);usleep(150000);key(code,false,[],true);usleep(150000)
  case "drag", "sketch", "pin":
   let start=CGPoint(x:Double(args[1])!,y:Double(args[2])!)
   let end=CGPoint(x:Double(args[3])!,y:Double(args[4])!)
   let flags=CGEventFlags(rawValue:UInt64(args[5])!)
   if ["sketch","pin"].contains(command) {key(56,true,flags,true);usleep(350000)}
   let down=CGEvent(mouseEventSource:source,mouseType:.leftMouseDown,mouseCursorPosition:start,mouseButton:.left)!
   down.flags=flags;down.post(tap:.cghidEventTap);usleep(50000)
   for step in (command=="pin" ? [] : Array(1...20)) {
    let t=Double(step)/20
    let point=CGPoint(x:start.x+(end.x-start.x)*t,y:start.y+(end.y-start.y)*t)
    let event=CGEvent(mouseEventSource:source,mouseType:.leftMouseDragged,mouseCursorPosition:point,mouseButton:.left)!
    event.flags=flags;event.post(tap:.cghidEventTap);usleep(25000)
   }
   let up=CGEvent(mouseEventSource:source,mouseType:.leftMouseUp,mouseCursorPosition:end,mouseButton:.left)!
   up.flags=flags;up.post(tap:.cghidEventTap);usleep(100000)
   if ["sketch","pin"].contains(command) {key(56,false,[],true);usleep(350000)}
  case "move", "click":
   let point=CGPoint(x:Double(args[1])!,y:Double(args[2])!);let flags=CGEventFlags(rawValue:UInt64(args.count>3 ? args[3]:"0")!)
   let move=CGEvent(mouseEventSource:source,mouseType:.mouseMoved,mouseCursorPosition:point,mouseButton:.left)!
   move.flags=flags;move.post(tap:.cghidEventTap)
   if command=="click" {
    usleep(30000)
    for type in [CGEventType.leftMouseDown,.leftMouseUp] {
     let event=CGEvent(mouseEventSource:source,mouseType:type,mouseCursorPosition:point,mouseButton:.left)!
     event.flags=flags;event.post(tap:.cghidEventTap);usleep(40000)
    }
   }
  case "clipboard":
   let board=NSPasteboard.general
   try output(["itemCount":board.pasteboardItems?.count ?? 0,"items":(board.pasteboardItems ?? []).map { item in item.types.map {["type":$0.rawValue,"bytes":item.data(forType:$0)?.count ?? 0] as [String:Any]} }])
  case "audio":
   var address=AudioObjectPropertyAddress(mSelector:kAudioHardwarePropertyDevices,mScope:kAudioObjectPropertyScopeGlobal,mElement:kAudioObjectPropertyElementMain)
   var size:UInt32=0
   guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject),&address,0,nil,&size)==noErr else {exit(3)}
   var devices=[AudioDeviceID](repeating:0,count:Int(size)/MemoryLayout<AudioDeviceID>.size)
   guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject),&address,0,nil,&size,&devices)==noErr else {exit(3)}
   var rows:[[String:Any]]=[]
   for id in devices {
    var nameValue:Unmanaged<CFString>?
    var nameSize=UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    var property=AudioObjectPropertyAddress(mSelector:kAudioObjectPropertyName,mScope:kAudioObjectPropertyScopeGlobal,mElement:kAudioObjectPropertyElementMain)
    guard AudioObjectGetPropertyData(id,&property,0,nil,&nameSize,&nameValue)==noErr, let nameValue else {continue}
    let name=nameValue.takeRetainedValue() as String
    rows.append(["id":id,"name":name])
    if args.count>1,(name).contains(args[1]) {
     for selector in [kAudioHardwarePropertyDefaultInputDevice,kAudioHardwarePropertyDefaultOutputDevice] {
      var value=id;let bytes=UInt32(MemoryLayout<AudioDeviceID>.size)
      property.mSelector=selector
      guard AudioObjectSetPropertyData(AudioObjectID(kAudioObjectSystemObject),&property,0,nil,bytes,&value)==noErr else {exit(3)}
     }
    }
   }
   try output(rows)
  default:exit(2)
  }
 }
}

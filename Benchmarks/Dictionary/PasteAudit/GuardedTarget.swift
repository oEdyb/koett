import AppKit
import Foundation
final class Target: NSTextView {
 override func paste(_ sender: Any?) {
  super.paste(sender)
  let received = ProcessInfo.processInfo.systemUptime
  let requestData = try! Data(contentsOf: URL(fileURLWithPath: "/tmp/koett-paste-request.json"))
  let request = try! JSONSerialization.jsonObject(with: requestData) as! [String: Any]
  let board = NSPasteboard.general
  let row: [String: Any] = [
   "receivedUptime": received, "text": string,
   "requestID": request["requestID"]!,
   "clipboardChangeCount": board.changeCount,
   "clipboardText": board.string(forType: .string) ?? "",
   "eventSourcePID": NSApp.currentEvent?.cgEvent?.getIntegerValueField(.eventSourceUnixProcessID) ?? -1
  ]
  let data = try! JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
  try! data.write(to: URL(fileURLWithPath: "/tmp/koett-paste-receipt.json"), options: .atomic)
 }
}
guard NSUserName() == "admin", NSHomeDirectory().hasPrefix("/Users/admin") else { exit(2) }
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let menu=NSMenu()
let editItem=NSMenuItem()
let edit=NSMenu(title:"Edit")
edit.addItem(withTitle:"Select All",action:#selector(NSText.selectAll(_:)),keyEquivalent:"a")
edit.addItem(withTitle:"Copy",action:#selector(NSText.copy(_:)),keyEquivalent:"c")
edit.addItem(withTitle:"Paste",action:#selector(NSText.paste(_:)),keyEquivalent:"v")
editItem.submenu=edit
menu.addItem(editItem)
app.mainMenu=menu
let window = NSWindow(contentRect: NSRect(x:100,y:100,width:600,height:300),styleMask:[.titled,.closable,.resizable],backing:.buffered,defer:false)
window.title="Koett synthetic paste target"
let text=Target(frame:window.contentView!.bounds)
window.contentView=text
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(text)
app.activate(ignoringOtherApps:true)
app.run()

import AppKit

guard NSUserName() == "admin", NSHomeDirectory() == "/Users/admin" else { exit(2) }
func id(_ screen: NSScreen) -> Int { (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.intValue ?? 0 }
func rect(_ frame: NSRect) -> [String: Double] { ["x": frame.minX, "y": frame.minY, "width": frame.width, "height": frame.height] }
let screens = NSScreen.screens
let result: [String: Any] = ["mainID": NSScreen.main.map(id) as Any? ?? NSNull(), "primaryTop": screens.first?.frame.maxY ?? 0,
    "screens": screens.map { ["id": id($0), "frame": rect($0.frame), "visibleFrame": rect($0.visibleFrame)] }]
print(String(decoding: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), as: UTF8.self))

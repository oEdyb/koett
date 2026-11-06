import Foundation
import CoreGraphics

guard NSUserName() == "admin", NSHomeDirectory() == "/Users/admin", CommandLine.arguments.count == 2,
      let pid = Int32(CommandLine.arguments[1]) else { exit(2) }
var count: UInt32 = 0
guard CGGetEventTapList(0, nil, &count) == .success else { exit(3) }
var taps = [CGEventTapInformation](repeating: CGEventTapInformation(), count: Int(count))
guard CGGetEventTapList(count, &taps, &count) == .success else { exit(4) }
let records = taps.prefix(Int(count)).filter { $0.tappingProcess == pid }.map {
    ["pid": Int($0.tappingProcess), "mask": $0.eventsOfInterest, "enabled": $0.enabled,
     "options": $0.options.rawValue, "point": $0.tapPoint.rawValue] as [String: Any]
}
let data = try JSONSerialization.data(withJSONObject: records, options: [.sortedKeys])
print(String(decoding: data, as: UTF8.self))

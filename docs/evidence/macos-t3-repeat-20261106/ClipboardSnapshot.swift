import AppKit
import CryptoKit
let board=NSPasteboard.general
let items=(board.pasteboardItems ?? []).map { item in
 item.types.map { type -> [String:Any] in
  let data=item.data(forType:type) ?? Data()
  return ["type":type.rawValue,"bytes":data.count,"sha256":SHA256.hash(data:data).map {String(format:"%02x",$0)}.joined()]
 }
}
let result:[String:Any]=["changeCount":board.changeCount,"itemCount":items.count,"items":items]
print(String(decoding:try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys]),as:UTF8.self))

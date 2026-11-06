import AudioToolbox
import AVFoundation
import CryptoKit
import Foundation
@_silgen_name("KoettAuditSetFixture") func auditSetFixture(_ samples:UnsafePointer<Float>,_ count:UInt64,_ rate:Double)
@_silgen_name("KoettAuditFill") func auditFill(_ data:UnsafeMutablePointer<AudioBufferList>,_ frames:UInt32,_ rate:Double)
@_silgen_name("KoettAuditFixtureFinished") func auditFinished() -> Int32
@_silgen_name("KoettAuditFixtureFrames") func auditFrames() -> UInt64
@_silgen_name("KoettAuditInstallRecorderTap") func auditInstallTap(_ recorder:AnyObject) -> Int32
@_silgen_name("KoettAuditFixtureInputRate") func auditInputRate() -> Double
@MainActor enum AuditFixture {
 static var generation=0
 static var sha=""
 static func prepare(_ path:String) throws {
  let url=URL(fileURLWithPath:path);let file=try AVAudioFile(forReading:url)
  guard file.processingFormat.commonFormat == .pcmFormatFloat32,file.processingFormat.channelCount==1,file.processingFormat.sampleRate==16000 else {throw NSError(domain:"FixtureFormat",code:1)}
  let buffer=AVAudioPCMBuffer(pcmFormat:file.processingFormat,frameCapacity:AVAudioFrameCount(file.length))!
  try file.read(into:buffer)
  auditSetFixture(buffer.floatChannelData![0],UInt64(buffer.frameLength),16000)
  generation += 1
  sha=SHA256.hash(data:try Data(contentsOf:url)).map {String(format:"%02x",$0)}.joined()
 }
}

final class AuditDefaults: UserDefaults, @unchecked Sendable {
 private let lock=NSLock()
 private var values:[String:Any]=[:]
 init() {super.init(suiteName:"com.olledyberg.Koett.PerformanceAudit20261105."+UUID().uuidString)!}
 override func object(forKey key:String)->Any? {lock.withLock {values[key]}}
 override func set(_ value:Any?,forKey key:String) {lock.withLock {values[key]=value}}
 override func set(_ value:Bool,forKey key:String) {set(value as Any,forKey:key)}
 override func set(_ value:Int,forKey key:String) {set(value as Any,forKey:key)}
 override func set(_ value:Float,forKey key:String) {set(value as Any,forKey:key)}
 override func set(_ value:Double,forKey key:String) {set(value as Any,forKey:key)}
 override func set(_ value:URL?,forKey key:String) {set(value.map {$0 as Any},forKey:key)}
 override func removeObject(forKey key:String) {_ = lock.withLock {values.removeValue(forKey:key)}}
 override func register(defaults:[String:Any]) {lock.withLock {values.merge(defaults){current,_ in current}}}
 override func bool(forKey key:String)->Bool {object(forKey:key) as? Bool ?? false}
 override func string(forKey key:String)->String? {object(forKey:key) as? String}
 override func data(forKey key:String)->Data? {object(forKey:key) as? Data}
 override func integer(forKey key:String)->Int {object(forKey:key) as? Int ?? 0}
 override func float(forKey key:String)->Float {object(forKey:key) as? Float ?? 0}
 override func double(forKey key:String)->Double {object(forKey:key) as? Double ?? 0}
 override func array(forKey key:String)->[Any]? {object(forKey:key) as? [Any]}
 override func stringArray(forKey key:String)->[String]? {object(forKey:key) as? [String]}
 override func dictionary(forKey key:String)->[String:Any]? {object(forKey:key) as? [String:Any]}
 override func dictionaryRepresentation()->[String:Any] {lock.withLock {values}}
 override func synchronize()->Bool {true}
}

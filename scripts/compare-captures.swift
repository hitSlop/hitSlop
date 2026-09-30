// Compare decoded pixels, ignoring compression and metadata.
// swiftc -O scripts/compare-captures.swift -o /tmp/compare-captures
// /tmp/compare-captures BEFORE_DIRECTORY AFTER_DIRECTORY
import Foundation
import CoreGraphics
import ImageIO
func pixels(_ url: URL) -> (Int, Int, Data)? {
  guard let source = CGImageSourceCreateWithURL(url as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {return nil}
  var data = Data(count: image.width * image.height * 4)
  data.withUnsafeMutableBytes { bytes in
    let context = CGContext(data: bytes.baseAddress, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
  }
  return (image.width, image.height, data)
}
guard CommandLine.arguments.count == 3 else { fatalError("Expected before and after directories") }
var failures = 0
let before = URL(fileURLWithPath: CommandLine.arguments[1])
let after = URL(fileURLWithPath: CommandLine.arguments[2])
let fm = FileManager.default
for path in (fm.enumerator(atPath: before.path)?.allObjects as? [String] ?? []).filter({$0.hasSuffix(".png")}).sorted() {
  guard let a = pixels(before.appendingPathComponent(path)), let b = pixels(after.appendingPathComponent(path)) else { print("MISSING \(path)"); failures += 1; continue }
  let different = a.0 == b.0 && a.1 == b.1 ? zip(a.2,b.2).filter{$0 != $1}.count : -1
  var maxDelta = 0
  if different > 0 { for (x, y) in zip(a.2, b.2) { maxDelta = max(maxDelta, abs(Int(x) - Int(y))) } }
  if different != 0 { failures += 1 }
  print("\(different == 0 ? "MATCH" : "DIFF") \(path) \(a.0)x\(a.1) -> \(b.0)x\(b.1) channels=\(different) maxDelta=\(maxDelta)")
}
exit(failures == 0 ? 0 : 1)

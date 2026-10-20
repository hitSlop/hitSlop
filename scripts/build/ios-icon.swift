// iOS supplies its own icon mask. Remove the Mac icon's transparent outer margin
// and flatten onto its white bezel so SpringBoard never composites black padding.
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let source = URL(fileURLWithPath: CommandLine.arguments[1])
let destination = URL(fileURLWithPath: CommandLine.arguments[2])
guard let input = CGImageSourceCreateWithURL(source as CFURL, nil),
  let image = CGImageSourceCreateImageAtIndex(input, 0, nil)
else { fatalError("Cannot read the app icon") }
let width = image.width
let height = image.height
let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!
let pixels = CGContext(
  data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
  space: colorSpace,
  bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue)!
pixels.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
let bytes = pixels.data!.assumingMemoryBound(to: UInt8.self)
var left = width, right = 0, top = height, bottom = 0
for y in 0..<height {
  for x in 0..<width where bytes[(y * width + x) * 4 + 3] >= 128 {
    left = min(left, x)
    right = max(right, x)
    top = min(top, y)
    bottom = max(bottom, y)
  }
}
guard right > left, bottom > top,
  let cropped = image.cropping(to: CGRect(x: left, y: top, width: right - left + 1, height: bottom - top + 1))
else { fatalError("The app icon is empty") }
let output = CGContext(
  data: nil, width: 1024, height: 1024, bitsPerComponent: 8, bytesPerRow: 1024 * 4,
  space: colorSpace,
  bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.noneSkipLast.rawValue)!
let canvas = CGRect(x: 0, y: 0, width: 1024, height: 1024)
output.setFillColor(CGColor(gray: 1, alpha: 1))
output.fill(canvas)
output.interpolationQuality = .high
output.draw(cropped, in: canvas)
guard let result = output.makeImage(),
  let png = CGImageDestinationCreateWithURL(destination as CFURL, UTType.png.identifier as CFString, 1, nil)
else { fatalError("Cannot write the iOS app icon") }
CGImageDestinationAddImage(png, result, nil)
guard CGImageDestinationFinalize(png) else { fatalError("Cannot finish the iOS app icon") }

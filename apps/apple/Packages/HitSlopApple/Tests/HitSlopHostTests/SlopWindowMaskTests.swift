import AppKit
import CoreGraphics
import Foundation
import HitSlopCore
import ImageIO
import QuartzCore
import Testing
import HitSlopTestSupport
@testable import HitSlopHost

@Test @MainActor func imageMaskHitTestingUsesVisualTopAndTenPercentAlpha() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(file: SlopFile(url: root))
    let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
    #expect(mask.contains(CGPoint(x: 120, y: 170), in: bounds))
    #expect(!mask.contains(CGPoint(x: 120, y: 10), in: bounds))

    let layer = mask.makeLayer()
    mask.update(layer, bounds: bounds)
    var rendered = [UInt8](repeating: 0, count: 240 * 180 * 4)
    let context = CGContext(data: &rendered, width: 240, height: 180, bitsPerComponent: 8, bytesPerRow: 240 * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)!
    layer.render(in: context)
    let alphaAtTop = rendered[(170 * 240 + 120) * 4 + 3]
    let alphaAtBottom = rendered[(10 * 240 + 120) * 4 + 3]
    #expect(alphaAtTop > 25)
    #expect(alphaAtBottom <= 25)
}

@Test @MainActor func transparentGeometryInstallsAClearBacking() throws {
    let stage = try Fixtures.minimalStage(slug: "transparent", manifest: ["presentation": ["width": 240, "height": 180, "background": "transparent"]])
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let mask = try SlopWindowMask(file: SlopFile(url: root))
    let layer = CALayer()
    mask.installBacking(on: layer)
    #expect(layer.backgroundColor?.alpha == 0)
}

private func maskedFixture(alpha: (Int, Int) -> UInt8 = { _, y in y < 90 ? 255 : 0 }) throws -> URL {
    let stage = try Fixtures.minimalStage(slug: "asymmetric", manifest: ["presentation": ["width": 240, "height": 180, "skin": "assets/window-mask.png"]])
    let width = 240, height = 180
    var pixels = [UInt8](repeating: 255, count: width * height * 4)
    for y in 0..<height {
        for x in 0..<width { pixels[(y * width + x) * 4 + 3] = alpha(x, y) }
    }
    let data = Data(pixels)
    guard let provider = CGDataProvider(data: data as CFData),
          let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithURL(stage.appendingPathComponent("assets/window-mask.png") as CFURL, "public.png" as CFString, 1, nil) else {
        throw SlopError.invalid("could not create asymmetric test mask")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw SlopError.invalid("could not write asymmetric test mask") }
    return try Fixtures.document(stage: stage, at: Fixtures.folder().appendingPathComponent("asymmetric.slop"))
}

@Test @MainActor func ringMaskLetsClicksFallThroughItsTransparentHole() throws {
    let root = try maskedFixture { x, y in let r = hypot(Double(x - 120), Double(y - 90)); return r >= 40 && r <= 80 ? 255 : 0 }
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(file: SlopFile(url: root))
    let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
    for point in [CGPoint(x: 60,y: 90),CGPoint(x: 180,y: 90),CGPoint(x: 120,y: 30),CGPoint(x: 120,y: 150)] { #expect(mask.contains(point, in: bounds)) }
    #expect(!mask.contains(CGPoint(x: 120,y: 90), in: bounds))
}
@Test @MainActor func imageMaskTreatsAlphaAtTheThresholdAsOpaqueAndJustBelowAsClickThrough() throws {
    let root = try maskedFixture { x, _ in x < 120 ? 26 : 25 }
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(file: SlopFile(url: root))
    #expect(mask.contains(CGPoint(x: 60,y: 90), in: CGRect(x: 0,y: 0,width: 240,height: 180)))
    #expect(!mask.contains(CGPoint(x: 180,y: 90), in: CGRect(x: 0,y: 0,width: 240,height: 180)))
}
@Test @MainActor func imageMaskHitTestingIsIndependentOfBackingScale() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(file: SlopFile(url: root))
    for point in [CGPoint(x: 120,y: 30),CGPoint(x: 120,y: 150)] {
        #expect(mask.contains(point, in: CGRect(x: 0,y: 0,width: 240,height: 180)) == mask.contains(CGPoint(x: point.x * 2,y: point.y * 2), in: CGRect(x: 0,y: 0,width: 480,height: 360)))
    }
}
@Test @MainActor func skinnedWindowsAreNeverResizableAndDropTheResizableStyleMask() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let file = try SlopFile(url: root)
    #expect(!file.isResizable)
    #expect(!slopDocumentWindowStyleMask(resizable: file.isResizable).contains(.resizable))
}

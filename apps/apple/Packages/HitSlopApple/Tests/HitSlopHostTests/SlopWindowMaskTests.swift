import AppKit
import CoreGraphics
import Foundation
import HitSlopCore
import ImageIO
import QuartzCore
import Testing
@testable import HitSlopHost

@Test @MainActor func imageMaskHitTestingUsesVisualTopAndTenPercentAlpha() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(package: SlopPackage(rootURL: root))
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
    let parent = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-transparent-\(UUID().uuidString)", isDirectory: true)
    let root = parent.appendingPathComponent("transparent.slop", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: parent) }
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    try Data(#"{"format":1,"root":{"kind":"object","properties":{}}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
    let manifest = #"{"runtime":"hitslop-v1","$schema":"https://api.hitslop.com/schemas/v1/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"transparent","title":"Transparent","description":"Tests transparent geometry.","categories":["utilities"],"presentation":{"width":240,"height":180,"background":"transparent"}}"#
    try Data(manifest.utf8).write(to: root.appendingPathComponent("manifest.json"))
    try writeCanonicalDocumentSkill(to: root)
    let mask = try SlopWindowMask(package: SlopPackage(rootURL: root))
    let layer = CALayer()
    mask.installBacking(on: layer)
    #expect(layer.backgroundColor?.alpha == 0)
}

private func maskedFixture(alpha: (Int, Int) -> UInt8 = { _, y in y < 90 ? 255 : 0 }) throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-host-mask-\(UUID().uuidString)", isDirectory: true)
    let root = directory.appendingPathComponent("asymmetric.slop", isDirectory: true)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    try Data(#"{"format":1,"root":{"kind":"object","properties":{}}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
    let manifest = #"{"runtime":"hitslop-v1","$schema":"https://api.hitslop.com/schemas/v1/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"asymmetric","title":"Asymmetric","description":"Tests image mask orientation.","categories":["utilities"],"presentation":{"width":240,"height":180,"skin":"assets/window-mask.png"}}"#
    try Data(manifest.utf8).write(to: root.appendingPathComponent("manifest.json"))
    try writeCanonicalDocumentSkill(to: root)

    let width = 240, height = 180
    var pixels = [UInt8](repeating: 255, count: width * height * 4)
    for y in 0..<height {
        for x in 0..<width { pixels[(y * width + x) * 4 + 3] = alpha(x, y) }
    }
    let data = Data(pixels)
    guard let provider = CGDataProvider(data: data as CFData),
          let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithURL(root.appendingPathComponent("assets/window-mask.png") as CFURL, "public.png" as CFString, 1, nil) else {
        throw SlopPackageError.invalid("could not create asymmetric test mask")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw SlopPackageError.invalid("could not write asymmetric test mask") }
    return root
}

private func writeCanonicalDocumentSkill(to root: URL) throws {
    let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    try FileManager.default.createDirectory(at: skill.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../../../packages/cli/skills/hitslop-document/SKILL.md").standardizedFileURL).write(to: skill)
}

@Test @MainActor func ringMaskLetsClicksFallThroughItsTransparentHole() throws {
    let root = try maskedFixture { x, y in let r = hypot(Double(x - 120), Double(y - 90)); return r >= 40 && r <= 80 ? 255 : 0 }
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(package: SlopPackage(rootURL: root))
    let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
    for point in [CGPoint(x: 60,y: 90),CGPoint(x: 180,y: 90),CGPoint(x: 120,y: 30),CGPoint(x: 120,y: 150)] { #expect(mask.contains(point, in: bounds)) }
    #expect(!mask.contains(CGPoint(x: 120,y: 90), in: bounds))
}
@Test @MainActor func imageMaskTreatsAlphaAtTheThresholdAsOpaqueAndJustBelowAsClickThrough() throws {
    let root = try maskedFixture { x, _ in x < 120 ? 26 : 25 }
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(package: SlopPackage(rootURL: root))
    #expect(mask.contains(CGPoint(x: 60,y: 90), in: CGRect(x: 0,y: 0,width: 240,height: 180)))
    #expect(!mask.contains(CGPoint(x: 180,y: 90), in: CGRect(x: 0,y: 0,width: 240,height: 180)))
}
@Test @MainActor func imageMaskHitTestingIsIndependentOfBackingScale() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let mask = try SlopWindowMask(package: SlopPackage(rootURL: root))
    for point in [CGPoint(x: 120,y: 30),CGPoint(x: 120,y: 150)] {
        #expect(mask.contains(point, in: CGRect(x: 0,y: 0,width: 240,height: 180)) == mask.contains(CGPoint(x: point.x * 2,y: point.y * 2), in: CGRect(x: 0,y: 0,width: 480,height: 360)))
    }
}
@Test @MainActor func skinnedWindowsAreNeverResizableAndDropTheResizableStyleMask() throws {
    let root = try maskedFixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let package = try SlopPackage(rootURL: root)
    #expect(!package.isResizable)
    #expect(!slopDocumentWindowStyleMask(resizable: package.isResizable).contains(.resizable))
}

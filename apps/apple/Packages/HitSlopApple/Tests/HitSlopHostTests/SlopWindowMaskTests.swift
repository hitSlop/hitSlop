import AppKit
import CoreGraphics
import Foundation
import HitSlopCore
import HitSlopDocument
import HitSlopTestSupport
import ImageIO
import QuartzCore
import Testing
import WebKit

@testable import HitSlopHost

@Test @MainActor func imageMaskHitTestingUsesVisualTopAndTenPercentAlpha() throws {
  let root = try maskedFixture()
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
  #expect(mask.contains(CGPoint(x: 120, y: 170), in: bounds))
  #expect(!mask.contains(CGPoint(x: 120, y: 10), in: bounds))

  let layer = mask.makeLayer()
  mask.update(layer, bounds: bounds)
  var rendered = [UInt8](repeating: 0, count: 240 * 180 * 4)
  let context = CGContext(
    data: &rendered, width: 240, height: 180, bitsPerComponent: 8, bytesPerRow: 240 * 4,
    space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)!
  layer.render(in: context)
  let alphaAtTop = rendered[(170 * 240 + 120) * 4 + 3]
  let alphaAtBottom = rendered[(10 * 240 + 120) * 4 + 3]
  #expect(alphaAtTop > 25)
  #expect(alphaAtBottom <= 25)
}

@Test @MainActor func transparentGeometryInstallsAClearBacking() throws {
  let stage = try Fixtures.minimalStage(
    slug: "transparent", fields: ["window": ["width": 240, "height": 180, "background": "transparent"]])
  let root = try Fixtures.document(stage: stage)
  defer { try? FileManager.default.removeItem(at: root) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  let layer = CALayer()
  mask.installBacking(on: layer)
  #expect(layer.backgroundColor?.alpha == 0)
}

private func maskedFixture(scale: Int = 1, alpha: (Int, Int) -> UInt8 = { _, y in y < 90 ? 255 : 0 }) throws -> URL {
  let stage = try Fixtures.minimalStage(
    slug: "asymmetric", fields: ["window": ["width": 240, "height": 180]])
  try Fixtures.png(width: 240 * scale, height: 180 * scale, alpha: { x, y in alpha(x / scale, y / scale) }).write(
    to: stage.appendingPathComponent("assets/window-mask.png"))
  try Fixtures.addSkin(stage, path: "assets/window-mask.png")
  return try Fixtures.document(stage: stage, at: Fixtures.folder().appendingPathComponent("asymmetric.slop"))
}

@Test @MainActor func doubleResolutionSkinKeepsItsPointSizeAndHitRegion() throws {
  let root = try maskedFixture(scale: 2)
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
  let layer = mask.makeLayer()
  mask.update(layer, bounds: bounds)
  #expect(layer.contentsScale == 2)
  #expect(layer.bounds.size == bounds.size)
  let backing = CALayer()
  mask.installBacking(on: backing)
  #expect(backing.contentsScale == 2)
  #expect(mask.contains(CGPoint(x: 120, y: 170), in: bounds))
  #expect(!mask.contains(CGPoint(x: 120, y: 10), in: bounds))
}

@Test @MainActor func ringMaskLetsClicksFallThroughItsTransparentHole() throws {
  let root = try maskedFixture { x, y in
    let r = hypot(Double(x - 120), Double(y - 90))
    return r >= 40 && r <= 80 ? 255 : 0
  }
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  let bounds = CGRect(x: 0, y: 0, width: 240, height: 180)
  for point in [CGPoint(x: 60, y: 90), CGPoint(x: 180, y: 90), CGPoint(x: 120, y: 30), CGPoint(x: 120, y: 150)] {
    #expect(mask.contains(point, in: bounds))
  }
  #expect(!mask.contains(CGPoint(x: 120, y: 90), in: bounds))
}
@Test @MainActor func imageMaskTreatsAlphaAtTheThresholdAsOpaqueAndJustBelowAsClickThrough() throws {
  let root = try maskedFixture { x, _ in x < 120 ? 26 : 25 }
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  #expect(mask.contains(CGPoint(x: 60, y: 90), in: CGRect(x: 0, y: 0, width: 240, height: 180)))
  #expect(!mask.contains(CGPoint(x: 180, y: 90), in: CGRect(x: 0, y: 0, width: 240, height: 180)))
}
@Test @MainActor func imageMaskHitTestingIsIndependentOfBackingScale() throws {
  let root = try maskedFixture()
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let mask = SlopWindowMask(file: try SlopFile(url: root))
  for point in [CGPoint(x: 120, y: 30), CGPoint(x: 120, y: 150)] {
    #expect(
      mask.contains(point, in: CGRect(x: 0, y: 0, width: 240, height: 180))
        == mask.contains(CGPoint(x: point.x * 2, y: point.y * 2), in: CGRect(x: 0, y: 0, width: 480, height: 360)))
  }
}
@Test @MainActor func skinnedWindowsAreNeverResizableAndDropTheResizableStyleMask() throws {
  let root = try maskedFixture()
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let file = try SlopFile(url: root)
  #expect(!file.isResizable)
  #expect(!slopDocumentWindowStyleMask(resizable: file.isResizable).contains(.resizable))
}

extension HostTests {
  @Test(.nightly) @MainActor func glassWindowsFrostBehindThePage() async throws {
    let stage = try Fixtures.minimalStage(
      slug: "glass", fields: ["window": ["width": 240, "height": 180, "background": "glass"]])
    let root = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.showWindow(nil)
    await controller.waitForPresentation()
    let window = try #require(controller.window)
    let container = try #require(window.contentView as? ShapedView)
    container.layoutSubtreeIfNeeded()
    let glass = try #require(container.glass)
    #expect(window.hasShadow, "the frosted surface casts the window's shadow")
    #expect(glass.blendingMode == .behindWindow && glass.state == .active && glass.maskImage != nil)
    #expect(glass.appearance?.name == .aqua, "the frost stays light, like the palette, in dark mode")
    #expect(container.layer?.backgroundColor?.alpha == 0)
    #expect(!WebViewBackground.get(controller.session.webView), "the page draws over the glass")
    #expect(container.subviews.first === glass)
    // A replaced page goes above the glass, not below every view.
    let replacement = WKWebView(frame: .zero)
    controller.pageSession(controller.session, didReplace: replacement)
    let order = container.subviews
    #expect(order.firstIndex(of: replacement) == (order.firstIndex(of: glass) ?? -1) + 1)
    try await controller.closeDocument()
  }
}

import CoreGraphics
import Foundation
import HitSlopCoreBinding

/// Immutable window geometry shared by clipping, pointer membership and raster capture.
/// The manifest shape is parsed once by the core during manifest validation; this type only
/// turns its normalized radii or segments into paths for the current bounds.
public struct SlopSilhouette: @unchecked Sendable {
  private enum Geometry {
    case radii(horizontal: [SilhouetteLength], vertical: [SilhouetteLength])
    case path(CGPath, CGSize)
  }
  private let geometry: Geometry
  public let fillRule: CGPathFillRule

  public init(parsed: WindowSilhouette) {
    switch parsed {
    case .radii(let horizontal, let vertical):
      geometry = .radii(horizontal: horizontal, vertical: vertical)
      fillRule = .winding
    case .path(let segments, let viewBoxWidth, let viewBoxHeight, let evenOdd):
      let path = CGMutablePath()
      for segment in segments {
        switch segment {
        case .move(let x, let y): path.move(to: CGPoint(x: x, y: y))
        case .line(let x, let y): path.addLine(to: CGPoint(x: x, y: y))
        case .cubic(let x1, let y1, let x2, let y2, let x, let y):
          path.addCurve(to: CGPoint(x: x, y: y), control1: CGPoint(x: x1, y: y1), control2: CGPoint(x: x2, y: y2))
        case .close: path.closeSubpath()
        }
      }
      geometry = .path(path.copy()!, CGSize(width: viewBoxWidth, height: viewBoxHeight))
      fillRule = evenOdd ? .evenOdd : .winding
    }
  }

  /// SVG and CSS use y-down coordinates; AppKit uses y-up. Convert in one place.
  public func path(in bounds: CGRect) -> CGPath {
    guard bounds.width > 0, bounds.height > 0, bounds.width.isFinite, bounds.height.isFinite else {
      return CGMutablePath()
    }
    let source: CGPath
    var transform: CGAffineTransform
    switch geometry {
    case .radii(let horizontal, let vertical):
      source = Self.rounded(horizontal: horizontal, vertical: vertical, width: bounds.width, height: bounds.height)
      transform = CGAffineTransform(a: 1, b: 0, c: 0, d: -1, tx: bounds.minX, ty: bounds.maxY)
    case .path(let path, let box):
      source = path
      transform = CGAffineTransform(
        a: bounds.width / box.width, b: 0, c: 0, d: -bounds.height / box.height, tx: bounds.minX, ty: bounds.maxY)
    }
    return source.copy(using: &transform)!
  }

  /// CSS `border-radius` geometry: percentages resolve against the bounds and all radii
  /// shrink by the overlap factor (CSS Backgrounds 3, "Overlapping Curves").
  private static func rounded(
    horizontal: [SilhouetteLength], vertical: [SilhouetteLength], width w: CGFloat, height h: CGFloat
  ) -> CGPath {
    func resolve(_ length: SilhouetteLength, _ extent: CGFloat) -> CGFloat {
      length.percent ? extent * (length.value / 100) : length.value
    }
    var x = horizontal.map { resolve($0, w) }
    var y = vertical.map { resolve($0, h) }
    var factor: CGFloat = 1
    for (length, total) in [(w, x[0] + x[1]), (w, x[3] + x[2]), (h, y[0] + y[3]), (h, y[1] + y[2])] {
      if total > 0 { factor = min(factor, length / total) }
    }
    x = x.map { $0 * factor }
    y = y.map { $0 * factor }
    let p = CGMutablePath()
    let k: CGFloat = 0.5522847498307936
    p.move(to: CGPoint(x: x[0], y: 0))
    p.addLine(to: CGPoint(x: w - x[1], y: 0))
    p.addCurve(
      to: CGPoint(x: w, y: y[1]), control1: CGPoint(x: w - x[1] + k * x[1], y: 0),
      control2: CGPoint(x: w, y: y[1] - k * y[1]))
    p.addLine(to: CGPoint(x: w, y: h - y[2]))
    p.addCurve(
      to: CGPoint(x: w - x[2], y: h), control1: CGPoint(x: w, y: h - y[2] + k * y[2]),
      control2: CGPoint(x: w - x[2] + k * x[2], y: h))
    p.addLine(to: CGPoint(x: x[3], y: h))
    p.addCurve(
      to: CGPoint(x: 0, y: h - y[3]), control1: CGPoint(x: x[3] - k * x[3], y: h),
      control2: CGPoint(x: 0, y: h - y[3] + k * y[3]))
    p.addLine(to: CGPoint(x: 0, y: y[0]))
    p.addCurve(
      to: CGPoint(x: x[0], y: 0), control1: CGPoint(x: 0, y: y[0] - k * y[0]),
      control2: CGPoint(x: x[0] - k * x[0], y: 0))
    p.closeSubpath()
    return p
  }
}

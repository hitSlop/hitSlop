import CoreGraphics
import Foundation
import HitSlopTestSupport
import Testing

@testable import HitSlopCore

private func pathShape(_ path: String, evenOdd: Bool = false, width: Double = 100, height: Double = 100) -> SlopShape {
  .slopPathShape(.init(fillRule: evenOdd ? .evenodd : .nonzero, path: path, viewBox: [width, height]))
}
// Rust owns parsing; these tests exercise native path construction from its output, read
// from a template the core packed.
private func silhouette(shape: SlopShape, width: Int, height: Int) throws -> SlopSilhouette {
  let stage = try Fixtures.minimalStage(
    slug: "shape-lab",
    manifest: [
      "presentation": [
        "width": max(240, width), "height": max(180, height),
        "shape": try JSONSerialization.jsonObject(with: JSONEncoder().encode(shape), options: .fragmentsAllowed),
      ]
    ])
  defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
  return try SlopFile(url: Fixtures.template(stage: stage)).silhouette
}
@Test func silhouettesPreserveOrientationOriginsAndEvenOddHoles() throws {
  let shape = try silhouette(
    shape: pathShape("M0 0H100V100H0Z M60 10H90V40H60Z", evenOdd: true), width: 100, height: 100)
  let bounds = CGRect(x: 20, y: 30, width: 200, height: 300)
  let path = shape.path(in: bounds)
  // Hole at SVG (75,25), not (75,75); scales from top-left into AppKit's y-up bounds.
  #expect(!path.contains(CGPoint(x: 170, y: 255), using: shape.fillRule))
  #expect(path.contains(CGPoint(x: 170, y: 105), using: shape.fillRule))
  #expect(path.contains(CGPoint(x: 40, y: 300), using: shape.fillRule))
  let winding = try silhouette(shape: pathShape("M0 0H100V100H0Z M60 10H90V40H60Z"), width: 100, height: 100)
  #expect(winding.path(in: bounds).contains(CGPoint(x: 170, y: 255), using: winding.fillRule))
}
@Test func radiiApplyCSSOverlapAndPercentagesInLogicalCoordinates() throws {
  let rect = CGRect(x: 0, y: 0, width: 200, height: 100)
  let ellipse = try silhouette(shape: .string("50%"), width: 200, height: 100).path(in: rect)
  #expect(!ellipse.contains(CGPoint(x: 10, y: 10)))
  #expect(ellipse.contains(CGPoint(x: 100, y: 10)))
  let capsule = try silhouette(shape: .string("9999px"), width: 200, height: 100).path(in: rect)
  #expect(capsule.contains(CGPoint(x: 50, y: 1)))
  #expect(!capsule.contains(CGPoint(x: 1, y: 1)))
  let asymmetric = try silhouette(shape: .string("50px 0 0 0"), width: 200, height: 100).path(in: rect)
  #expect(!asymmetric.contains(CGPoint(x: 1, y: 99)))
  #expect(asymmetric.contains(CGPoint(x: 1, y: 1)))
  #expect(asymmetric.contains(CGPoint(x: 199, y: 99)))
}
@Test func arcsRespectSweepRotationAndRadiusCorrection() throws {
  let circle = try silhouette(shape: pathShape("M50 0a50 50 0 1 0 0 100a50 50 0 1 0 0-100Z"), width: 100, height: 100)
    .path(in: CGRect(x: 0, y: 0, width: 100, height: 100))
  #expect(circle.contains(CGPoint(x: 50, y: 50)))
  #expect(!circle.contains(CGPoint(x: 1, y: 1)))
  #expect(abs(circle.boundingBoxOfPath.width - 100) < 0.001)
  let corrected = try silhouette(shape: pathShape("M0 50A1 1 0 0 1 100 50Z"), width: 100, height: 100).path(
    in: CGRect(x: 0, y: 0, width: 100, height: 100))
  #expect(corrected.contains(CGPoint(x: 50, y: 75)))
  #expect(!corrected.contains(CGPoint(x: 50, y: 25)))
  let rotated = try silhouette(
    shape: pathShape("M50 10A40 20 90 1 1 50 90A40 20 90 1 1 50 10Z"), width: 100, height: 100
  ).path(in: CGRect(x: 0, y: 0, width: 100, height: 100))
  #expect(abs(rotated.boundingBoxOfPath.width - 40) < 0.01)
  #expect(abs(rotated.boundingBoxOfPath.height - 80) < 0.01)
}

import AppKit
import Foundation
import HitSlopCore
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

// One parent suite keeps shared AppKit/WebView integration tests serialized.
@Suite(.serialized) struct HostTests {
  func fixture(_ name: String = "quick-checklist") throws -> URL { try Fixtures.native(name) }

  /// The conformance app's document with the platform probe app (tests/abi/probe) in place of
  /// its frozen consumer; disposable documents are safe for host behavior probes. `edit`
  /// changes the build stage before it is packed.
  func contractFixture(edit: (_ stage: URL) throws -> Void = { _ in }) throws -> URL {
    let stage = try Fixtures.stage()
    let app = stage.appendingPathComponent("assets/app.js")
    try FileManager.default.removeItem(at: app)
    try FileManager.default.copyItem(at: Fixtures.repository.appendingPathComponent("tests/abi/probe/app.js"), to: app)
    try edit(stage)
    return try Fixtures.document(stage: stage)
  }

  /// A CLI batch replacing the probe's title.
  func setTitle(_ text: String) throws -> [String: Any] {
    let op = try JSONSerialization.data(withJSONObject: [["type": "set", "path": ["title"], "value": text]])
    return ["ops": String(decoding: op, as: UTF8.self)]
  }
  /// The document's value, as `slop get` prints it.
  @MainActor func savedValue(_ root: URL) async throws -> NSDictionary? {
    let state = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as? [String: Any]
    return (state?["state"] as? [String: Any])?["value"] as? NSDictionary
  }

  /// Runs the helper with `args`, writing `input` to its standard input.
  func cli(_ args: [String], input: Data? = nil) async throws -> (Int32, String, String) {
    let helper = Fixtures.repository.appendingPathComponent("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native")
    return try await Task.detached { try Fixtures.run(helper, args, input: input) }.value
  }
}


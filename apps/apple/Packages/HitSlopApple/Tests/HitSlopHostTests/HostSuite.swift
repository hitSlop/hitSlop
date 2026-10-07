import AppKit
import Foundation
import HitSlopCore
import HitSlopTestSupport
import PDFKit
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

// One parent suite keeps shared AppKit/WebView integration tests serialized.
@Suite(.serialized) struct HostTests {
  func fixture(_ name: String = "quick-checklist") throws -> URL { try Fixtures.native(name) }

  /// The conformance app's document with the platform probe app (tests/abi/probe) in place of
  /// its frozen consumer; disposable documents are safe for host behavior probes. `edit`
  /// changes the build stage before it is packed.
  func contractFixture(edit: (_ stage: URL) throws -> Void = { _ in }) throws -> URL {
    let stage = try Fixtures.stage()
    let app = stage.appendingPathComponent("assets/ui.js")
    try FileManager.default.removeItem(at: app)
    try FileManager.default.copyItem(at: Fixtures.repository.appendingPathComponent("tests/abi/probe/app.js"), to: app)
    try edit(stage)
    try Fixtures.writeApp(String(contentsOf: app, encoding: .utf8), to: stage)
    return try Fixtures.document(stage: stage)
  }

  /// A CLI batch replacing the probe's title.
  func setTitle(_ text: String) throws -> [String: Any] {
    let op = try JSONSerialization.data(withJSONObject: [["type": "set", "path": ["title"], "value": text]])
    return [
      "batch": ["intents": try JSONSerialization.jsonObject(with: Data((String(decoding: op, as: UTF8.self)).utf8))]
    ]
  }
  /// The document's value, as `slop get` prints it.
  @MainActor func savedValue(_ root: URL) async throws -> NSDictionary? {
    let state = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as? [String: Any]
    return state?["value"] as? NSDictionary
  }

  /// Runs a native tool of the debug build in this build's command protocol,
  /// writing `input` to its standard input: the document engine `slop` runs unless `tool`
  /// names the rendering helper.
  func cli(input: Data, tool: String = "slop-engine") async throws -> (Int32, String, String) {
    let binary =
      tool == "slop-engine"
      ? Fixtures.engine
      : Fixtures.repository.appendingPathComponent("apps/apple/Packages/HitSlopApple/.build/debug/\(tool)")
    let named = ["--client-protocol", String(HelperProtocol.version)]
    return try await Task.detached { try Fixtures.run(binary, named, input: input) }.value
  }
}

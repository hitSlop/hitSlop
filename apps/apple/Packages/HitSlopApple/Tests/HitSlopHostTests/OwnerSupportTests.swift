import AppKit
import Foundation
import HitSlopCore
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

// One parent suite keeps shared AppKit/WebView integration tests serialized.
@Suite(.serialized) struct OwnerClientTests {
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

  /// A CLI edit replacing the probe's title.
  func setTitle(_ text: String) throws -> Data {
    try JSONSerialization.data(withJSONObject: ["type": "set", "path": ["title"], "value": text])
  }

  func cli(_ args: [String]) async throws -> (Int32, String, String) {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    return try await Task.detached {
      let process = Process()
      process.executableURL = URL(
        fileURLWithPath: repository
          + "/apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native")
      process.arguments = args
      let stdout = Pipe()
      let stderr = Pipe()
      process.standardOutput = stdout
      process.standardError = stderr
      try process.run()
      let output = stdout.fileHandleForReading.readDataToEndOfFile()
      let error = stderr.fileHandleForReading.readDataToEndOfFile()
      process.waitUntilExit()
      return (
        process.terminationStatus, String(decoding: output, as: UTF8.self),
        String(decoding: error, as: UTF8.self)
      )
    }.value
  }
}

extension OwnerClientTests {
  /// Draft input and dedicated export surface, independent of example UI copy.
  func captureFixture(edit: (_ stage: URL) throws -> Void = { _ in }) throws -> URL { try contractFixture(edit: edit) }
}

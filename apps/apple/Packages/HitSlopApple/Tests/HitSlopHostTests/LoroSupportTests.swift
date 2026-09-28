import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import Testing

@testable import HitSlopHost
@testable import HitSlopDocument

// One parent suite keeps shared AppKit/WebView integration tests serialized.
@Suite(.serialized) struct LoroClientTests {
  func fixture(_ name: String = "quick-checklist") throws -> URL {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.copyItem(
      at: URL(fileURLWithPath: repository + "/generated/v1/native-fixtures/\(name).slop"), to: root)
    return root
  }

  /// The sealed conformance package with the platform probe app (tests/abi/probe) in place of
  /// its frozen consumer; disposable copies are safe for host behavior probes.
  func contractFixture() throws -> URL {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/4-1/document", toPath: root.path)
    let app = root.appendingPathComponent("assets/app.js")
    try FileManager.default.removeItem(at: app)
    try FileManager.default.copyItem(atPath: repository + "/tests/abi/probe/app.js", toPath: app.path)
    return root
  }

  func prefixTitle(_ text: String) throws -> Data {
    try JSONSerialization.data(withJSONObject: [
      "type": "splice", "path": ["title"], "index": 0, "delete": 0, "insert": text,
    ])
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

extension LoroClientTests {
  /// Draft input and dedicated export surface, independent of example UI copy.
  func captureFixture() throws -> URL { try contractFixture() }
}

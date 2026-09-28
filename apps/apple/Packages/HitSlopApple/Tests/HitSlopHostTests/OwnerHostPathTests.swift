import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import Testing
@testable import HitSlopHost
@testable import HitSlopDocument

extension LoroClientTests {
  @Test @MainActor func fixtureDocumentsSurviveHostCLIThemeAndExport() async throws {
    _ = NSApplication.shared
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let fixtures = URL(fileURLWithPath: repository + "/tests/fixtures")
    let entries = try FileManager.default.contentsOfDirectory(at: fixtures, includingPropertiesForKeys: nil)
      .filter { FileManager.default.fileExists(atPath: $0.appendingPathComponent("fixture.json").path) }
    #expect(!entries.isEmpty)
    for fixture in entries {
      let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
      defer { try? FileManager.default.removeItem(at: root) }
      try FileManager.default.copyItem(at: fixture.appendingPathComponent("document"), to: root)
      let record = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: fixture.appendingPathComponent("fixture.json"))) as? [String: Any])
      let expected = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: fixture.appendingPathComponent("expected.json"))) as? NSDictionary)
      let baseline = try await DocumentCommand.run(method: "get", url: root)
      #expect(try JSONSerialization.jsonObject(with: baseline) as? NSDictionary == expected)
      // Frozen apps check the ABI while mounting; a failed check never becomes ready.
      let controller = try await SlopDocumentWindowController.open(packageURL: root)
      try await controller.session.waitUntilReady()
      // Only the conformance schema has a title; template specimens get generic checks.
      let conformance = record["kind"] as? String != "template"
      if conformance {
        _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("Live command"))
        _ = try await DocumentCommand.run(method: "theme.set", url: root, themeValues: Data(##"{"accent":"#654321"}"##.utf8))
        _ = try await DocumentCommand.run(method: "compact", url: root)
      }
      try await controller.session.finish()
      if conformance {
        _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("Closed command"))
        _ = try await DocumentCommand.run(method: "compact", url: root)
        _ = try await DocumentCommand.run(method: "apply", url: root, operation: prefixTitle("Candidate update"))
      }
      // Renders run the authored app (including its self-checks) against a snapshot:
      // saved state is byte-for-byte what a later read returns.
      let saved = try await DocumentCommand.run(method: "get", url: root)
      let session = try await SlopRuntimeSession.open(packageURL: root, purpose: .backgroundRender)
      session.load()
      try await session.waitUntilReady()
      let png = try await SlopRenderer.exportPNGData(session: session)
      #expect(NSBitmapImageRep(data: png) != nil)
      let pdf = try await SlopRenderer.exportPDFData(session: session)
      #expect((PDFDocument(data: pdf)?.pageCount ?? 0) > 0)
      try await session.finish()
      #expect(try await DocumentCommand.run(method: "get", url: root) == saved)
      // SQLite replay, scenarios, issues and current-engine convergence run in Bun.
      // This test retains the frozen apps' WebKit, CLI, theme and export boundary.
    }
  }
}

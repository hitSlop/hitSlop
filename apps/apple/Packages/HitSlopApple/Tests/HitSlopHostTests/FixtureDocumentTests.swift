import AppKit
import Foundation
import HitSlopCore
import PDFKit
import Testing
import HitSlopTestSupport
@testable import HitSlopHost
@testable import HitSlopDocument

extension HostTests {
  @Test @MainActor func fixtureDocumentsSurviveHostCLIThemeAndExport() async throws {
    _ = NSApplication.shared
    let fixtures = Fixtures.repository.appendingPathComponent("tests/fixtures")
    let entries = try FileManager.default.contentsOfDirectory(at: fixtures, includingPropertiesForKeys: nil)
      .filter { FileManager.default.fileExists(atPath: $0.appendingPathComponent("fixture.json").path) }
    #expect(!entries.isEmpty)
    for fixture in entries {
      let root = try Fixtures.document("tests/fixtures/\(fixture.lastPathComponent)/document")
      defer { try? FileManager.default.removeItem(at: root) }
      let record = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: fixture.appendingPathComponent("fixture.json"))) as? [String: Any])
      let expected = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: fixture.appendingPathComponent("expected.json"))) as? NSDictionary)
      #expect(try await savedValue(root) == expected)
      // Frozen apps check the ABI while mounting; a failed check never becomes ready.
      let controller = try await SlopDocumentWindowController.open(url: root)
      try await controller.session.waitUntilReady()
      // Only the conformance schema has a title; template specimens get generic checks.
      let conformance = record["kind"] as? String != "template"
      if conformance {
        #expect(try await command("batch", url: root, setTitle("Live command")).ok)
        #expect(try await command("theme.set", url: root, ["values": ["accent": "#654321"]]).ok)
      }
      try await controller.session.close()
      if conformance {
        #expect(try await command("batch", url: root, setTitle("Closed command")).ok)
        #expect(try await command("batch", url: root, setTitle("Candidate update")).ok)
      }
      // Renders run the authored app (including its self-checks) against a snapshot:
      // saved state is byte-for-byte what a later read returns.
      let saved = try await savedValue(root)
      let session = try await DocumentSession.open(url: root, storage: .snapshot)
      session.load()
      try await session.waitUntilReady()
      let png = try await SlopRenderer.exportPNGData(session: session)
      #expect(NSBitmapImageRep(data: png) != nil)
      let pdf = try await SlopRenderer.exportPDFData(session: session)
      #expect((PDFDocument(data: pdf)?.pageCount ?? 0) > 0)
      try await session.close()
      #expect(try await savedValue(root) == saved)
      // SQLite replay, scenarios, issues and current-engine convergence run in Bun.
      // This test retains the frozen apps' WebKit, CLI, theme and export boundary.
    }
  }
}

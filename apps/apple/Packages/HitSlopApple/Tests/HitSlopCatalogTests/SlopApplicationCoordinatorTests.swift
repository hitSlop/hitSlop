import Foundation
@testable import HitSlopCatalog
import HitSlopCore
import HitSlopFeatures
import HitSlopTestSupport
import Testing

/// What a Finder open asked the catalog to create from.
private final class Chosen: @unchecked Sendable {
  private let lock = NSLock()
  private var entries: [CatalogEntry] = []
  func append(_ entry: CatalogEntry) { lock.withLock { entries.append(entry) } }
  var all: [CatalogEntry] { lock.withLock { entries } }
}

@MainActor private func coordinator(_ chosen: Chosen, templates: URL) -> SlopApplicationCoordinator {
  var client = CatalogClient.empty
  client.chooseDestination = { entry in chosen.append(entry); return nil }
  return SlopApplicationCoordinator(templatesURL: templates, presentsWindows: false, catalogClient: client)
}

@MainActor private func eventually(_ condition: () -> Bool) async throws {
  for _ in 0..<200 where !condition() { try await Task.sleep(for: .milliseconds(50)) }
  #expect(condition())
}

/// A template opened from anywhere creates a document from itself; it never opens as one.
@MainActor @Test func openingATemplateCreatesADocumentFromIt() async throws {
  let stage = try Fixtures.stage()
  defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
  let template = try Fixtures.template(stage: stage, named: "received")
  let chosen = Chosen()
  let app = coordinator(chosen, templates: stage.deletingLastPathComponent())

  app.openDocument(template)
  try await eventually { !chosen.all.isEmpty }
  let entry = try #require(chosen.all.first)
  #expect(entry.source == .local(SlopPath.canonical(template)))
  #expect(entry.title == (try SlopFile(url: template)).manifest.title)
  #expect(app.store.documents.isEmpty)
}

/// Anything that isn't a template goes to the document path, which reports a file it
/// refuses; nothing is created.
@MainActor @Test func openingAFileThatIsNotATemplateOpensItAsADocument() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let file = folder.appendingPathComponent("damaged.slop")
  try Data("not a hitSlop file".utf8).write(to: file)
  let chosen = Chosen()
  let app = coordinator(chosen, templates: folder)

  app.openDocument(file)
  try await eventually { app.store.alert != nil }
  #expect(chosen.all.isEmpty)
  #expect(app.store.documents.isEmpty)
}

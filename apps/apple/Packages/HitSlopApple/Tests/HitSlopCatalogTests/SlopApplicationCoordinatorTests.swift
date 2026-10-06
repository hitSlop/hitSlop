import Foundation
@testable import HitSlopCatalog
import HitSlopCore
import HitSlopFeatures
import HitSlopTestSupport
import Testing


@MainActor private func coordinator(_ chosen: Locked<[CatalogEntry]>, templates: URL) -> SlopApplicationCoordinator {
  var client = CatalogClient.empty
  client.chooseDestination = { entry in chosen.modify { $0.append(entry) }; return nil }
  return SlopApplicationCoordinator(templatesURL: templates, presentsWindows: false, catalogClient: client)
}

/// A template opened from anywhere creates a document from itself; it never opens as one.
@MainActor @Test func openingATemplateCreatesADocumentFromIt() async throws {
  let stage = try Fixtures.stage()
  defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
  let template = try Fixtures.template(stage: stage, named: "received")
  let chosen = Locked<[CatalogEntry]>([])
  let app = coordinator(chosen, templates: stage.deletingLastPathComponent())

  app.openDocument(template)
  #expect(await eventually(timeout: .seconds(10)) { !chosen.value.isEmpty })
  let entry = try #require(chosen.value.first)
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
  let chosen = Locked<[CatalogEntry]>([])
  let app = coordinator(chosen, templates: folder)

  app.openDocument(file)
  #expect(await eventually(timeout: .seconds(10)) { app.store.alert != nil })
  #expect(chosen.value.isEmpty)
  #expect(app.store.documents.isEmpty)
}

import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopTestSupport
import Testing

@testable import HitSlopCatalog

@MainActor private func coordinator(
  _ chosen: Locked<[CatalogEntry]>, alerts: Locked<[AppAlert]> = Locked([]), templates: URL
) -> SlopApplicationCoordinator {
  var client = CatalogClient.empty
  client.chooseDestination = { entry in
    chosen.modify { $0.append(entry) }
    return nil
  }
  return SlopApplicationCoordinator(
    templatesURL: templates, presentsWindows: false, catalogClient: client,
    alert: { alert, _ in alerts.modify { $0.append(alert) } })
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
  #expect(app.model.documents.isEmpty)
}

/// Anything that isn't a template goes to the document path, which reports a file it
/// refuses; nothing is created.
@MainActor @Test func openingAFileThatIsNotATemplateOpensItAsADocument() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let file = folder.appendingPathComponent("damaged.slop")
  try Data("not a hitSlop file".utf8).write(to: file)
  let chosen = Locked<[CatalogEntry]>([])
  let alerts = Locked<[AppAlert]>([])
  let app = coordinator(chosen, alerts: alerts, templates: folder)

  app.openDocument(file)
  #expect(await eventually(timeout: .seconds(10)) { !alerts.value.isEmpty })
  #expect(chosen.value.isEmpty)
  #expect(app.model.documents.isEmpty)
}

import Foundation
import Testing
import TinysCore
import TinysFeatures
import TinysTestSupport

@testable import TinysCatalog

@Test func copiedDraftsFromOneQuitAccumulateWithoutAdmittingAFailedCopy() {
  var copies = QuitDraftCopies()
  var clipboard = ""
  #expect(
    copies.copy("First window") {
      clipboard = $0
      return true
    })
  #expect(!copies.copy("Failed window") { _ in false })
  #expect(
    copies.copy("Second window") {
      clipboard = $0
      return true
    })
  #expect(clipboard == "First window\n\n---\n\nSecond window")
  #expect(
    !copies.copy("Discarded after a failed copy") { _ in
      clipboard = ""
      return false
    })
  #expect(!copies.restore { _ in false })
  #expect(
    copies.restore {
      clipboard = $0
      return true
    })
  #expect(clipboard == "First window\n\n---\n\nSecond window")
  copies = QuitDraftCopies()
  #expect(
    copies.copy("Next quit") {
      clipboard = $0
      return true
    })
  #expect(clipboard == "Next quit")
}

@MainActor private func coordinator(
  _ chosen: Locked<[CatalogEntry]>, alerts: Locked<[AppAlert]> = Locked([]), templates: URL
) -> TinyApplicationCoordinator {
  var client = CatalogClient.empty
  client.chooseDestination = { entry in
    chosen.modify { $0.append(entry) }
    return nil
  }
  return TinyApplicationCoordinator(
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
  #expect(entry.source == .local(TinyPath.canonical(template)))
  #expect(entry.title == (try TinyFile(url: template)).metadata.title)
  #expect(app.model.documents.isEmpty)
}

/// Anything that isn't a template goes to the document path, which reports a file it
/// refuses; nothing is created.
@MainActor @Test func openingAFileThatIsNotATemplateOpensItAsADocument() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let file = folder.appendingPathComponent("damaged.tiny")
  try Data("not a tinys file".utf8).write(to: file)
  let chosen = Locked<[CatalogEntry]>([])
  let alerts = Locked<[AppAlert]>([])
  let app = coordinator(chosen, alerts: alerts, templates: folder)

  app.openDocument(file)
  #expect(await eventually(timeout: .seconds(10)) { !alerts.value.isEmpty })
  #expect(chosen.value.isEmpty)
  #expect(app.model.documents.isEmpty)
}

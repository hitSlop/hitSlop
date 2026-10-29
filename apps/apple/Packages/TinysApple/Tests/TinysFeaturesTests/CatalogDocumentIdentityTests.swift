import Foundation
import Testing
import TinysCore

@testable import TinysFeatures

@Test func recentIdentityPreservesFilenameAndAbbreviatesOnlyHomeDirectory() {
  let home = FileManager.default.homeDirectoryForCurrentUser
  let url = home.appendingPathComponent("Documents/Personal/Little things — 今日.tiny")
  let identity = TinyDocumentIdentity(url: url)
  #expect(identity.filename == "Little things — 今日.tiny")
  #expect(identity.folderPath == "~/Documents/Personal")
  #expect(identity.path == url.path)
  let outside = TinyDocumentIdentity(url: URL(fileURLWithPath: "/external/Work/Plans/Week.tiny"))
  #expect(outside.folderPath == "/external/Work/Plans")
}

@Test @MainActor func recentsSearchMatchesFilenameFolderAndTemplate() async {
  let first = CatalogEntry(
    id: "first", source: .recent(URL(fileURLWithPath: "/Work/Clients/Little things.tiny")), title: "Document fixture")
  let second = CatalogEntry(
    id: "second", source: .recent(URL(fileURLWithPath: "/Work/Personal/Little things.tiny")), title: "Document fixture")
  let catalog = Catalog()
  catalog.recents = { [first, second] }
  let model = CatalogModel(client: catalog.client)
  model.refreshRecents()
  await model.work.settled()
  model.select(.recents)
  #expect(first.displayTitle == "Little things.tiny")
  #expect(first.documentIdentity?.folderPath != second.documentIdentity?.folderPath)
  model.query = "LITTLE THINGS"
  #expect(model.visibleEntries == [first, second])
  model.query = "clients"
  #expect(model.visibleEntries == [first])
  model.query = "document fixture"
  #expect(model.visibleEntries == [first, second])
}

@Test func templateIdentityAndSearchStillUseManifestMetadata() {
  var entry = CatalogEntry(
    id: "template", source: .local(URL(fileURLWithPath: "/Templates/document-fixture.tiny")), title: "Document fixture")
  entry.categories = [.productivity]
  #expect(entry.displayTitle == "Document fixture")
  #expect(entry.documentIdentity == nil)
  #expect(entry.searchableText.contains("productivity"))
  #expect(!entry.searchableText.contains("/templates/"))
}

@Test @MainActor func recentsShowLibraryDocumentsAsSyncedEntriesAndDropOtherLibraryFiles() async {
  let library = "/Support/tinys/Library"
  let local = CatalogEntry(id: "local", source: .recent(URL(fileURLWithPath: "/Work/Plans.tiny")), title: "Plans")
  var opened = CatalogEntry(
    id: "opened", source: .recent(URL(fileURLWithPath: library + "/account/documents/a.tiny")), title: "Recipe")
  opened.updatedAt = Date(timeIntervalSince1970: 1)
  let stale = CatalogEntry(
    id: "stale", source: .recent(URL(fileURLWithPath: library + "-development-backup/documents/b.tiny")), title: "Old")
  let sibling = CatalogEntry(
    id: "sibling", source: .recent(URL(fileURLWithPath: library + "-backup/Plans.tiny")), title: "Backup")
  let catalog = Catalog()
  catalog.recents = { [opened, stale, sibling, local] }
  let model = CatalogModel(client: catalog.client)
  model.managedFolder = library
  var synced = CatalogEntry(id: "library:a", source: opened.source, title: "Weeknight Recipe")
  synced.isSynced = true
  model.synced = [synced]
  model.refreshRecents()
  await model.work.settled()
  #expect(model.recents.map(\.id) == ["library:a", "sibling", "local"])
  #expect(model.recents.first?.displayTitle == "Weeknight Recipe")
  #expect(model.recents.first?.updatedAt == opened.updatedAt)
}

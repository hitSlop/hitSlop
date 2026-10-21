import Foundation
import HitSlopCore
import Testing

@testable import HitSlopFeatures

@Test func recentIdentityPreservesFilenameAndAbbreviatesOnlyHomeDirectory() {
  let home = FileManager.default.homeDirectoryForCurrentUser
  let url = home.appendingPathComponent("Documents/Personal/Little things — 今日.slop")
  let identity = SlopDocumentIdentity(url: url)
  #expect(identity.filename == "Little things — 今日.slop")
  #expect(identity.folderPath == "~/Documents/Personal")
  #expect(identity.path == url.path)
  let outside = SlopDocumentIdentity(url: URL(fileURLWithPath: "/external/Work/Plans/Week.slop"))
  #expect(outside.folderPath == "/external/Work/Plans")
}

@Test @MainActor func recentsSearchMatchesFilenameFolderAndTemplate() async {
  let first = CatalogEntry(
    id: "first", source: .recent(URL(fileURLWithPath: "/Work/Clients/Little things.slop")), title: "Document fixture")
  let second = CatalogEntry(
    id: "second", source: .recent(URL(fileURLWithPath: "/Work/Personal/Little things.slop")), title: "Document fixture")
  let catalog = Catalog()
  catalog.recents = { [first, second] }
  let model = CatalogModel(client: catalog.client)
  model.refreshRecents()
  await model.work.settled()
  model.select(.recents)
  #expect(first.displayTitle == "Little things.slop")
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
    id: "template", source: .local(URL(fileURLWithPath: "/Templates/document-fixture.slop")), title: "Document fixture")
  entry.categories = [.productivity]
  #expect(entry.displayTitle == "Document fixture")
  #expect(entry.documentIdentity == nil)
  #expect(entry.searchableText.contains("productivity"))
  #expect(!entry.searchableText.contains("/templates/"))
}

@Test @MainActor func recentsShowLibraryDocumentsAsSyncedEntriesAndDropOtherLibraryFiles() async {
  let library = "/Support/hitSlop/Library"
  let local = CatalogEntry(id: "local", source: .recent(URL(fileURLWithPath: "/Work/Plans.slop")), title: "Plans")
  var opened = CatalogEntry(
    id: "opened", source: .recent(URL(fileURLWithPath: library + "/account/documents/a.slop")), title: "Recipe")
  opened.updatedAt = Date(timeIntervalSince1970: 1)
  let stale = CatalogEntry(
    id: "stale", source: .recent(URL(fileURLWithPath: library + "-development-backup/documents/b.slop")), title: "Old")
  let catalog = Catalog()
  catalog.recents = { [opened, stale, local] }
  let model = CatalogModel(client: catalog.client)
  model.managedFolder = library
  var synced = CatalogEntry(id: "library:a", source: opened.source, title: "Weeknight Recipe")
  synced.isSynced = true
  model.synced = [synced]
  model.refreshRecents()
  await model.work.settled()
  #expect(model.recents.map(\.id) == ["library:a", "local"])
  #expect(model.recents.first?.displayTitle == "Weeknight Recipe")
  #expect(model.recents.first?.updatedAt == opened.updatedAt)
}

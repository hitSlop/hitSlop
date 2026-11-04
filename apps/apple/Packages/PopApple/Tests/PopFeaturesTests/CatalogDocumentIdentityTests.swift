import Foundation
import PopCore
import Testing

@testable import PopFeatures

@Test func recentIdentityPreservesFilenameAndAbbreviatesOnlyHomeDirectory() {
  let home = FileManager.default.homeDirectoryForCurrentUser
  let url = home.appendingPathComponent("Documents/Personal/Little things — 今日.pop")
  let identity = PopDocumentIdentity(url: url)
  #expect(identity.filename == "Little things — 今日.pop")
  #expect(identity.folderPath == "~/Documents/Personal")
  #expect(identity.path == url.path)
  let outside = PopDocumentIdentity(url: URL(fileURLWithPath: "/external/Work/Plans/Week.pop"))
  #expect(outside.folderPath == "/external/Work/Plans")
}

@Test @MainActor func recentsSearchMatchesFilenameFolderAndTemplate() async {
  let first = CatalogEntry(
    id: "first", source: .recent(URL(fileURLWithPath: "/Work/Clients/Little things.pop")), title: "Document fixture")
  let second = CatalogEntry(
    id: "second", source: .recent(URL(fileURLWithPath: "/Work/Personal/Little things.pop")), title: "Document fixture")
  let catalog = Catalog()
  catalog.recents = { [first, second] }
  let model = CatalogModel(client: catalog.client)
  model.refreshRecents()
  await model.work.settled()
  model.select(.recents)
  #expect(first.displayTitle == "Little things.pop")
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
    id: "template", source: .local(URL(fileURLWithPath: "/Templates/document-fixture.pop")), title: "Document fixture")
  entry.categories = [.productivity]
  #expect(entry.displayTitle == "Document fixture")
  #expect(entry.documentIdentity == nil)
  #expect(entry.searchableText.contains("productivity"))
  #expect(!entry.searchableText.contains("/templates/"))
}

@Test @MainActor func recentsShowLibraryDocumentsAsSyncedEntriesAndDropOtherLibraryFiles() async {
  let library = "/Support/pop/Library"
  let local = CatalogEntry(id: "local", source: .recent(URL(fileURLWithPath: "/Work/Plans.pop")), title: "Plans")
  var opened = CatalogEntry(
    id: "opened", source: .recent(URL(fileURLWithPath: library + "/account/documents/a.pop")), title: "Recipe")
  opened.updatedAt = Date(timeIntervalSince1970: 1)
  let stale = CatalogEntry(
    id: "stale", source: .recent(URL(fileURLWithPath: library + "-development-backup/documents/b.pop")), title: "Old")
  let sibling = CatalogEntry(
    id: "sibling", source: .recent(URL(fileURLWithPath: library + "-backup/Plans.pop")), title: "Backup")
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

@Test @MainActor func signOutHidesPromotedExternalFilesWhileIndependentLocalCopiesRemain() async {
  let promoted = CatalogEntry(
    id: "external", source: .recent(URL(fileURLWithPath: "/Work/Private.pop")), title: "Private")
  let independent = CatalogEntry(
    id: "copy", source: .recent(URL(fileURLWithPath: "/Work/Private copy.pop")), title: "Local copy")
  let catalog = Catalog()
  catalog.recents = { [promoted, independent] }
  let model = CatalogModel(client: catalog.client)
  model.accountBoundPaths = ["/Work/Private.pop"]
  var cloud = CatalogEntry(id: "cloud", source: promoted.source, title: "Private cloud document")
  cloud.isSynced = true
  model.synced = [cloud]
  model.refreshRecents()
  await model.work.settled()
  #expect(model.recents.map(\.id) == ["cloud", "copy"])
  model.synced = []
  #expect(model.recents.map(\.id) == ["copy"])
  model.synced = [cloud]
  #expect(model.recents.map(\.id) == ["cloud", "copy"])
}

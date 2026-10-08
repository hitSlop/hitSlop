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

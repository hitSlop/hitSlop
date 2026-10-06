import Foundation
import HitSlopCore
import Testing
@testable import HitSlopFeatures

/// A started catalog fed by `local`, with its first listings in.
@MainActor private func started(_ catalog: Catalog, local: AsyncStream<CatalogSnapshot>) async -> CatalogModel {
  catalog.local = { local }
  let model = CatalogModel(client: catalog.client)
  model.start()
  await model.work.settled()
  return model
}

@Test @MainActor func catalogSelectionFollowsSnapshotRemoval() async {
  let catalog = Catalog(), local = AsyncStream<CatalogSnapshot>.makeStream()
  let model = await started(catalog, local: local.stream)
  local.continuation.yield(CatalogSnapshot(entries: [entry("a")]))
  await until { !model.local.isEmpty }
  #expect(model.selectedID == "a")
  local.continuation.yield(CatalogSnapshot())
  await until { model.local.isEmpty }
  #expect(model.selectedID == nil)
}

@Test @MainActor func cancelledCreationIsNotAnError() async {
  let catalog = Catalog(), gate = Gate()
  let template = entry("a")
  catalog.chooseDestination = { _ in
    await gate.wait()
    return nil
  }
  let model = CatalogModel(client: catalog.client)
  var ended: [URL?] = []
  model.onCreationEnded = { ended.append($0) }
  model.primaryAction(template)
  #expect(model.creating == template)
  model.primaryAction(template)
  gate.open()
  await model.work.settled()
  #expect(model.creating == nil && model.creationError == nil)
  #expect(catalog.calls == [.choose("a"), .recents])
  #expect(ended == [nil])
}

@Test @MainActor func failedCreationCanBeRetried() async {
  let catalog = Catalog()
  catalog.chooseDestination = { _ in documentURL }
  catalog.create = { _, _ in throw Failure() }
  let model = CatalogModel(client: catalog.client)
  for _ in 0..<2 {
    model.primaryAction(entry("a"))
    #expect(model.creating != nil)
    await model.work.settled()
    #expect(model.creating == nil && !model.isCopying)
    #expect(model.creationError == "Save failed")
    model.creationError = nil
  }
  #expect(catalog.calls == [.choose("a"), .create("a"), .choose("a"), .create("a")])
}

@Test @MainActor func creationIsRefusedWhileQuitting() async {
  let catalog = Catalog()
  let model = CatalogModel(client: catalog.client)
  model.isQuitting = true
  model.primaryAction(entry("a"))
  #expect(model.creating == nil && catalog.calls.isEmpty)
}

// A listing that finished after a newer one began can never replace it.
@Test @MainActor func aRecentsListingOvertakenByANewerOneIsDropped() async {
  let catalog = Catalog(), gate = Gate()
  let old = entry("old", source: .recent(documentURL))
  catalog.recents = {
    guard catalog.calls.filter({ $0 == .recents }).count == 1 else { return [] }
    await gate.wait()
    return [old]
  }
  let model = CatalogModel(client: catalog.client)
  model.refreshRecents()
  await until { catalog.calls.count == 1 }
  model.refreshRecents()
  await until { catalog.calls.count == 2 }
  gate.open()
  await model.work.settled()
  #expect(model.recents.isEmpty)
}

@Test @MainActor func refreshingSourcesRetainsTheLocalSubscription() async {
  let catalog = Catalog(), local = AsyncStream<CatalogSnapshot>.makeStream()
  let model = await started(catalog, local: local.stream)
  model.start()
  model.refreshSources()
  await model.work.settled()
  await until { catalog.calls.contains(.local) }
  local.continuation.finish()
  #expect(catalog.calls.filter { $0 == .local }.count == 1)
  #expect(catalog.calls.filter { $0 == .refreshLocal(true) }.count == 1)
}

// New artwork for one document rereads that entry, not every recent document.
@Test @MainActor func artworkChangeRereadsOnlyThatRecentDocument() async {
  let first = entry("recent:/a.slop", "A", source: .recent(URL(fileURLWithPath: "/a.slop")))
  let second = entry("recent:/b.slop", "B", source: .recent(URL(fileURLWithPath: "/b.slop")))
  var changed = first
  changed.fileBytes = 42
  let catalog = Catalog()
  catalog.recents = { [first, second] }
  catalog.recent = { [changed] _ in changed }
  let model = await started(catalog, local: AsyncStream { $0.finish() })
  model.select(.recents)
  #expect(model.selectedID == first.id)
  model.artworkChanged(URL(fileURLWithPath: "/a.slop"))
  await model.work.settled()
  #expect(model.recents == [changed, second])
  #expect(catalog.calls.filter { $0 == .recents }.count == 1)
}

@Test @MainActor func categoriesFollowLocalManifestsAndRemovedCategoryReturnsToTemplates() async {
  var bundled = entry("bundled", "Bundled")
  bundled.isBundled = true
  bundled.categories = [.personal, .productivity]
  var installed = entry("installed", "Installed")
  installed.categories = [.finance, .personal]
  let catalog = Catalog(), local = AsyncStream<CatalogSnapshot>.makeStream()
  let model = await started(catalog, local: local.stream)
  local.continuation.yield(CatalogSnapshot(entries: [bundled, installed]))
  await until { model.local.count == 2 }
  #expect(model.selectedID == "bundled")
  #expect(model.categories == [.productivity, .finance, .personal])
  model.select(.category(.finance))
  #expect(model.selectedID == "installed")
  #expect(model.visibleEntries == [installed])
  local.continuation.yield(CatalogSnapshot(entries: [bundled]))
  await until { model.local.count == 1 }
  #expect(model.filter == .all && model.selectedID == "bundled")
  #expect(model.categories == [.productivity, .personal])
}

@Test @MainActor func localSearchImmediatelyFiltersAndKeepsCategorySelectionConsistent() async {
  var checklist = entry("checklist", "Checklist")
  checklist.categories = [.personal, .productivity]
  var expenses = entry("expenses", "Small Expenses")
  expenses.categories = [.productivity]
  let catalog = Catalog(), local = AsyncStream<CatalogSnapshot>.makeStream()
  let model = await started(catalog, local: local.stream)
  local.continuation.yield(CatalogSnapshot(entries: [checklist, expenses]))
  await until { model.local.count == 2 }
  #expect(model.selectedID == checklist.id)
  model.query = "  EXPENSES  "
  #expect(model.selectedID == expenses.id && model.visibleEntries == [expenses])
  model.select(.category(.personal))
  #expect(model.selectedID == nil && model.visibleEntries.isEmpty)
  model.query = ""
  #expect(model.selectedID == checklist.id && model.visibleEntries == [checklist])
  model.select(.all)
  #expect(model.visibleEntries == [checklist, expenses])
}

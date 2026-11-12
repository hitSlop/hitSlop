import Foundation
import PopCore
import PopTestSupport
import Testing

@testable import PopCatalog
@testable import PopFeatures

/// An empty OS history must not erase documents the app already remembered, including
/// after restarting the app. No real user history is read or changed by this test.
@Test @MainActor func recentHistorySurvivesAnEmptySystemListAndRelaunch() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try Fixtures.document(at: folder.appendingPathComponent("remember.pop"))
  let file = folder.appendingPathComponent("recents.json")
  let systemURLs = Locked([document])
  let services = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { systemURLs.value })
  #expect(await services.recents().map(\.source) == [.recent(document)])
  systemURLs.modify { $0 = [] }
  #expect(await services.recents().map(\.source) == [.recent(document)])
  let relaunched = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [] })
  #expect(await relaunched.recents().map(\.source) == [.recent(document)])
}

@Test func recentHistoryReopeningDeduplicatesAndRefreshDoesNotChangeOrder() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let first = try recentFile("first", in: folder)
  let second = try recentFile("second", in: folder)
  let history = RecentDocumentHistory(fileURL: folder.appendingPathComponent("recents.json"))
  let initial = await history.recordOpened(first, at: Date(timeIntervalSince1970: 1))
  let id = try #require(initial.documents.first?.id)
  _ = await history.recordOpened(second, at: Date(timeIntervalSince1970: 2))
  let reopened = await history.recordOpened(first, at: Date(timeIntervalSince1970: 3))
  #expect(reopened.documents.map(\.url) == [first, second])
  #expect(reopened.documents.first?.id == id)
  let refreshed = await history.snapshot()
  #expect(refreshed.documents == reopened.documents)
  let relaunched = RecentDocumentHistory(fileURL: folder.appendingPathComponent("recents.json"))
  #expect(await relaunched.snapshot().documents == refreshed.documents)
}

@Test func recentHistoryFollowsRenamesAndRetainsUnavailableReferences() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let original = try recentFile("original", in: folder)
  let moved = folder.appendingPathComponent("renamed.pop")
  let file = folder.appendingPathComponent("recents.json")
  let history = RecentDocumentHistory(fileURL: file)
  let initial = await history.recordOpened(original)
  let id = try #require(initial.documents.first?.id)
  try FileManager.default.moveItem(at: original, to: moved)
  let renamed = await history.snapshot()
  #expect(renamed.documents.first?.url == moved)
  #expect(try await history.resolve(id) == moved)
  #expect(renamed.documents.first?.id == id)
  #expect(renamed.documents.first?.openedAt == initial.documents.first?.openedAt)
  let again = await history.recordOpened(moved)
  #expect(again.documents.count == 1)
  #expect(again.documents.first?.id == id)
  try FileManager.default.removeItem(at: moved)
  let missing = await history.snapshot()
  #expect(missing.documents.first?.isUnavailable == true)
  #expect(missing.documents.first?.url == moved)
  await #expect(throws: (any Error).self) { try await history.resolve(id) }
  let relaunched = RecentDocumentHistory(fileURL: file)
  #expect(await relaunched.snapshot().documents == missing.documents)
  try Data().write(to: moved)
  let restored = await relaunched.snapshot()
  #expect(restored.documents.first?.isUnavailable == false)
  #expect(restored.documents.first?.id == id)
}

@Test @MainActor func recentHistoryImportsDocumentsOnceAndClearingStaysEmpty() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let first = try Fixtures.document(at: folder.appendingPathComponent("first.pop"))
  let second = try Fixtures.document(at: folder.appendingPathComponent("second.pop"))
  let stage = try Fixtures.minimalStage()
  defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
  let template = try Fixtures.template(stage: stage)
  let unsupported = folder.appendingPathComponent("unsupported.txt")
  try Data().write(to: unsupported)
  let file = folder.appendingPathComponent("recents.json")
  let services = CatalogServices(
    templatesURL: nil, historyURL: file, systemRecents: { [first, template, unsupported, second, first] })
  #expect(await services.recents().map(\.source) == [.recent(first), .recent(second)])
  await services.clearRecents()
  #expect(await services.recents().isEmpty)
  let relaunched = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [first, second] })
  #expect(await relaunched.recents().isEmpty)
  #expect(relaunched.recentsIssue == nil)
  let emptyFile = folder.appendingPathComponent("initially-empty.json")
  let empty = CatalogServices(templatesURL: nil, historyURL: emptyFile, systemRecents: { [] })
  #expect(await empty.recents().isEmpty)
  let later = CatalogServices(templatesURL: nil, historyURL: emptyFile, systemRecents: { [first] })
  #expect(await later.recents().isEmpty, "An initially empty history is also initialized only once")
}

@Test func recentHistoryRetriesStorageFailuresWithoutLosingPendingOpenings() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let first = try recentFile("first", in: folder)
  let second = try recentFile("second", in: folder)
  let file = folder.appendingPathComponent("recents.json")
  let original = RecentDocumentHistory(fileURL: file)
  _ = await original.recordOpened(first)
  let good = try Data(contentsOf: file)
  let damaged = Data("unreadable history".utf8)
  try damaged.write(to: file)
  let recovered = RecentDocumentHistory(fileURL: file)
  let pending = await recovered.recordOpened(second)
  #expect(pending.failure != nil)
  #expect(pending.documents.map(\.url) == [second])
  #expect(try Data(contentsOf: file) == damaged, "A load failure never replaces saved history")
  try good.write(to: file)
  let retried = await recovered.snapshot()
  #expect(retried.failure == nil)
  #expect(retried.documents.map(\.url) == [second, first])
  let relaunched = RecentDocumentHistory(fileURL: file)
  #expect(await relaunched.snapshot().documents == retried.documents)
}

@Test func recentHistoryRetainsMemoryWhenAnAtomicWriteFailsAndRetries() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try recentFile("document", in: folder)
  let file = folder.appendingPathComponent("recents.json")
  let history = RecentDocumentHistory(fileURL: file)
  _ = await history.snapshot()
  let backup = folder.appendingPathComponent("saved.json")
  try FileManager.default.moveItem(at: file, to: backup)
  // A directory at the destination makes atomic replacement fail, even under a user
  // with elevated permissions. The last good file remains independently readable.
  try FileManager.default.createDirectory(at: file, withIntermediateDirectories: true)
  let failed = await history.recordOpened(document)
  #expect(failed.failure != nil)
  #expect(failed.documents.map(\.url) == [document])
  let good = try Data(contentsOf: backup)
  #expect(!good.isEmpty)
  try FileManager.default.removeItem(at: file)
  try FileManager.default.moveItem(at: backup, to: file)
  let retry = await history.snapshot()
  #expect(retry.failure == nil)
  let relaunched = RecentDocumentHistory(fileURL: file)
  #expect(await relaunched.snapshot().documents == retry.documents)
}

@Test func recentHistoryDoesNotExpireOrCapEntriesAndSeparatesBuilds() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let release = RecentDocumentHistory.location(in: folder, development: false)
  let debug = RecentDocumentHistory.location(in: folder, development: true)
  #expect(release != debug)
  let history = RecentDocumentHistory(fileURL: release)
  for index in 0..<105 {
    let url = try recentFile("document-\(index)", in: folder)
    _ = await history.recordOpened(url, at: Date(timeIntervalSince1970: Double(index)))
  }
  let documents = await history.snapshot().documents
  #expect(documents.count == 105)
  #expect(documents.last?.url.lastPathComponent == "document-0.pop")
  let relaunched = RecentDocumentHistory(fileURL: release)
  #expect(await relaunched.snapshot().documents == documents)
  let development = RecentDocumentHistory(fileURL: debug)
  #expect(await development.snapshot().documents.isEmpty)
}

@Test @MainActor func recentHistoryCatalogAndMenuUseTheSameUnavailableEntries() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try recentFile("document", in: folder)
  let file = folder.appendingPathComponent("recents.json")
  let history = RecentDocumentHistory(fileURL: file)
  _ = await history.recordOpened(document)
  try FileManager.default.removeItem(at: document)
  let services = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [] })
  let entries = await services.recents()
  #expect(entries.count == 1)
  #expect(entries.first?.isUnavailable == true)
  let app = PopApplicationCoordinator(templatesURL: nil, presentsWindows: false, historyURL: file)
  await app.refreshRecentDocuments()
  #expect(app.recentDocuments == entries)
  #expect(app.model.catalog.recents == entries)
  var alerts: [AppAlert] = []
  let opening = PopApplicationCoordinator(
    templatesURL: nil, presentsWindows: false, historyURL: file, alert: { alert, _ in alerts.append(alert) })
  await opening.refreshRecentDocuments()
  opening.openRecentDocument(id: try #require(entries.first?.id))
  await opening.model.settled()
  #expect(alerts.count == 1)
  #expect(opening.recentDocuments == entries)
  #expect(opening.model.documents.isEmpty)
}

/// Recents can name a foreign file. It is reachable, so it must say why
/// it can't open (the core's refusal), not look openable or unreachable, and leave Recents on
/// request without the file changing.
@Test @MainActor func recentForeignDocumentIsRefusedAndCanLeaveRecents() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let older = try Fixtures.document(at: folder.appendingPathComponent("older.pop"))
  // A foreign application ID.
  try Fixtures.sql(older, "PRAGMA application_id = \(0x504F_5000)")
  let bytes = try Data(contentsOf: older)
  let current = try Fixtures.document(at: folder.appendingPathComponent("current.pop"))
  let file = folder.appendingPathComponent("recents.json")
  let history = RecentDocumentHistory(fileURL: file)
  _ = await history.recordOpened(current, at: Date(timeIntervalSince1970: 1))
  _ = await history.recordOpened(older, at: Date(timeIntervalSince1970: 2))
  let services = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [] })
  let catalog = CatalogModel(client: services.client)
  await catalog.reloadRecents()
  let entry = try #require(catalog.recents.first)
  #expect(entry.source == .recent(older))
  #expect(!entry.isUnavailable, "The file is reachable")
  #expect(entry.needsAttention)
  let refusal = try #require(entry.refusal)
  #expect(refusal.contains("not a pop document"))
  #expect(refusal.hasSuffix(". The file is unchanged."))
  #expect(catalog.recents.last?.refusal == nil)

  catalog.removeRecent(entry)
  await catalog.work.settled()
  #expect(catalog.recents.map(\.source) == [.recent(current)])
  let relaunched = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [] })
  #expect(await relaunched.recents().map(\.source) == [.recent(current)])
  #expect(try Data(contentsOf: older) == bytes, "Listing and removal never modify the refused file")
}

@Test @MainActor func recentHistoryWarningCanRetryWithoutDroppingTheCatalog() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try recentFile("document", in: folder)
  let file = folder.appendingPathComponent("recents.json")
  let history = RecentDocumentHistory(fileURL: file)
  _ = await history.recordOpened(document)
  let good = try Data(contentsOf: file)
  try Data("broken".utf8).write(to: file)
  let services = CatalogServices(templatesURL: nil, historyURL: file, systemRecents: { [] })
  let catalog = CatalogModel(client: services.client)
  await catalog.reloadRecents()
  #expect(catalog.recentsIssue != nil)
  #expect(catalog.recents.isEmpty)
  try good.write(to: file)
  await catalog.reloadRecents()
  #expect(catalog.recentsIssue == nil)
  #expect(catalog.recents.map(\.source) == [.recent(document)])
}

@Test @MainActor func recentHistoryRecordsOnlySuccessfulOpensAndCannotFailDocumentOpening() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let file = folder.appendingPathComponent("recents.json")
  let damaged = try recentFile("damaged", in: folder)
  let document = try Fixtures.document(at: folder.appendingPathComponent("document.pop"))
  let alerts = Locked<[AppAlert]>([])
  let app = PopApplicationCoordinator(
    templatesURL: nil, presentsWindows: false, historyURL: file,
    alert: { alert, _ in alerts.modify { $0.append(alert) } })
  app.openDocument(damaged)
  #expect(await eventually(timeout: .seconds(10)) { !alerts.value.isEmpty })
  await app.refreshRecentDocuments()
  #expect(app.recentDocuments.isEmpty)
  #expect(app.model.documents.isEmpty)
  // A corrupt history is retained, but cannot stop an otherwise valid document opening.
  let broken = Data("broken history".utf8)
  try broken.write(to: file)
  let opening = PopApplicationCoordinator(
    templatesURL: nil, presentsWindows: false, historyURL: file,
    alert: { alert, _ in alerts.modify { $0.append(alert) } })
  opening.openDocument(document)
  #expect(
    await eventually(timeout: .seconds(15)) {
      opening.model.documents.count == 1 && opening.model.documents.allSatisfy { !$0.isOpening }
    })
  await opening.model.settled()
  #expect(alerts.value.count == 1)
  #expect(opening.model.catalog.recents.map(\.source) == [.recent(document)])
  #expect(opening.model.catalog.recentsIssue != nil)
  #expect(try Data(contentsOf: file) == broken)
  let id = try #require(opening.model.documents.first?.id)
  opening.model.send(.close, to: id)
  await opening.model.settled()
  #expect(opening.model.documents.isEmpty)
}

private func recentFile(_ name: String, in folder: URL) throws -> URL {
  // History does not inspect or edit document contents. Opening still goes through
  // the normal owner acceptance path; these are only filesystem reference fixtures.
  let url = folder.appendingPathComponent(name + ".pop")
  try Data().write(to: url)
  return PopPath.canonical(url)
}

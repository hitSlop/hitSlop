import Foundation
import PopCoreBinding
import PopTestSupport
import Testing

@testable import PopCore
@testable import PopDocument

@Suite(.serialized) struct LibraryOwnerTests {
  @Test @MainActor func downloadedPreviousLibraryDocumentsRemainLocalWithoutAnAccount() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let legacy = folder.appendingPathComponent("Library")
    let saved = legacy.appendingPathComponent(String(repeating: "a", count: 64))
      .appendingPathComponent("PopLibrary/documents")
    try FileManager.default.createDirectory(at: saved, withIntermediateDirectories: true)
    let original = try Fixtures.document(stage: Fixtures.stage())
    let id = try #require(try sharingLocalDocument(path: original.path))
    let cached = saved.appendingPathComponent(id + ".pop")
    try FileManager.default.moveItem(at: original, to: cached)
    try Fixtures.sql(cached, "INSERT INTO share(id,room,endpoint) VALUES(1,'legacy-library','cloudkit')")
    let library = PopLibrary(root: folder.appendingPathComponent("CloudDocuments"), legacyRoot: legacy)
    try await library.setAccount("")
    let entry = try #require(library.visibleEntries.first)
    #expect(entry.localOnly)
    #expect(entry.id == id)
    #expect(URL(fileURLWithPath: entry.path).resolvingSymlinksInPath() == cached.resolvingSymlinksInPath())
    let owner = try DocumentOwner(url: cached)
    _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":2}]}"#)
    try await owner.close()
    let reopened = try DocumentOwner(url: cached)
    let reading = try Fixtures.object(await reopened.state())
    #expect((reading["value"] as? [String: Any])?["hits"] as? Int == 2)
    #expect(reopened.documentID == id)
    try await reopened.close()
    // Repeated discovery does not create a second identity or move the original file.
    let nextLaunch = PopLibrary(root: folder.appendingPathComponent("CloudDocuments"), legacyRoot: legacy)
    try await nextLaunch.setAccount("")
    #expect(nextLaunch.visibleEntries.map(\.id) == [id])
    #expect(FileManager.default.fileExists(atPath: cached.path))
  }

  @Test @MainActor func localCreationAndAccountChangesKeepTheSameFile() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let template = try Fixtures.template(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let owners = LibraryOwners()
    let library = PopLibrary(root: folder, owners: owners)
    let local = try await library.create(template: template, title: "Offline first")
    #expect(local.localOnly)
    let owner = try DocumentOwner(url: URL(fileURLWithPath: local.path))
    #expect(owner.documentID == local.id)
    try owners.register(owner.native)
    _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
    try await library.setAccount("first-account")
    try await library.setAccount("")
    #expect(library.entries.contains { $0.id == local.id && $0.path == local.path })
    #expect(owner.documentID == local.id)
    #expect((try Fixtures.object(await owner.state())["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await owner.close()
  }

  @Test @MainActor func interruptedPromotionKeepsTheLiveOwnerUsableForIndependentCopies() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let source = try Fixtures.document(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: source.deletingLastPathComponent()) }
    let owners = LibraryOwners()
    let library = PopLibrary(root: folder, owners: owners)
    try await library.setAccount("owner")
    let owner = try DocumentOwner(url: source)
    try owners.register(owner.native)
    let id = try #require(owner.documentID)
    let inode = try FileManager.default.attributesOfItem(atPath: source.path)[.systemFileNumber] as? NSNumber
    _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
    library.tokenProvider = { throw PopFailure("Offline before authentication") }
    await #expect(throws: (any Error).self) { try await library.enableSync(source: source, title: "Existing document") }
    await library.refresh()
    let entry = try #require(library.entries.first { $0.id == id })
    var copy: SyncedPop?
    var copyError: String?
    do { copy = try await library.localCopy(entry) } catch { copyError = libraryMessage(error) }
    // Even interrupted promotion must leave the original writer available to its page.
    _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":4}]}"#)
    #expect(owner.documentID == id)
    #expect(URL(fileURLWithPath: entry.path).resolvingSymlinksInPath() == source.resolvingSymlinksInPath())
    #expect(try FileManager.default.attributesOfItem(atPath: source.path)[.systemFileNumber] as? NSNumber == inode)
    #expect((try Fixtures.object(await owner.state())["value"] as? [String: Any])?["hits"] as? Int == 7)
    try await owner.close()
    let independent = try #require(
      copy, "The indexed document must reuse its existing live writer: \(copyError ?? "no copy returned")")
    let copied = try DocumentOwner(url: URL(fileURLWithPath: independent.path))
    #expect(copied.documentID != id)
    #expect((try Fixtures.object(await copied.state())["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await copied.close()
  }

  @Test @MainActor func lateCatalogAndTokenCallbacksCannotReopenTheOldAccount() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let library = PopLibrary(root: folder)
    try await library.setAccount("first-account")
    let oldID = UUID().uuidString.lowercased()
    let newID = UUID().uuidString.lowercased()
    func entry(_ id: String, _ title: String) -> DocumentEntry {
      DocumentEntry(
        documentId: id, title: title, templateId: "fixture", appRevision: nil,
        previewRef: nil, modifiedAt: 1, role: .owner, accessGeneration: 1, catalogRevision: 1, deleted: false)
    }
    var releaseToken: CheckedContinuation<String, Never>?
    var tokenRequested: CheckedContinuation<Void, Never>?
    library.tokenProvider = {
      await withCheckedContinuation { continuation in
        releaseToken = continuation
        tokenRequested?.resume()
      }
    }
    let syncing = Task { await library.syncNow() }
    await withCheckedContinuation { tokenRequested = $0 }
    try await library.setAccount("second-account")
    try await library.receiveCatalog([entry(newID, "Current account")], account: "second-account")
    try await library.receiveCatalog([entry(oldID, "Late old callback")], account: "first-account")
    releaseToken?.resume(returning: "expired-old-account-token")
    await syncing.value
    #expect(library.accountIdentity == "second-account")
    #expect(library.entries.map(\.id) == [newID])
    #expect(library.lastError == nil)
    try await library.setAccount("first-account")
    #expect(!library.entries.contains { $0.id == oldID })
  }
  @Test @MainActor func failedAccountCloseKeepsOwnershipAndExternalLossHidesTheCatalog() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let library = PopLibrary(root: folder)
    try await library.setAccount("owner")
    let id = UUID().uuidString.lowercased()
    let item = DocumentEntry(
      documentId: id, title: "Private", templateId: "fixture", appRevision: nil,
      previewRef: nil, modifiedAt: 1, role: .owner, accessGeneration: 1, catalogRevision: 1, deleted: false)
    try await library.receiveCatalog([item], account: "owner")
    library.closeAccountDocuments = { _ in throw PopFailure("Recover unsaved text first") }
    await #expect(throws: (any Error).self) { try await library.setAccount("") }
    #expect(library.accountIdentity == "owner")
    #expect(library.entries.map(\.id) == [id])
    await #expect(throws: (any Error).self) { try await library.setAccount("", external: true) }
    #expect(library.entries.isEmpty)
    #expect(!library.isReady)
    // Reauthentication restores the retained account; it never discards the blocked editor.
    try await library.setAccount("owner", external: true)
    #expect(library.isReady)
    #expect(library.entries.map(\.id) == [id])
  }

}

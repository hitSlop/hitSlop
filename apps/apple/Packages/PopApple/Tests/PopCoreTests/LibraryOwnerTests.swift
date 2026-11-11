import Foundation
import PopCoreBinding
import PopTestSupport
import Testing

@testable import PopCore
@testable import PopDocument

@Suite(.serialized) struct LibraryOwnerTests {
  @Test @MainActor func localCreationAndAccountChangesKeepTheSameFile() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let template = try Fixtures.template(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let library = PopLibrary(root: folder)
    let local = try await library.create(template: template, title: "Offline first")
    #expect(local.localOnly)
    let owner = try DocumentOwner(url: URL(fileURLWithPath: local.path))
    #expect(owner.documentID == local.id)
    try library.registerOwner(owner.native)
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
    let library = PopLibrary(root: folder)
    try await library.setAccount("owner")
    let owner = try DocumentOwner(url: source)
    try library.registerOwner(owner.native)
    let id = try #require(owner.documentID)
    let inode = try FileManager.default.attributesOfItem(atPath: source.path)[.systemFileNumber] as? NSNumber
    _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
    library.tokenProvider = { throw PopFailure("Offline before authentication") }
    await #expect(throws: (any Error).self) { try await library.enableSync(source: source, title: "Existing document") }
    await library.refresh()
    let entry = try #require(library.entries.first { $0.id == id })
    var copy: LibraryEntry?
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

  @Test @MainActor func sharingRefreshWaitsForTheAcceptedToggleAndQueuesTheLatestStop() async throws {
    let document = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    func state(_ sharing: Bool) -> AccessState {
      AccessState(
        document: document, sharing: sharing,
        capability: sharing ? String(repeating: "b", count: 64) : nil, role: .owner)
    }
    var calls: [Bool] = []
    var server = state(false)
    var first: CheckedContinuation<AccessState, Error>?
    var second: CheckedContinuation<AccessState, Error>?
    defer {
      first?.resume(throwing: PopFailure("Test finished"))
      second?.resume(throwing: PopFailure("Test finished"))
    }
    let model = DocumentSharing(
      services: .init(
        account: { "account-a" }, refresh: { server },
        setSharing: { enabled in
          calls.append(enabled)
          let committed = try await withCheckedThrowingContinuation { continuation in
            if enabled { first = continuation } else { second = continuation }
          }
          server = committed
          return committed
        }), start: false)
    #expect(model.create())
    let clock = ContinuousClock()
    var deadline = clock.now + .seconds(3)
    while first == nil && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    let accepted = try #require(first)
    model.refresh()
    model.stop()
    accepted.resume(returning: state(true))
    first = nil
    deadline = clock.now + .seconds(3)
    while second == nil && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(calls == [true, false], "A stop accepted while busy must execute after the start")
    #expect(model.sharingEnabled, "A routine refresh must not discard the committed start result")
    second?.resume(returning: state(false))
    second = nil
    deadline = clock.now + .seconds(3)
    while model.busy && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(!model.sharingEnabled)
    #expect(!model.busy && !model.acting)
    try await model.pause()
  }

  @Test @MainActor func accountChangesClearSharingAccessAndFenceQueuedOldAccountControls() async throws {
    let old = AccessState(
      document: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", sharing: false, capability: nil, role: .owner)
    @MainActor final class Identity { var account = "account-a" }
    let identity = Identity()
    var calls: [String] = []
    var commit: CheckedContinuation<AccessState, Error>?
    defer { commit?.resume(throwing: PopFailure("Test finished")) }
    let model = DocumentSharing(
      services: .init(
        account: { identity.account }, refresh: { identity.account == "account-a" ? old : nil },
        setSharing: { enabled in
          calls.append("\(identity.account):\(enabled)")
          return try await withCheckedThrowingContinuation { commit = $0 }
        }), start: false)
    let clock = ContinuousClock()
    model.refresh()
    var deadline = clock.now + .seconds(3)
    while model.busy && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(model.access?.role == .owner)
    #expect(model.create())
    deadline = clock.now + .seconds(3)
    while commit == nil && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    let accepted = try #require(commit)
    model.stop()
    identity.account = "account-b"
    model.refresh()
    #expect(model.access == nil && model.link == nil && model.status == nil)
    accepted.resume(returning: old)
    commit = nil
    deadline = clock.now + .seconds(3)
    while model.busy && clock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(model.access == nil && model.error == nil)
    #expect(calls == ["account-a:true"], "Queued controls from the old account must not run under the new identity")
    try await model.pause()
  }

  @Test @MainActor func nativeIndexMutationRefreshesTheSnapshotWithoutStartingTheFallbackTimer() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let template = try Fixtures.template(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let library = PopLibrary(root: folder)
    try await library.setAccount("")
    let native = try #require(library.nativeLibrary)
    let created = try await PopPreparation.run {
      try native.create(template: template.path, title: "Callback document")
    }
    let clock = ContinuousClock()
    let deadline = clock.now + .seconds(3)
    while !library.entries.contains(where: { $0.id == created.id }) && clock.now < deadline {
      try await Task.sleep(for: .milliseconds(10))
    }
    #expect(library.entries.contains { $0.id == created.id && $0.title == "Callback document" })
    #expect(library.lastError == nil)
  }

  @Test @MainActor func accountActivationDoesNotRequireCredentialsOrStartSync() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let library = PopLibrary(root: folder)
    library.tokenProvider = { throw PopFailure("Account activation must not acquire credentials") }
    try await library.connectAccount("account-a")
    #expect(library.isReady && library.accountIdentity == "account-a")
    #expect(library.lastError == nil, "Activation must finish independently of token acquisition")
  }

  @Test @MainActor func lateTokenCallbacksCannotReopenTheOldAccount() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let library = PopLibrary(root: folder)
    try await library.setAccount("first-account")
    let oldID = UUID().uuidString.lowercased()
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
    let template = try Fixtures.template(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let current = try await library.create(template: template, title: "Current account")
    let newID = current.id
    releaseToken?.resume(returning: "expired-old-account-token")
    await syncing.value
    #expect(library.accountIdentity == "second-account")
    #expect(library.entries.map(\.id) == [newID])
    #expect(library.lastError == nil)
    #expect(!library.isSyncing, "An old token callback must not leave the replacement account busy")
    try await library.setAccount("first-account")
    #expect(!library.entries.contains { $0.id == oldID })
  }
  @Test @MainActor func failedAccountCloseKeepsOwnershipAndExternalLossHidesTheCatalog() async throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let library = PopLibrary(root: folder)
    try await library.setAccount("owner")
    let template = try Fixtures.template(stage: Fixtures.stage())
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let created = try await library.create(template: template, title: "Private")
    let id = created.id
    let native = try NativeLibrary.open(root: folder.path, account: "owner")
    _ = try? native.enableSync(account: "owner", id: id, token: "")
    await library.refresh()
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

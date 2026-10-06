import Foundation
import HitSlopCore
import Testing
@testable import HitSlopFeatures

let documentID = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
let otherID = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!
let documentURL = URL(fileURLWithPath: "/tmp/example.slop")
struct Failure: LocalizedError { var errorDescription: String? { "Save failed" } }

/// The URL a seeded document opened from.
func url(_ id: UUID) -> URL { id == documentID ? documentURL : URL(fileURLWithPath: "/tmp/\(id.uuidString).slop") }

/// The native side of `AppModel`: it records every call in order. A test sets what an
/// operation does; one it did not set records an issue.
@MainActor final class Native {
  enum Call: Equatable {
    case open(UUID), focus(UUID), perform(UUID, SlopDocumentCommand)
    case prepare(UUID), finish(UUID), cancel(UUID), reply(Bool)
    case alert(AppAlert, UUID?), noDocuments
  }
  var calls: [Call] = []
  /// Whether each window takes commands, as last told.
  var enabled: [UUID: Bool] = [:]
  var open: @MainActor (UUID, URL) async throws -> Void = { _, _ in Issue.record("unexpected open") }
  var perform: @MainActor (UUID, SlopDocumentCommand) async throws -> URL? = { _, _ in
    Issue.record("unexpected perform")
    return nil
  }
  var prepareToQuit: @MainActor (UUID) async throws -> Void = { _ in Issue.record("unexpected prepareToQuit") }
  var finishQuit: @MainActor (UUID) async throws -> Void = { _ in Issue.record("unexpected finishQuit") }

  var performed: [SlopDocumentCommand] { calls.compactMap { if case .perform(_, let command) = $0 { command } else { nil } } }
  var alerts: [Call] { calls.filter { if case .alert = $0 { true } else { false } } }
  var replies: [Bool] { calls.compactMap { if case .reply(let allowed) = $0 { allowed } else { nil } } }

  var client: AppClient {
    AppClient(
      open: { id, url in self.calls.append(.open(id)); try await self.open(id, url) },
      focus: { self.calls.append(.focus($0)) },
      perform: { id, command in self.calls.append(.perform(id, command)); return try await self.perform(id, command) },
      prepareToQuit: { self.calls.append(.prepare($0)); try await self.prepareToQuit($0) },
      finishQuit: { self.calls.append(.finish($0)); try await self.finishQuit($0) },
      cancelQuit: { self.calls.append(.cancel($0)) },
      replyToQuit: { self.calls.append(.reply($0)) },
      alert: { self.calls.append(.alert($0, $1)) },
      commandsEnabled: { self.enabled[$0] = $1 },
      noDocumentsOpen: { self.calls.append(.noDocuments) })
  }
}

/// The catalog's side: listings it is given, and the calls it receives.
@MainActor final class Catalog {
  enum Call: Equatable { case local, refreshLocal(Bool), recents, recent(URL), choose(String), create(String) }
  var calls: [Call] = []
  var local: @MainActor () async -> AsyncStream<CatalogSnapshot> = { AsyncStream { $0.finish() } }
  var refreshLocal: @MainActor (Bool) async -> Void = { _ in }
  var recents: @MainActor () async -> [CatalogEntry] = { [] }
  var recent: @MainActor (URL) async -> CatalogEntry? = { _ in
    Issue.record("unexpected recent")
    return nil
  }
  var chooseDestination: @MainActor (CatalogEntry) async throws -> URL? = { _ in
    Issue.record("unexpected chooseDestination")
    return nil
  }
  var create: @MainActor (CatalogEntry, URL) async throws -> URL = { _, url in
    Issue.record("unexpected create")
    return url
  }

  var client: CatalogClient {
    CatalogClient(
      local: { self.calls.append(.local); return await self.local() },
      refreshLocal: { self.calls.append(.refreshLocal($0)); await self.refreshLocal($0) },
      recents: { self.calls.append(.recents); return await self.recents() },
      recent: { self.calls.append(.recent($0)); return await self.recent($0) },
      chooseDestination: { self.calls.append(.choose($0.id)); return try await self.chooseDestination($0) },
      create: { entry, url in self.calls.append(.create(entry.id)); return try await self.create(entry, url) })
  }
}

/// Holds work until the test opens it. Cancellation does not open it, so work that was
/// cancelled still finishes late.
@MainActor final class Gate {
  private var isOpen = false
  private var waiters: [CheckedContinuation<Void, Never>] = []
  func wait() async {
    guard !isOpen else { return }
    await withCheckedContinuation { waiters.append($0) }
  }
  func open() {
    isOpen = true
    for waiter in waiters { waiter.resume() }
    waiters = []
  }
}

/// Lets main-actor work run until `condition` holds.
@MainActor func until(_ condition: () -> Bool, sourceLocation: SourceLocation = #_sourceLocation) async {
  for _ in 0..<10_000 {
    if condition() { return }
    await Task.yield()
  }
  Issue.record("the condition never held", sourceLocation: sourceLocation)
}

/// An app with the documents `opened` open and idle, and `ids` for the documents it opens
/// next. The calls made while opening them are forgotten.
@MainActor func app(_ native: Native, catalog: Catalog = Catalog(), open opened: [UUID] = [], ids: [UUID] = []) async
  -> AppModel
{
  var next = opened + ids
  let model = AppModel(
    client: native.client, catalog: CatalogModel(client: catalog.client), makeID: { next.removeFirst() })
  let open = native.open
  native.open = { _, _ in }
  for id in opened { model.open(url(id)) }
  await model.settled()
  native.open = open
  native.calls = []
  native.enabled = [:]
  catalog.calls = []
  return model
}

func entry(_ id: String, _ title: String = "Counter", source: CatalogEntry.Source = .local(documentURL)) -> CatalogEntry {
  CatalogEntry(id: id, source: source, title: title)
}

import Foundation
import HitSlopCore

/// How the app reaches its native side: document windows, their operations, quitting and
/// alerts.
public struct AppClient {
  /// Opens the document at `url` as `id`; throws `CancellationError` when the person cancels.
  public var open: @MainActor (UUID, URL) async throws -> Void
  /// Brings a document forward, or its progress while it opens.
  public var focus: @MainActor (UUID) -> Void
  /// Runs a document operation and returns the new document a duplicate made. Close returns
  /// only after native teardown. Failures arrive classified as `SlopDocumentFailure`.
  public var perform: @MainActor (UUID, SlopDocumentCommand) async throws -> URL?
  public var prepareToQuit: @MainActor (UUID) async throws -> Void
  public var finishQuit: @MainActor (UUID) async throws -> Void
  public var cancelQuit: @MainActor (UUID) async -> Void
  /// Answers AppKit's request to terminate.
  public var replyToQuit: @MainActor (Bool) -> Void
  /// Shows an alert on the document's window, or over the app when there is none.
  public var alert: @MainActor (AppAlert, UUID?) -> Void
  /// Whether a document takes operations now; its toolbar and menus follow.
  public var commandsEnabled: @MainActor (UUID, Bool) -> Void
  /// The last document went away while the app keeps running.
  public var noDocumentsOpen: @MainActor () -> Void

  public init(
    open: @escaping @MainActor (UUID, URL) async throws -> Void,
    focus: @escaping @MainActor (UUID) -> Void,
    perform: @escaping @MainActor (UUID, SlopDocumentCommand) async throws -> URL?,
    prepareToQuit: @escaping @MainActor (UUID) async throws -> Void,
    finishQuit: @escaping @MainActor (UUID) async throws -> Void,
    cancelQuit: @escaping @MainActor (UUID) async -> Void,
    replyToQuit: @escaping @MainActor (Bool) -> Void,
    alert: @escaping @MainActor (AppAlert, UUID?) -> Void,
    commandsEnabled: @escaping @MainActor (UUID, Bool) -> Void,
    noDocumentsOpen: @escaping @MainActor () -> Void
  ) {
    self.open = open
    self.focus = focus
    self.perform = perform
    self.prepareToQuit = prepareToQuit
    self.finishQuit = finishQuit
    self.cancelQuit = cancelQuit
    self.replyToQuit = replyToQuit
    self.alert = alert
    self.commandsEnabled = commandsEnabled
    self.noDocumentsOpen = noDocumentsOpen
  }
}

/// The open documents and their lifecycle: opening, one operation at a time per document
/// (`CommandQueue`), and quitting once nothing is opening, creating or running. Every
/// change happens on the main actor; anything read before an `await` is read again after.
@MainActor public final class AppModel {
  public enum QuitPhase: Equatable, Sendable { case running, waiting, preparing, finished }
  public struct Document: Equatable, Identifiable, Sendable {
    public let id: UUID
    public let url: URL
    public internal(set) var isOpening = true
    var queue = CommandQueue()
  }

  public private(set) var documents: [Document] = []
  public private(set) var quitPhase = QuitPhase.running
  public let catalog: CatalogModel
  private let client: AppClient
  private let makeID: () -> UUID
  private let work = TaskSet()

  public init(client: AppClient, catalog: CatalogModel, makeID: @escaping () -> UUID = UUID.init) {
    self.client = client
    self.catalog = catalog
    self.makeID = makeID
    catalog.onOpen = { [weak self] url in self?.open(url) }
    catalog.onCreationEnded = { [weak self] url in self?.creationEnded(url) }
  }

  public subscript(id id: UUID) -> Document? { documents.first { $0.id == id } }

  /// Whether `id` takes operations now: open, not quitting, and with none running or waiting.
  public func acceptsCommands(_ id: UUID) -> Bool {
    guard let document = self[id: id] else { return false }
    return !document.isOpening && quitPhase == .running && document.queue.isIdle
  }

  /// Opens the document at `url` (resolved by the caller), or brings it forward when it is
  /// already open. Nothing new opens once quit began.
  public func open(_ url: URL) {
    guard quitPhase == .running else { return }
    insert(url)
  }

  /// Runs `command` now, or after the operation in progress when it is a close or an answer
  /// to the save-failure sheet. Quit takes no new operations, but a save recovery still
  /// runs: quit waits for it, as it waits for any operation.
  public func send(_ command: SlopDocumentCommand, to id: UUID) {
    guard let index = index(id), !documents[index].isOpening, quitPhase == .running || command.isSaveRecovery
    else { return }
    let now = documents[index].queue.admit(command)
    refresh(id)
    if let now { work.run { await self.run(now, for: id) } }
  }

  /// AppKit asked to terminate. Quit waits until nothing is opening, creating or running,
  /// prepares every document to close, then closes each; `replyToQuit` gets the answer
  /// once. A request while quitting is ignored.
  public func requestQuit() {
    guard quitPhase == .running else { return }
    quitPhase = .waiting
    catalog.isQuitting = true
    refreshAll()
    advanceQuit()
  }

  /// Waits for everything this model and its catalog started.
  func settled() async {
    while !work.isEmpty || !catalog.work.isEmpty {
      await work.settled()
      await catalog.work.settled()
    }
  }

  // MARK: Opening

  /// Adds a document for `url` and starts opening it; an open one comes forward instead.
  /// Quit waits for a document added here.
  private func insert(_ url: URL) {
    if let existing = documents.first(where: { $0.url == url }) { return client.focus(existing.id) }
    let id = makeID()
    documents.append(Document(id: id, url: url))
    work.run { await self.finishOpening(id, url) }
  }

  private func finishOpening(_ id: UUID, _ url: URL) async {
    do {
      try await client.open(id, url)
      if let index = index(id) { documents[index].isOpening = false }
      refresh(id)
      catalog.refreshRecents()
    } catch is CancellationError {
      remove(id)
    } catch {
      remove(id)
      let message = error.localizedDescription
      let update = SlopFailureContext.classify(error).reason == .requiresUpdate
      client.alert(update ? .requiresUpdate(message) : .failure(message), nil)
    }
    advanceQuit()
  }

  /// The catalog finished creating, successfully or not. A new document opens even while
  /// quit waits, since quit waited for its creation.
  private func creationEnded(_ url: URL?) {
    if let url { insert(url) }
    advanceQuit()
  }

  // MARK: Operations

  private func run(_ first: SlopDocumentCommand, for id: UUID) async {
    var next: SlopDocumentCommand? = first
    while let command = next {
      let result: Result<URL?, SlopDocumentFailure>
      do { result = .success(try await client.perform(id, command)) } catch {
        result = .failure(SlopDocumentFailure(error))
      }
      next = finished(command, of: id, result)
    }
    advanceQuit()
  }

  /// Records how `command` ended and returns what runs next.
  private func finished(
    _ command: SlopDocumentCommand, of id: UUID, _ result: Result<URL?, SlopDocumentFailure>
  ) -> SlopDocumentCommand? {
    guard let index = index(id) else { return nil }
    switch result {
    case .success(let url):
      if command == .close {
        remove(id)
        return nil
      }
      let next = documents[index].queue.finish(failed: false)
      refresh(id)
      // The copy opens even while quit waits; quit then waits for it too.
      if command == .duplicate, let url { insert(url) }
      return next
    case .failure(let failure):
      let next = documents[index].queue.finish(failed: true)
      refresh(id)
      if command == .close, quitPhase == .waiting {
        cancelQuit(failure)
      } else if case .other(let message) = failure {
        // The window's save-failure sheet presents a save failure.
        client.alert(.failure(message), id)
      }
      return next
    }
  }

  // MARK: Quitting

  private func advanceQuit() {
    guard quitPhase == .waiting, catalog.creating == nil,
      documents.allSatisfy({ !$0.isOpening && $0.queue.isIdle })
    else { return }
    quitPhase = .preparing
    let ids = documents.map(\.id)
    work.run { await self.quit(ids) }
  }

  private func quit(_ ids: [UUID]) async {
    var remaining = ids[...]
    do {
      for id in ids { try await client.prepareToQuit(id) }
      for id in ids {
        try await client.finishQuit(id)
        remaining = remaining.dropFirst()
        remove(id)
      }
      quitPhase = .finished
      client.replyToQuit(true)
    } catch {
      // A completed close has destroyed its renderer and cannot be rolled back.
      for id in remaining { await client.cancelQuit(id) }
      cancelQuit(SlopDocumentFailure(error))
    }
  }

  /// A save failure is already on its document's save-failure sheet; other failures alert.
  private func cancelQuit(_ failure: SlopDocumentFailure) {
    quitPhase = .running
    catalog.isQuitting = false
    refreshAll()
    if case .other(let message) = failure { client.alert(.failure(message), nil) }
    client.replyToQuit(false)
  }

  // MARK: Documents

  private func index(_ id: UUID) -> Int? { documents.firstIndex { $0.id == id } }

  private func remove(_ id: UUID) {
    documents.removeAll { $0.id == id }
    if documents.isEmpty, quitPhase == .running { client.noDocumentsOpen() }
  }

  private func refresh(_ id: UUID) {
    if self[id: id] != nil { client.commandsEnabled(id, acceptsCommands(id)) }
  }

  private func refreshAll() {
    for document in documents { refresh(document.id) }
  }
}

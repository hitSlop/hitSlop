import Combine
import Foundation
import PopCore
import PopCoreBinding
import PopWidgets

/// Shared capture ordering and publication. Hosts supply their document key and source lookup.
@MainActor public final class WidgetPublisher {
  private var tail: Task<Void, Never>?
  private var pending: [String: (version: UUID, task: Task<Void, Never>)] = [:]
  private var lifetime = UUID()
  private let report: (Error) -> Void

  public init(report: @escaping (Error) -> Void) { self.report = report }

  public func invalidate() {
    lifetime = UUID()
    for save in pending.values { save.task.cancel() }
    pending.removeAll()
  }

  public func cancelSave(key: String) { pending.removeValue(forKey: key)?.task.cancel() }

  public func saved(key: String, refresh: @escaping @MainActor () async -> Void) {
    cancelSave(key: key)
    let version = UUID()
    let task = Task { [weak self] in
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      guard let self, pending[key]?.version == version else { return }
      defer {
        if pending[key]?.version == version { pending[key] = nil }
      }
      await refresh()
    }
    pending[key] = (version, task)
  }

  public func serialized(_ work: @escaping @MainActor () async throws -> Void) async {
    let previous = tail
    let started = lifetime
    let task = Task { [self] in
      await previous?.value
      guard !Task.isCancelled, lifetime == started else { return }
      do { try await work() } catch {
        if !Task.isCancelled { report(error) }
      }
    }
    tail = task
    await withTaskCancellationHandler {
      await task.value
    } onCancel: {
      task.cancel()
    }
  }

  /// Close may already hold the owner's snapshot barrier and bypass queued refreshes.
  public func capture(
    valid: () -> Bool = { true }, render: () async throws -> DocumentWidgetCapture,
    publish: (DocumentWidgetCapture) throws -> Bool, reload: () -> Void
  ) async {
    let started = lifetime
    do {
      guard !Task.isCancelled, valid() else { return }
      let capture = try await render()
      try Task.checkCancellation()
      guard lifetime == started, valid() else { return }
      if try publish(capture) { reload() }
    } catch {
      if !Task.isCancelled { report(error) }
    }
  }

  /// The Rust fingerprint includes saved state, renderer identity and widget canvases.
  public func refresh(
    url: URL, currentVersion: () -> String?, scheduled: Bool = false,
    valid: () -> Bool = { true }, render: () async throws -> DocumentWidgetCapture,
    publish: (DocumentWidgetCapture) throws -> Bool, reload: () -> Void
  ) async throws {
    let started = lifetime
    let version = try await DocumentArtworkRenderer.widgetSourceVersion(url: url)
    guard !Task.isCancelled, lifetime == started, valid(), currentVersion() != version || scheduled else { return }
    await capture(valid: valid, render: render, publish: publish, reload: reload)
  }
}

// Shared library membership, account isolation and picker publication for both hosts.
@MainActor public final class WidgetLibraryIndex {
  public private(set) var entries: [LibraryEntry] = []
  public private(set) var lifetime = UUID()
  private let cache: WidgetCache
  private let publisher: WidgetPublisher
  private let changed: (Set<String>) -> Void
  private let reload: () -> Void
  private let report: (Error) -> Void
  private var revision = UUID()
  private var subscriptions: Set<AnyCancellable> = []

  public init(
    cache: WidgetCache, publisher: WidgetPublisher, entries: [LibraryEntry] = [], reload: @escaping () -> Void,
    report: @escaping (Error) -> Void, changed: @escaping (Set<String>) -> Void
  ) {
    self.entries = entries
    self.cache = cache
    self.publisher = publisher
    self.reload = reload
    self.report = report
    self.changed = changed
  }

  public func observe(_ library: PopLibrary) {
    library.$accountIdentity.sink { [weak self] in self?.accountChanged($0) }.store(in: &subscriptions)
    library.$entries.sink { [weak self, weak library] entries in
      guard library?.hasLoaded == true else { return }
      self?.update(entries)
    }.store(in: &subscriptions)
    library.$hasLoaded.sink { [weak self, weak library] loaded in
      if loaded, let library { self?.update(library.entries) }
    }.store(in: &subscriptions)
  }

  public func entry(id: String) -> LibraryEntry? { entries.first { $0.id == id && !$0.deleted } }
  public func entry(at url: URL) -> LibraryEntry? {
    entries.first { !$0.deleted && URL(fileURLWithPath: $0.path).standardizedFileURL == url.standardizedFileURL }
  }

  private func accountChanged(_ account: String) {
    let marker = cache.root.appendingPathComponent("account")
    guard (try? String(contentsOf: marker, encoding: .utf8)) != account else { return }
    lifetime = UUID()
    revision = UUID()
    publisher.invalidate()
    entries.removeAll { $0.accountBound }
    do {
      // Local documents survive sign-in and sign-out, including across app launches.
      let stored = cache.libraryIndex().filter { !$0.accountBound }
      let localEntries = entries.filter { !$0.accountBound }
      let storedIDs = Set(stored.map(\.id))
      let local =
        stored
        + localEntries.filter { !storedIDs.contains($0.id) }.map {
          WidgetLibraryEntry(id: $0.id, title: $0.title, icon: nil)
        }
      let ids = Set(local.map(\.id))
      for record in cache.records() where !ids.contains(record.id) { try cache.remove(libraryID: record.id) }
      try cache.writeLibraryIndex(local, retainingPrevious: false)
      try Data(account.utf8).write(to: marker, options: .atomic)
      reload()
      changed(Set(entries.map(\.id)))
    } catch { report(error) }
  }

  public func update(_ next: [LibraryEntry]) {
    let previous = Dictionary(uniqueKeysWithValues: entries.map { ($0.id, $0.version) })
    entries = next.filter { !$0.deleted }
    let visible = entries
    let ids = Set(visible.map(\.id))
    revision = UUID()
    let current = revision
    do {
      var removed = false
      for record in cache.records() where !ids.contains(record.id) {
        try cache.remove(libraryID: record.id)
        removed = true
      }
      // Remove inaccessible account documents from the picker before asynchronous icon work.
      let index = cache.libraryIndex()
      let current = Dictionary(uniqueKeysWithValues: visible.map { ($0.id, $0) })
      try cache.writeLibraryIndex(
        index.compactMap { item in
          guard let entry = current[item.id] else { return nil }
          return WidgetLibraryEntry(id: entry.id, title: entry.title, icon: item.icon, accountBound: entry.accountBound)
        }, retainingPrevious: !index.contains { $0.accountBound && !ids.contains($0.id) })
      if removed { reload() }
    } catch { report(error) }
    Task { [weak self] in
      do {
        let index = try await DocumentArtworkRenderer.libraryIndex(entries: visible)
        guard let self, revision == current else { return }
        try cache.writeLibraryIndex(index)
        reload()
      } catch { self?.report(error) }
    }
    changed(Set(visible.filter { previous[$0.id] != $0.version }.map(\.id)))
  }

  public func invalidate() {
    lifetime = UUID()
    revision = UUID()
    subscriptions.removeAll()
    publisher.invalidate()
  }
}

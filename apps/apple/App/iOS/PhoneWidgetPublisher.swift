import Combine
import Foundation
import HitSlopCore
import HitSlopDocument
import HitSlopWidgets
import WidgetKit

@MainActor final class PhoneWidgetPublisher {
  private let cache = try? WidgetCache.shared()
  private var subscriptions: Set<AnyCancellable> = []
  private var entries: [SyncedSlop] = []
  private var pending: [String: Task<Void, Never>] = [:]
  private var tail: Task<Void, Never>?
  private var lifetime = UUID()
  private var indexRevision = UUID()
  private var account = ""
  private let session: () -> DocumentSession?

  init(session: @escaping () -> DocumentSession?) {
    self.session = session
    SlopLibrary.shared.$accountIdentity.sink { [weak self] in self?.accountChanged($0) }.store(in: &subscriptions)
    SlopLibrary.shared.$entries.sink { [weak self] in self?.libraryChanged($0) }.store(in: &subscriptions)
  }

  private func accountChanged(_ account: String) {
    guard !account.isEmpty, account != self.account, let cache else { return }
    self.account = account
    lifetime = UUID()
    indexRevision = UUID()
    for task in pending.values { task.cancel() }
    pending.removeAll()
    let marker = cache.root.appendingPathComponent("account")
    do {
      let previous = try? String(contentsOf: marker, encoding: .utf8)
      if previous != account { try cache.clear() }
      try FileManager.default.createDirectory(at: cache.root, withIntermediateDirectories: true)
      try Data(account.utf8).write(to: marker, options: .atomic)
      if account != "unavailable" {
        libraryChanged(SlopLibrary.shared.entries)
        foreground()
      }
    } catch { report(error) }
  }

  private func libraryChanged(_ next: [SyncedSlop]) {
    let previous = Dictionary(uniqueKeysWithValues: entries.map { ($0.id, $0.version) })
    entries = next
    guard !account.isEmpty, account != "unavailable", let cache else { return }
    let revision = UUID()
    indexRevision = revision
    let started = lifetime
    let visible = next.filter { !$0.deleted }
    let ids = Set(visible.map(\.id))
    do {
      for record in cache.records() where !ids.contains(record.id) { try cache.remove(libraryID: record.id) }
    } catch { report(error) }
    Task {
      do {
        let index = try await DocumentArtworkRenderer.libraryIndex(entries: visible)
        guard lifetime == started, indexRevision == revision else { return }
        try cache.writeLibraryIndex(index)
      } catch { report(error) }
    }
    let changed = Set(visible.filter { previous[$0.id] != $0.version }.map(\.id))
    Task { await refresh(ids: changed) }
  }

  func foreground() { Task { await refresh(ids: nil) } }

  func saved(_ session: DocumentSession) {
    guard let entry = SlopLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    pending[entry.id]?.cancel()
    pending[entry.id] = Task { [weak self] in
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      guard let self else { return }
      pending[entry.id] = nil
      await refresh(ids: [entry.id])
    }
  }

  func closing(_ session: DocumentSession) async {
    guard let entry = SlopLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    pending.removeValue(forKey: entry.id)?.cancel()
    await refresh(ids: [entry.id])
  }

  private func placedIDs() async -> Set<String> {
    do {
      let configurations: [WidgetInfo] = try await withCheckedThrowingContinuation { continuation in
        WidgetCenter.shared.getCurrentConfigurations { continuation.resume(with: $0) }
      }
      return Set(
        configurations.filter { $0.kind == WidgetCache.kind }.compactMap {
          $0.widgetConfigurationIntent(of: SlopWidgetIntent.self)?.slop?.id
        })
    } catch {
      report(error)
      return []
    }
  }

  private func refresh(ids: Set<String>?) async {
    let started = lifetime
    let placed = await placedIDs()
    guard lifetime == started else { return }
    let requested = ids.map { placed.intersection($0) } ?? placed
    guard !requested.isEmpty else { return }
    let previous = tail
    let task = Task { [self] in
      await previous?.value
      for id in requested {
        guard lifetime == started, account != "unavailable", !account.isEmpty, let cache,
          let entry = entries.first(where: { $0.id == id && !$0.deleted })
        else { continue }
        do {
          let url = URL(fileURLWithPath: entry.path)
          let version = try await DocumentArtworkRenderer.widgetSourceVersion(url: url)
          guard lifetime == started, cache.record(id: id)?.sourceVersion != version else { continue }
          let capturedAt = Date()
          let capture: DocumentWidgetCapture
          if let open = session(), open.file.url == url {
            guard open.isReady else { continue }
            capture = try await DocumentArtworkRenderer.widgetCapture(session: open)
          } else {
            capture = try await DocumentArtworkRenderer.widgetCapture(url: url)
          }
          guard lifetime == started, let current = entries.first(where: { $0.id == id && !$0.deleted }) else {
            continue
          }
          let previousGeneration = cache.record(id: id)?.generation
          let record = try cache.publish(
            libraryID: id, title: current.title, images: capture.images, capturedAt: capturedAt,
            sourceVersion: capture.sourceVersion)
          if record.generation != previousGeneration { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
        } catch { report(error) }
      }
    }
    tail = task
    await task.value
  }

  private func report(_ error: Error) {
    NSLog("hitSlop widget refresh failed: %@", error.localizedDescription)
  }
}

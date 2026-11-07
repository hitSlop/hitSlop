import Combine
import Foundation
import PopCore
import PopCoreBinding
import PopDocument
import PopWidgets
import UIKit
import WidgetKit

@MainActor final class PhoneWidgetPublisher {
  private let cache = try? WidgetCache.shared()
  private var subscriptions: Set<AnyCancellable> = []
  private var entries: [LibraryEntry] = []
  /// A saved document's debounced refresh. A newer save replaces it; only the current one clears itself.
  private var pending: [String: (version: UUID, task: Task<Void, Never>)] = [:]
  private var tail: Task<Void, Never>?
  private var backgroundRefreshes = 0
  private var captureAllowed: Bool {
    UIApplication.shared.applicationState == .active || backgroundRefreshes > 0
  }
  private var lifetime = UUID()
  private var indexRevision = UUID()
  private var account = ""
  /// The live session for a document, on screen or still closing.
  private let session: (URL) -> DocumentSession?

  init(session: @escaping (URL) -> DocumentSession?) {
    self.session = session
    PopLibrary.shared.$accountIdentity.sink { [weak self] in self?.accountChanged($0) }.store(in: &subscriptions)
    PopLibrary.shared.$entries.sink { [weak self] in self?.libraryChanged($0) }.store(in: &subscriptions)
  }

  private func accountChanged(_ account: String) {
    guard !account.isEmpty, account != self.account, let cache else { return }
    self.account = account
    lifetime = UUID()
    indexRevision = UUID()
    for save in pending.values { save.task.cancel() }
    pending.removeAll()
    let marker = cache.root.appendingPathComponent("account")
    do {
      let previous = try? String(contentsOf: marker, encoding: .utf8)
      if previous != account {
        try cache.clear()
        WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind)
      }
      try FileManager.default.createDirectory(at: cache.root, withIntermediateDirectories: true)
      try Data(account.utf8).write(to: marker, options: .atomic)
      if account != "unavailable" {
        libraryChanged(PopLibrary.shared.entries)
        foreground()
      }
    } catch { report(error) }
  }

  private func libraryChanged(_ next: [LibraryEntry]) {
    let previous = Dictionary(uniqueKeysWithValues: entries.map { ($0.id, $0.version) })
    entries = next
    guard !account.isEmpty, account != "unavailable", let cache else { return }
    let revision = UUID()
    indexRevision = revision
    let started = lifetime
    let visible = next.filter { !$0.deleted }
    let ids = Set(visible.map(\.id))
    var removed = false
    do {
      for record in cache.records() where !ids.contains(record.id) {
        try cache.remove(libraryID: record.id)
        removed = true
      }
    } catch { report(error) }
    if removed { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
    Task {
      do {
        let index = try await DocumentArtworkRenderer.libraryIndex(entries: visible)
        guard lifetime == started, indexRevision == revision else { return }
        try cache.writeLibraryIndex(index)
      } catch { report(error) }
    }
    let changed = Set(visible.filter { previous[$0.id] != $0.version }.map(\.id))
    if UIApplication.shared.applicationState == .active && backgroundRefreshes == 0 {
      Task { await refresh(ids: changed) }
    }
  }

  func foreground() {
    guard UIApplication.shared.applicationState == .active, backgroundRefreshes == 0 else { return }
    Task { await refresh(ids: nil) }
  }

  func beginBackgroundRefresh() { backgroundRefreshes += 1 }

  func endBackgroundRefresh() { backgroundRefreshes -= 1 }

  func refreshPlaced() async { await refresh(ids: nil) }

  func saved(_ session: DocumentSession) {
    guard UIApplication.shared.applicationState == .active, backgroundRefreshes == 0 else { return }
    guard let entry = PopLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    pending[entry.id]?.task.cancel()
    let version = UUID()
    let task = Task { [weak self] in
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      // An older save woken after a newer one replaced it leaves the newer one in place.
      guard let self, pending[entry.id]?.version == version else { return }
      pending[entry.id] = nil
      await refresh(ids: [entry.id])
    }
    pending[entry.id] = (version, task)
  }

  func closing(_ session: DocumentSession) async {
    guard let entry = PopLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    pending.removeValue(forKey: entry.id)?.task.cancel()
    await refresh(ids: [entry.id])
  }

  private func placedIDs() async -> Set<String> {
    do {
      let configurations: [WidgetInfo] = try await withCheckedThrowingContinuation { continuation in
        WidgetCenter.shared.getCurrentConfigurations { continuation.resume(with: $0) }
      }
      return Set(
        configurations.filter { $0.kind == WidgetCache.kind }.compactMap {
          $0.widgetConfigurationIntent(of: PopWidgetIntent.self)?.pop?.id
        })
    } catch {
      report(error)
      return []
    }
  }

  private func refresh(ids: Set<String>?) async {
    guard !Task.isCancelled, captureAllowed else { return }
    let started = lifetime
    let placed = await placedIDs()
    guard !Task.isCancelled, captureAllowed, lifetime == started else { return }
    let requested = ids.map { placed.intersection($0) } ?? placed
    guard !requested.isEmpty else { return }
    let previous = tail
    let task = Task { [self] in
      await previous?.value
      for id in requested {
        guard !Task.isCancelled, captureAllowed else { return }
        guard lifetime == started, account != "unavailable", !account.isEmpty, let cache,
          let entry = entries.first(where: { $0.id == id && !$0.deleted })
        else { continue }
        do {
          let url = URL(fileURLWithPath: entry.path)
          let version = try await DocumentArtworkRenderer.widgetSourceVersion(url: url)
          try Task.checkCancellation()
          guard captureAllowed else { return }
          guard lifetime == started, cache.record(id: id)?.sourceVersion != version else { continue }
          let capturedAt = Date()
          let capture: DocumentWidgetCapture
          if let open = session(url) {
            guard open.isReady else { continue }
            capture = try await DocumentArtworkRenderer.widgetCapture(session: open)
          } else {
            capture = try await DocumentArtworkRenderer.widgetCapture(url: url)
          }
          try Task.checkCancellation()
          guard captureAllowed else { return }
          guard lifetime == started, let current = entries.first(where: { $0.id == id && !$0.deleted }) else {
            continue
          }
          let previousGeneration = cache.record(id: id)?.generation
          let record = try cache.publish(
            libraryID: id, title: current.title, images: capture.images, capturedAt: capturedAt,
            sourceVersion: capture.sourceVersion)
          if record.generation != previousGeneration { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
        } catch {
          if Task.isCancelled { return }
          report(error)
        }
      }
    }
    tail = task
    // A cancelled refresh stops only its own captures; a closing document's capture goes on.
    await withTaskCancellationHandler {
      await task.value
    } onCancel: {
      task.cancel()
    }
  }

  private func report(_ error: Error) {
    NSLog("pop widget refresh failed: %@", error.localizedDescription)
  }
}

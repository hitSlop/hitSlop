import Combine
import Foundation
import TinysCore
import TinysDocument
import TinysWidgets
import UIKit
import WidgetKit

@MainActor final class PhoneWidgetPublisher {
  private let cache = try? WidgetCache.shared()
  private var subscriptions: Set<AnyCancellable> = []
  private var entries: [SyncedTiny] = []
  private var pending: [String: Task<Void, Never>] = [:]
  private var tail: Task<Void, Never>?
  private var captures: [UUID: Task<Void, Never>] = [:]
  private var backgroundRefreshes = 0
  private var captureLifetime = UUID()
  private var captureAllowed: Bool {
    UIApplication.shared.applicationState == .active || backgroundRefreshes > 0
  }
  private var lifetime = UUID()
  private var indexRevision = UUID()
  private var account = ""
  private let session: () -> DocumentSession?

  init(session: @escaping () -> DocumentSession?) {
    self.session = session
    TinyLibrary.shared.$accountIdentity.sink { [weak self] in self?.accountChanged($0) }.store(in: &subscriptions)
    TinyLibrary.shared.$entries.sink { [weak self] in self?.libraryChanged($0) }.store(in: &subscriptions)
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
      if previous != account {
        try cache.clear()
        WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind)
      }
      try FileManager.default.createDirectory(at: cache.root, withIntermediateDirectories: true)
      try Data(account.utf8).write(to: marker, options: .atomic)
      if account != "unavailable" {
        libraryChanged(TinyLibrary.shared.entries)
        foreground()
      }
    } catch { report(error) }
  }

  private func libraryChanged(_ next: [SyncedTiny]) {
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

  private func cancelCaptures() {
    captureLifetime = UUID()
    for task in captures.values { task.cancel() }
    for task in pending.values { task.cancel() }
    pending.removeAll()
    tail?.cancel()
  }

  func saved(_ session: DocumentSession) {
    guard UIApplication.shared.applicationState == .active, backgroundRefreshes == 0 else { return }
    guard let entry = TinyLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    pending[entry.id]?.cancel()
    pending[entry.id] = Task { [weak self] in
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      guard let self else { return }
      pending[entry.id] = nil
      await refresh(ids: [entry.id])
    }
  }

  func closing(_ session: DocumentSession) async {
    guard let entry = TinyLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
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
          $0.widgetConfigurationIntent(of: TinyWidgetIntent.self)?.tiny?.id
        })
    } catch {
      report(error)
      return []
    }
  }

  private func refresh(ids: Set<String>?) async {
    guard !Task.isCancelled, captureAllowed else { return }
    let started = lifetime
    let captureStarted = captureLifetime
    let placed = await placedIDs()
    guard !Task.isCancelled, captureAllowed, captureLifetime == captureStarted, lifetime == started else { return }
    let requested = ids.map { placed.intersection($0) } ?? placed
    guard !requested.isEmpty else { return }
    let previous = tail
    let task = Task { [self] in
      await previous?.value
      for id in requested {
        guard !Task.isCancelled, captureAllowed, captureLifetime == captureStarted else { return }
        guard lifetime == started, account != "unavailable", !account.isEmpty, let cache,
          let entry = entries.first(where: { $0.id == id && !$0.deleted })
        else { continue }
        do {
          let url = URL(fileURLWithPath: entry.path)
          let version = try await DocumentArtworkRenderer.widgetSourceVersion(url: url)
          try Task.checkCancellation()
          guard captureAllowed, captureLifetime == captureStarted else { return }
          guard lifetime == started, cache.record(id: id)?.sourceVersion != version else { continue }
          let capturedAt = Date()
          let capture: DocumentWidgetCapture
          if let open = session(), open.file.url == url {
            guard open.isReady else { continue }
            capture = try await DocumentArtworkRenderer.widgetCapture(session: open)
          } else {
            capture = try await DocumentArtworkRenderer.widgetCapture(url: url)
          }
          try Task.checkCancellation()
          guard captureAllowed, captureLifetime == captureStarted else { return }
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
    let token = UUID()
    captures[token] = task
    defer { captures[token] = nil }
    await withTaskCancellationHandler {
      await task.value
    } onCancel: {
      task.cancel()
      Task { @MainActor [weak self] in self?.cancelCaptures() }
    }
  }

  private func report(_ error: Error) {
    NSLog("tinys widget refresh failed: %@", error.localizedDescription)
  }
}

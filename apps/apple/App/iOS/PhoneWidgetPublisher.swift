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
  private lazy var publisher = WidgetPublisher { [weak self] in self?.report($0) }
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
    publisher.invalidate()
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
    publisher.saved(key: entry.id) { [weak self] in
      await self?.refresh(ids: [entry.id])
    }
  }

  func closing(_ session: DocumentSession) async {
    guard let entry = PopLibrary.shared.entry(at: session.file.url), !entry.deleted else { return }
    publisher.cancelSave(key: entry.id)
    await refresh(ids: [entry.id])
  }

  private func refresh(ids: Set<String>?) async {
    guard !Task.isCancelled, captureAllowed else { return }
    let started = lifetime
    let placed = await PopWidgetIntent.placedDocumentIDs()
    guard !Task.isCancelled, captureAllowed, lifetime == started else { return }
    let requested = ids.map { placed.intersection($0) } ?? placed
    guard !requested.isEmpty else { return }
    await publisher.serialized { [self] in
      for id in requested {
        guard !Task.isCancelled, captureAllowed else { return }
        guard lifetime == started, account != "unavailable", !account.isEmpty, let cache,
          let entry = entries.first(where: { $0.id == id && !$0.deleted })
        else { continue }
        do {
          let url = URL(fileURLWithPath: entry.path)
          let capturedAt = Date()
          try await publisher.refresh(
            url: url, currentVersion: { cache.record(id: id)?.sourceVersion },
            valid: {
              self.captureAllowed && self.lifetime == started && self.entries.contains { $0.id == id && !$0.deleted }
            },
            render: {
              if let open = self.session(url) {
                guard open.isReady else { throw CancellationError() }
                return try await DocumentArtworkRenderer.widgetCapture(session: open)
              }
              return try await DocumentArtworkRenderer.widgetCapture(url: url)
            },
            publish: { capture in
              guard let current = self.entries.first(where: { $0.id == id && !$0.deleted }) else { return false }
              let previous = cache.record(id: id)?.generation
              let record = try cache.publish(
                libraryID: id, title: current.title, images: capture.images, capturedAt: capturedAt,
                sourceVersion: capture.sourceVersion)
              return record.generation != previous
            }, reload: { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) })
        } catch {
          if Task.isCancelled { return }
          report(error)
        }
      }
    }
  }

  private func report(_ error: Error) {
    NSLog("pop widget refresh failed: %@", error.localizedDescription)
  }
}

import Foundation
import PopCore
import PopCoreBinding
import PopDocument
import PopWidgets
import UIKit
import WidgetKit

@MainActor final class PhoneWidgetPublisher {
  private let cache = try? WidgetCache.shared()
  private var library: WidgetLibraryIndex?
  private lazy var publisher = WidgetPublisher { [weak self] in self?.report($0) }
  private var backgroundRefreshes = 0
  private var captureAllowed: Bool {
    UIApplication.shared.applicationState == .active || backgroundRefreshes > 0
  }
  /// The live session for a document, on screen or still closing.
  private let session: (URL) -> DocumentSession?

  init(session: @escaping (URL) -> DocumentSession?) {
    self.session = session
    if let cache {
      library = WidgetLibraryIndex(
        cache: cache, publisher: publisher,
        reload: { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) },
        report: { [weak self] in self?.report($0) },
        changed: { [weak self] ids in
          guard let self, UIApplication.shared.applicationState == .active, self.backgroundRefreshes == 0 else {
            return
          }
          Task { await self.refresh(ids: ids) }
        })
      library?.observe(PopLibrary.shared)
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
    guard let entry = library?.entry(at: session.file.url), !entry.deleted else { return }
    publisher.saved(key: entry.id) { [weak self] in
      await self?.refresh(ids: [entry.id])
    }
  }

  func closing(_ session: DocumentSession) async {
    guard let entry = library?.entry(at: session.file.url), !entry.deleted else { return }
    publisher.cancelSave(key: entry.id)
    await refresh(ids: [entry.id])
  }

  private func refresh(ids: Set<String>?) async {
    guard !Task.isCancelled, captureAllowed, let library else { return }
    let started = library.lifetime
    let placed = await PopWidgetIntent.placedDocumentIDs()
    guard !Task.isCancelled, captureAllowed, library.lifetime == started else { return }
    let requested = ids.map { placed.intersection($0) } ?? placed
    guard !requested.isEmpty else { return }
    await publisher.serialized { [self] in
      for id in requested {
        guard !Task.isCancelled, captureAllowed else { return }
        guard library.lifetime == started, let cache,
          let entry = library.entry(id: id), entry.downloaded
        else { continue }
        do {
          let url = URL(fileURLWithPath: entry.path)
          let capturedAt = Date()
          try await publisher.refresh(
            url: url, currentVersion: { cache.record(id: id)?.sourceVersion },
            valid: {
              self.captureAllowed && library.lifetime == started && library.entry(id: id)?.path == entry.path
            },
            render: {
              if let open = self.session(url) {
                guard open.isReady else { throw CancellationError() }
                return try await DocumentArtworkRenderer.widgetCapture(session: open)
              }
              return try await DocumentArtworkRenderer.widgetCapture(url: url)
            },
            publish: { capture in
              guard let current = library.entry(id: id) else { return false }
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

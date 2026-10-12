import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument
import HitSlopWidgets
import OSLog
import WidgetKit

/// The app's only widget writer. Scheduled captures are serialized; close never queues behind them.
@MainActor public final class SlopWidgetPublisher {
  public static let shared = SlopWidgetPublisher()
  public private(set) var enabled = false
  private var scheduler: NSBackgroundActivityScheduler?
  private var tail: Task<Void, Never>?
  private var cache: WidgetCache?
  private var openSession: (String) -> DocumentSession? = { _ in nil }
  private let logger = Logger(subsystem: "com.hitslop.app", category: "widgets")

  public func start(openSession: @escaping (String) -> DocumentSession?) {
    guard let cache = try? WidgetCache.shared() else { return }
    start(cache: cache, openSession: openSession)
  }

  func start(cache: WidgetCache, openSession: @escaping (String) -> DocumentSession?) {
    guard !enabled else { return }
    self.cache = cache
    enabled = true
    self.openSession = openSession
    let activity = NSBackgroundActivityScheduler(identifier: "com.hitslop.widget-capture")
    activity.interval = Double(widgetLimits().minRefreshMinutes) * 60
    activity.tolerance = 5 * 60
    activity.repeats = true
    activity.qualityOfService = .utility
    activity.schedule { [weak self] completion in
      Task { @MainActor in
        await self?.refreshDue()
        completion(.finished)
      }
    }
    scheduler = activity
    Task { await refreshDue() }
  }
  public func stop() {
    scheduler?.invalidate()
    scheduler = nil
    enabled = false
    cache = nil
  }

  func needsCapture(url: URL, edited: Bool) -> Bool {
    enabled && (edited || cache?.record(for: url) == nil)
  }

  public func capture(renderer: DocumentSession, url: URL, identity: String, capturedAt: Date) async {
    guard enabled, let options = renderer.file.views.widget else { return }
    do {
      let images = try await SlopRenderer.widgetImages(session: renderer)
      try publish(
        url: url, identity: identity, images: images, capturedAt: capturedAt, refreshMinutes: options.refreshMinutes)
    } catch {
      logger.error("Widget capture retained its previous image: \(error.localizedDescription, privacy: .private)")
    }
  }
  private func serialized(_ work: @escaping @MainActor () async throws -> Void) async {
    let previous = tail
    let task = Task { @MainActor [self] in
      await previous?.value
      do { try await work() } catch {
        logger.error("Widget capture retained its previous image: \(error.localizedDescription, privacy: .private)")
      }
    }
    tail = task
    await task.value
  }
  private func publish(url: URL, identity: String, images: [WidgetImage], capturedAt: Date, refreshMinutes: UInt32?)
    throws
  {
    guard try WidgetCache.identity(url) == identity else {
      throw SlopFailure("The widget document moved during capture")
    }
    guard enabled, let cache else { return }
    try cache.publish(
      url: url, title: url.deletingPathExtension().lastPathComponent,
      images: images, capturedAt: capturedAt, refreshMinutes: refreshMinutes)
    WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind)
  }
  private func refreshDue() async {
    guard enabled, let cache else { return }
    if cache.pruneUnavailable() { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
    let configurations: [WidgetInfo] = await withCheckedContinuation { continuation in
      WidgetCenter.shared.getCurrentConfigurations { continuation.resume(returning: (try? $0.get()) ?? []) }
    }
    let ids = Set(
      configurations.filter { $0.kind == WidgetCache.kind }
        .compactMap { $0.widgetConfigurationIntent(of: SlopWidgetIntent.self)?.slop?.id })
    await refresh(ids: ids)
  }

  func refresh(ids: Set<String>) async {
    for id in ids {
      guard enabled else { break }
      await serialized { [self] in
        guard enabled, let cache, let record = cache.record(id: id), let requested = record.refreshMinutes else {
          return
        }
        let limits = widgetLimits()
        let interval = min(limits.maxRefreshMinutes, max(limits.minRefreshMinutes, requested))
        guard Date().timeIntervalSince1970 - record.updatedAt >= Double(interval) * 60 else { return }
        let url = try cache.resolve(record)
        let capturedAt = Date()
        let images: [WidgetImage]
        if let session = openSession(record.fileIdentity) {
          guard session.isReady else { return }
          images = try await SlopRenderer.widgetImages(session: session)
        } else {
          images = try await SlopRenderer.widgetImages(url: url)
        }
        try publish(
          url: url, identity: record.fileIdentity, images: images, capturedAt: capturedAt, refreshMinutes: interval)
      }
    }
  }
}

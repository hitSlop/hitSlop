import Foundation
import OSLog
import PopCore
import PopCoreBinding
import PopDocument
import PopWidgets
import WidgetKit

/// The app's only widget writer. Scheduled captures are serialized; close never queues behind them.
@MainActor public final class PopWidgetPublisher {
  public static let shared = PopWidgetPublisher()
  public private(set) var enabled = false
  private var scheduler: NSBackgroundActivityScheduler?
  private var cache: WidgetCache?
  private var openSession: (URL) -> DocumentSession? = { _ in nil }
  private var activeSelections: () async -> Set<String> = { [] }
  private var watchers: [String: DispatchSourceFileSystemObject] = [:]
  private var changedCaptures: [String: Task<Void, Never>] = [:]
  private var changedVersions: [String: UUID] = [:]
  private var lifetime = UUID()
  private var library: WidgetLibraryIndex?
  private var reload: () -> Void = { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
  private let logger = Logger(subsystem: "click.dotpop", category: "widgets")
  private lazy var publisher = WidgetPublisher { [weak self] error in
    self?.logger.error("Widget capture retained its previous image: \(error.localizedDescription, privacy: .private)")
  }

  public func start(
    activeSelections: @escaping () async -> Set<String>, openSession: @escaping (URL) -> DocumentSession?
  ) {
    guard let cache = try? WidgetCache.shared() else { return }
    start(cache: cache, activeSelections: activeSelections, openSession: openSession)
    library?.observe(PopLibrary.shared)
  }

  func start(
    cache: WidgetCache, entries: [LibraryEntry] = [], activeSelections: @escaping () async -> Set<String> = { [] },
    reload: @escaping () -> Void = { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) },
    openSession: @escaping (URL) -> DocumentSession?
  ) {
    guard !enabled else { return }
    self.cache = cache
    lifetime = UUID()
    enabled = true
    self.openSession = openSession
    self.activeSelections = activeSelections
    self.reload = reload
    library = WidgetLibraryIndex(
      cache: cache, publisher: publisher, entries: entries, reload: reload,
      report: { [weak self] in self?.logger.error("Widget index failed: \($0.localizedDescription, privacy: .private)")
      },
      changed: { [weak self] _ in
        guard let self else { return }
        for watcher in self.watchers.values { watcher.cancel() }
        self.watchers.removeAll()
        Task { await self.refreshDue() }
      })
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
    activeSelections = { [] }
    library?.invalidate()
    library = nil
    for watcher in watchers.values { watcher.cancel() }
    watchers.removeAll()
    for task in changedCaptures.values { task.cancel() }
    changedCaptures.removeAll()
    changedVersions.removeAll()
    lifetime = UUID()
  }

  /// A successful save refreshes the widget even while its editor stays open.
  /// Coalesce bursts; capture still crosses the owner's flush/snapshot boundary.
  func saved(_ session: DocumentSession) {
    guard enabled, session.file.views.widget != nil, let entry = library?.entry(at: session.file.url) else { return }
    publisher.saved(key: entry.id) { [weak self, weak session] in
      guard let self, let session, session.isReady else { return }
      await self.refresh(ids: [entry.id], scheduledIDs: [])
    }
  }

  func opened(_ session: DocumentSession) {
    guard enabled, session.file.views.widget != nil, let entry = library?.entry(at: session.file.url) else { return }
    if cache?.record(id: entry.id) != nil { sourceChanged(id: entry.id) } else { saved(session) }
  }

  func needsCapture(url: URL, edited: Bool) -> Bool {
    guard enabled, let entry = library?.entry(at: url) else { return false }
    return edited || cache?.record(id: entry.id) == nil
  }

  public func capture(renderer: DocumentSession, url: URL, capturedAt: Date) async {
    guard let options = renderer.file.views.widget else { return }
    await capture(url: url, capturedAt: capturedAt, options: options) {
      try await PopRenderer.widgetCapture(session: renderer)
    }
  }

  func capture(
    url: URL, capturedAt: Date, options: WidgetOptions,
    render: () async throws -> PopRenderer.WidgetCapture
  ) async {
    guard enabled, let library, let entry = library.entry(at: url) else { return }
    let started = library.lifetime
    await publisher.capture(
      valid: { self.enabled && library.lifetime == started && library.entry(id: entry.id)?.path == entry.path },
      render: render,
      publish: { capture in
        try self.publish(
          id: entry.id, capture: capture, capturedAt: capturedAt, refreshMinutes: options.refreshMinutes)
      }, reload: reload)
  }

  private func publish(
    id: String, capture: PopRenderer.WidgetCapture, capturedAt: Date, refreshMinutes: UInt32?,
    scheduledRefresh: Bool = false
  ) throws -> Bool {
    guard enabled, let cache, let entry = library?.entry(id: id) else { return false }
    if !scheduledRefresh, cache.record(id: id)?.sourceVersion == capture.sourceVersion { return false }
    let previous = cache.record(id: id)?.generation
    let record = try cache.publish(
      libraryID: id, title: entry.title, images: capture.images, capturedAt: capturedAt,
      refreshMinutes: refreshMinutes, sourceVersion: capture.sourceVersion)
    watch(id: id, url: URL(fileURLWithPath: entry.path))
    return record.generation != previous
  }
  private func refreshDue() async {
    guard enabled else { return }
    let started = lifetime
    let entries = library?.entries ?? []
    let ids = Set(entries.map(\.id))
    for id in Array(watchers.keys) where !ids.contains(id) {
      watchers.removeValue(forKey: id)?.cancel()
    }
    for entry in entries { watch(id: entry.id, url: URL(fileURLWithPath: entry.path)) }
    let scheduledIDs = await activeSelections()
    guard enabled, lifetime == started else { return }
    await refresh(ids: ids, scheduledIDs: scheduledIDs)
  }

  func refresh(ids: Set<String>, scheduledIDs: Set<String>? = nil) async {
    let started = lifetime
    for id in ids {
      guard enabled, lifetime == started else { break }
      await publisher.serialized { [self] in
        guard enabled, lifetime == started, let library, let entry = library.entry(id: id), entry.downloaded else {
          return
        }
        let accountLifetime = library.lifetime
        let url = URL(fileURLWithPath: entry.path)
        let current = cache?.record(id: id)
        guard let options = (try? PopFile(url: url))?.views.widget else { return }
        let requested = options.refreshMinutes
        let due: Bool
        if (scheduledIDs ?? ids).contains(id), let requested {
          let limits = widgetLimits()
          let interval = min(limits.maxRefreshMinutes, max(limits.minRefreshMinutes, requested))
          due = Date().timeIntervalSince1970 - (current?.updatedAt ?? 0) >= Double(interval) * 60
        } else {
          due = false
        }
        let capturedAt = Date()
        try await publisher.refresh(
          url: url, currentVersion: { self.cache?.record(id: id)?.sourceVersion }, scheduled: due,
          valid: {
            self.enabled && self.lifetime == started && library.lifetime == accountLifetime
              && library.entry(id: id)?.path == entry.path
          },
          render: {
            if let session = self.openSession(url) {
              guard session.isReady else { throw CancellationError() }
              return try await PopRenderer.widgetCapture(session: session)
            }
            return try await PopRenderer.widgetCapture(url: url)
          },
          publish: { capture in
            try self.publish(
              id: id, capture: capture, capturedAt: capturedAt,
              refreshMinutes: requested, scheduledRefresh: due)
          }, reload: reload)
      }
    }
  }

  /// Observe saved-file activity, including writes by a closed CLI owner. The event
  /// is only a hint: the core's state/renderer token decides whether pixels need refreshing.
  private func watch(id: String, url: URL) {
    guard enabled, watchers[id] == nil else { return }
    let descriptor = open(url.path, O_EVTONLY)
    guard descriptor >= 0 else { return }
    let started = lifetime
    let source = DispatchSource.makeFileSystemObjectSource(
      fileDescriptor: descriptor, eventMask: [.write, .extend, .rename, .delete, .revoke], queue: .main)
    source.setEventHandler { [weak self, weak source] in
      guard let self, self.enabled, self.lifetime == started else { return }
      if let source, !source.data.isDisjoint(with: [.rename, .delete, .revoke]) {
        source.cancel()
        self.watchers[id] = nil
      }
      self.sourceChanged(id: id)
    }
    source.setCancelHandler { close(descriptor) }
    watchers[id] = source
    source.resume()
    // Close the gap between acquiring the rendered snapshot and installing its
    // first watcher. A save in that gap must not leave the published image stale.
    sourceChanged(id: id)
  }

  private func sourceChanged(id: String) {
    changedCaptures[id]?.cancel()
    let started = lifetime
    let version = UUID()
    changedVersions[id] = version
    changedCaptures[id] = Task { @MainActor [weak self] in
      defer {
        if let self, self.changedVersions[id] == version {
          self.changedCaptures[id] = nil
          self.changedVersions[id] = nil
        }
      }
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      guard let self, self.enabled, self.lifetime == started else { return }
      await self.refresh(ids: [id], scheduledIDs: [])
    }
  }
}

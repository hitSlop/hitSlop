import Foundation
import OSLog
import TinysCore
import TinysCoreBinding
import TinysDocument
import TinysWidgets
import WidgetKit

/// The app's only widget writer. Scheduled captures are serialized; close never queues behind them.
@MainActor public final class TinyWidgetPublisher {
  public static let shared = TinyWidgetPublisher()
  public private(set) var enabled = false
  private var scheduler: NSBackgroundActivityScheduler?
  private var tail: Task<Void, Never>?
  private var cache: WidgetCache?
  private var openSession: (String) -> DocumentSession? = { _ in nil }
  private var activeSelections: () async -> Set<String> = { [] }
  private var savedCaptures: [String: Task<Void, Never>] = [:]
  private var savedVersions: [String: UUID] = [:]
  private var watchers: [String: DispatchSourceFileSystemObject] = [:]
  private var changedCaptures: [String: Task<Void, Never>] = [:]
  private var changedVersions: [String: UUID] = [:]
  private var lifetime = UUID()
  var refreshLocation: (WidgetCache, String, URL) throws -> Bool = { try $0.refreshLocation(id: $1, url: $2) }
  private var reload: () -> Void = { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) }
  private let logger = Logger(subsystem: "com.hitslop.app", category: "widgets")

  public func start(
    activeSelections: @escaping () async -> Set<String>, openSession: @escaping (String) -> DocumentSession?
  ) {
    guard let cache = try? WidgetCache.shared() else { return }
    start(cache: cache, activeSelections: activeSelections, openSession: openSession)
  }

  func start(
    cache: WidgetCache, activeSelections: @escaping () async -> Set<String> = { [] },
    reload: @escaping () -> Void = { WidgetCenter.shared.reloadTimelines(ofKind: WidgetCache.kind) },
    openSession: @escaping (String) -> DocumentSession?
  ) {
    guard !enabled else { return }
    self.cache = cache
    lifetime = UUID()
    enabled = true
    self.openSession = openSession
    self.activeSelections = activeSelections
    self.reload = reload
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
    for task in savedCaptures.values { task.cancel() }
    savedCaptures.removeAll()
    savedVersions.removeAll()
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
    guard enabled, session.file.views.widget != nil,
      let identity = try? WidgetCache.identity(session.file.url)
    else { return }
    savedCaptures[identity]?.cancel()
    let version = UUID()
    savedVersions[identity] = version
    savedCaptures[identity] = Task { @MainActor [weak self, weak session] in
      defer {
        if let self, self.savedVersions[identity] == version {
          self.savedCaptures[identity] = nil
          self.savedVersions[identity] = nil
        }
      }
      do { try await Task.sleep(for: .seconds(1)) } catch { return }
      guard let self, let session, self.enabled, session.isReady else { return }
      await self.serialized { [self, weak session] in
        guard savedVersions[identity] == version, let session, session.isReady else { return }
        if let record = cache?.record(for: session.file.url),
          let sourceVersion = try? await TinyRenderer.widgetSourceVersion(url: session.file.url),
          record.sourceVersion == sourceVersion
        {
          return
        }
        guard savedVersions[identity] == version, enabled, session.isReady else { return }
        await capture(renderer: session, url: session.file.url, identity: identity, capturedAt: Date())
      }
    }
  }

  func opened(_ session: DocumentSession) {
    guard enabled, session.file.views.widget != nil else { return }
    if let record = cache?.record(for: session.file.url) {
      sourceChanged(id: record.id)
    } else {
      saved(session)
    }
  }

  func needsCapture(url: URL, edited: Bool) -> Bool {
    enabled && (edited || cache?.record(for: url) == nil)
  }

  public func capture(renderer: DocumentSession, url: URL, identity: String, capturedAt: Date) async {
    guard enabled, let options = renderer.file.views.widget else { return }
    await capture(url: url, identity: identity, capturedAt: capturedAt, options: options) {
      try await TinyRenderer.widgetCapture(session: renderer)
    }
  }

  func capture(
    url: URL, identity: String, capturedAt: Date, options: WidgetOptions,
    render: () async throws -> TinyRenderer.WidgetCapture
  ) async {
    guard enabled else { return }
    let started = lifetime
    do {
      let capture = try await render()
      try Task.checkCancellation()
      guard lifetime == started else { return }
      try publish(
        url: url, identity: identity, capture: capture, capturedAt: capturedAt, refreshMinutes: options.refreshMinutes)
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
  private func publish(
    url: URL, identity: String, capture: TinyRenderer.WidgetCapture, capturedAt: Date, refreshMinutes: UInt32?
  )
    throws
  {
    guard try WidgetCache.identity(url) == identity else {
      throw TinyFailure("The widget document moved during capture")
    }
    guard enabled, let cache else { return }
    let previous = cache.record(for: url)?.generation
    let record = try cache.publish(
      url: url, title: url.deletingPathExtension().lastPathComponent,
      images: capture.images, capturedAt: capturedAt, refreshMinutes: refreshMinutes,
      sourceVersion: capture.sourceVersion)
    watch(record)
    // A refused stale capture did not publish and must not consume a reload request.
    if record.generation != previous { reload() }
  }
  private func refreshDue() async {
    guard enabled, let cache else { return }
    let started = lifetime
    if cache.pruneUnavailable() { reload() }
    let records = cache.records()
    let ids = Set(records.map(\.id))
    for id in Array(watchers.keys) where !ids.contains(id) {
      watchers.removeValue(forKey: id)?.cancel()
    }
    for record in records { watch(record) }
    let scheduledIDs = await activeSelections()
    guard enabled, lifetime == started else { return }
    await refresh(ids: ids, scheduledIDs: scheduledIDs)
  }

  func refresh(ids: Set<String>, scheduledIDs: Set<String>? = nil) async {
    let started = lifetime
    for id in ids {
      guard enabled, lifetime == started else { break }
      await serialized { [self] in
        guard enabled, lifetime == started, let cache, let record = cache.record(id: id),
          let identity = record.fileIdentity
        else { return }
        let url = try cache.resolve(record)
        // Location is independent of rendered content, and must catch up even when the
        // source token is unchanged or a subsequent capture fails.
        do {
          if try refreshLocation(cache, id, url) { reload() }
        } catch {
          logger.error("Widget location could not be refreshed: \(error.localizedDescription, privacy: .private)")
        }
        watch(record)
        let version = try await TinyRenderer.widgetSourceVersion(url: url)
        guard enabled, lifetime == started else { return }
        let current = cache.record(id: id) ?? record
        let due: Bool
        if (scheduledIDs ?? ids).contains(id), let requested = current.refreshMinutes {
          let limits = widgetLimits()
          let interval = min(limits.maxRefreshMinutes, max(limits.minRefreshMinutes, requested))
          due = Date().timeIntervalSince1970 - current.updatedAt >= Double(interval) * 60
        } else {
          due = false
        }
        guard current.sourceVersion != version || due else { return }
        let capturedAt = Date()
        let capture: TinyRenderer.WidgetCapture
        if let session = openSession(identity) {
          guard session.isReady else { return }
          capture = try await TinyRenderer.widgetCapture(session: session)
        } else {
          capture = try await TinyRenderer.widgetCapture(url: url)
        }
        guard lifetime == started else { return }
        try publish(
          url: url, identity: identity, capture: capture, capturedAt: capturedAt,
          refreshMinutes: current.refreshMinutes)
      }
    }
  }

  /// Observe saved-file activity, including writes by a closed CLI owner. The event
  /// is only a hint: the core's state/renderer token decides whether pixels need refreshing.
  private func watch(_ record: WidgetRecord) {
    guard enabled, watchers[record.id] == nil, let cache, let url = try? cache.resolve(record) else { return }
    let descriptor = open(url.path, O_EVTONLY)
    guard descriptor >= 0 else { return }
    let started = lifetime
    let source = DispatchSource.makeFileSystemObjectSource(
      fileDescriptor: descriptor, eventMask: [.write, .extend, .rename, .delete, .revoke], queue: .main)
    source.setEventHandler { [weak self, weak source] in
      guard let self, self.enabled, self.lifetime == started else { return }
      if let source, !source.data.isDisjoint(with: [.rename, .delete, .revoke]) {
        source.cancel()
        self.watchers[record.id] = nil
      }
      self.sourceChanged(id: record.id)
    }
    source.setCancelHandler { close(descriptor) }
    watchers[record.id] = source
    source.resume()
    // Close the gap between acquiring the rendered snapshot and installing its
    // first watcher. A save in that gap must not leave the published image stale.
    sourceChanged(id: record.id)
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

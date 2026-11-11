import Foundation
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

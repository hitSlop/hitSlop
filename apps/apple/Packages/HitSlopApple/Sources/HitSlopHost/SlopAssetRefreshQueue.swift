import Foundation
import HitSlopCore
import HitSlopDocument

/// Refreshes a closed document's artwork from its saved state, one render at a time. Quitting
/// never starts another render: the refreshes it leaves are remembered and resume at the
/// next launch, so Finder and Quick Look never keep stale artwork.
@MainActor enum SlopDocumentAssetRefreshQueue {
  private struct Job {
    let destination: URL
    let generation: UUID
    let telemetry: SlopTelemetry
  }
  private static var generations: [URL: UUID] = [:]
  private static var pending: [Job] = []
  private static var worker: Task<Void, Never>?
  private static var rendering: URL?
  /// The documents whose refresh a quit left unfinished.
  static let unfinishedKey = "HitSlopUnfinishedArtworkRefreshes"
  /// Resumed when the worker exits; quitting waits on it.
  private static var drained: CheckedContinuation<Void, Never>?

  static func invalidate(_ url: URL) {
    let key = url.standardizedFileURL
    generations[key] = UUID()
    pending.removeAll { $0.destination == key }
  }
  static func schedule(presentedURL: URL, telemetry: SlopTelemetry = .disabled) {
    let key = presentedURL.standardizedFileURL
    invalidate(key)
    let generation = generations[key]!
    pending.append(Job(destination: key, generation: generation, telemetry: telemetry))
    startWorkerIfNeeded()
  }
  private static func startWorkerIfNeeded() {
    guard worker == nil, !pending.isEmpty else { return }
    worker = Task { @MainActor in
      defer {
        rendering = nil
        worker = nil
        drained?.resume()
        drained = nil
        startWorkerIfNeeded()
      }
      while !pending.isEmpty, !Task.isCancelled {
        let job = pending.removeFirst()
        rendering = job.destination
        defer { rendering = nil }
        do {
          let assets = try await SlopRenderer.documentAssetsPNGData(url: job.destination, telemetry: job.telemetry)
          guard !Task.isCancelled, generations[job.destination] == job.generation else { continue }
          SlopPreviewWriter.writeRendered(
            preview: assets.previewPNG, icon: assets.finderIconPNG, marker: assets.marker,
            to: job.destination, telemetry: job.telemetry)
          remember(unfinished().filter { $0 != job.destination })
        } catch {
          if !Task.isCancelled, generations[job.destination] == job.generation { job.telemetry.failure(.artwork, error: error) }
        }
      }
    }
  }
  /// The refreshes a quit left unfinished, oldest first.
  private static func unfinished() -> [URL] {
    (UserDefaults.standard.stringArray(forKey: unfinishedKey) ?? []).map { URL(fileURLWithPath: $0) }
  }
  private static func remember(_ urls: [URL]) {
    let paths = Array(NSOrderedSet(array: urls.map(\.path))) as? [String] ?? []
    if paths.isEmpty { UserDefaults.standard.removeObject(forKey: unfinishedKey) }
    else { UserDefaults.standard.set(paths, forKey: unfinishedKey) }
  }
  /// Schedules the refreshes the last quit left unfinished.
  static func resume(telemetry: SlopTelemetry = .disabled) {
    let urls = unfinished()
    remember([])
    for url in urls where FileManager.default.fileExists(atPath: url.path) {
      schedule(presentedURL: url, telemetry: telemetry)
    }
  }
  static func finishForTermination(grace: Duration = .seconds(5)) async {
    // Do not start another WebView while quitting. Its synchronous construction
    // could consume the entire grace period before the timer gets to run. What is left is
    // remembered for the next launch; a render that finishes meanwhile forgets its own.
    remember(unfinished() + pending.map(\.destination) + [rendering].compactMap { $0 })
    pending.removeAll()
    if rendering == nil {
      worker?.cancel()
      generations.removeAll()
      return
    }
    // The render in flight finishes, or the grace period ends, whichever is first.
    await withCheckedContinuation { continuation in
      drained = continuation
      Task { @MainActor in
        try? await Task.sleep(for: grace)
        drained?.resume()
        drained = nil
      }
    }
    if worker != nil {
      worker?.cancel()
      pending.removeAll()
      generations.removeAll()
    }
  }
}


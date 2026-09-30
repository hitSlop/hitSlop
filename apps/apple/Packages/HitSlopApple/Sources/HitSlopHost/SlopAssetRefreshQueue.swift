import Foundation
import HitSlopCore
import HitSlopDocument

@MainActor enum SlopDocumentAssetRefreshQueue {
  private struct Job {
    let destination: URL
    let generation: UUID
    let telemetry: SlopTelemetry
  }
  private static var generations: [URL: UUID] = [:]
  private static var pending: [Job] = []
  private static var worker: Task<Void, Never>?
  private static var rendering = false
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
        rendering = false
        worker = nil
        drained?.resume()
        drained = nil
        startWorkerIfNeeded()
      }
      while !pending.isEmpty, !Task.isCancelled {
        let job = pending.removeFirst()
        rendering = true
        defer { rendering = false }
        do {
          let assets = try await SlopRenderer.documentAssetsPNGData(packageURL: job.destination, telemetry: job.telemetry)
          guard !Task.isCancelled, generations[job.destination] == job.generation else { continue }
          if let preview = assets.previewPNG {
            do { try SlopPreviewWriter.write(preview, to: job.destination) } catch {
              job.telemetry.failure(.artwork, error: error)
            }
          }
          if let icon = assets.finderIconPNG {
            SlopPreviewWriter.installFinderIcon(icon, for: job.destination, telemetry: job.telemetry)
          }
        } catch {
          if !Task.isCancelled, generations[job.destination] == job.generation { job.telemetry.failure(.artwork, error: error) }
        }
      }
    }
  }
  static func finishForTermination(grace: Duration = .seconds(5)) async {
    // Do not start another WebView while quitting. Its synchronous construction
    // could consume the entire grace period before the timer gets to run.
    pending.removeAll()
    if !rendering {
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


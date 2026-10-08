import Foundation

/// Bounded blocking work (filesystem, SQLite opening, synchronous engine setup)
/// runs here, outside both the UI actor and Swift's cooperative thread pool.
public enum SlopPreparation {
  /// Opening documents, at the priority of a person waiting for a window.
  public static let documents = DispatchQueue(label: "com.hitslop.preparation", qos: .userInitiated)
  /// The catalog's file work, away from document opening.
  public static let catalog = DispatchQueue(label: "hitslop.catalog", qos: .utility)

  /// Runs `work` on `queue`, off the main actor.
  public static func run<T: Sendable>(on queue: DispatchQueue = documents, _ work: @escaping @Sendable () throws -> T)
    async throws -> T
  {
    try Task.checkCancellation()
    // Always deliver resources produced by work, even after cancellation. The
    // caller owns disposing them before propagating cancellation.
    return try await withCheckedThrowingContinuation { continuation in
      queue.async { continuation.resume(with: Result { try work() }) }
    }
  }
}

import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// One saved source and one serial capture queue. Rust owns rendering and publication.
@MainActor public enum DocumentArtworkRenderer {
  private static let queue = DispatchQueue(label: "com.hitslop.document-artwork", qos: .utility)
  private static let runtime: Result<CaptureRuntime, Error> = Result {
    #if os(iOS)
      guard let path = Bundle.main.executableURL?.path else { throw SlopFailure("Artwork renderer is unavailable") }
    #else
      guard let path = DocumentOwner.evaluatorPath else { throw SlopFailure("Artwork renderer is unavailable") }
    #endif
    return try CaptureRuntime(evaluatorPath: path)
  }

  @discardableResult public static func refresh(session: DocumentSession) async throws -> Bool {
    guard session.isReady else { return false }
    let entry = SlopLibrary.shared.entry(at: session.file.url)
    let stale = entry.map { $0.artworkVersion != $0.version } ?? false
    let missing = await session.artwork(.preview) == nil
    guard session.edited || stale || missing else { return false }
    let renderer = try runtime.get()
    return try await session.withCaptureSnapshot { source in
      let artwork = try await SlopPreparation.run(on: queue) { try renderer.renderArtwork(path: source.path) }
      try Task.checkCancellation()
      guard artwork.preview != nil || artwork.icon != nil else { return false }
      try await session.owner.installArtwork(artwork)
      return true
    }
  }
}

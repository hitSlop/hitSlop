import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopWidgets
import ImageIO
import UniformTypeIdentifiers

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

  public static func libraryIndex(entries: [SyncedSlop]) async throws -> [WidgetLibraryEntry] {
    try await SlopPreparation.run(on: queue) {
      entries.filter { !$0.deleted }.map { entry in
        var icon: Data?
        if let png = SlopArtwork.png(URL(fileURLWithPath: entry.path), .icon),
          let source = CGImageSourceCreateWithData(png as CFData, nil),
          let image = CGImageSourceCreateThumbnailAtIndex(
            source, 0,
            [
              kCGImageSourceCreateThumbnailFromImageAlways: true,
              kCGImageSourceThumbnailMaxPixelSize: 64,
            ] as CFDictionary)
        {
          let data = NSMutableData()
          if let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) {
            CGImageDestinationAddImage(destination, image, nil)
            if CGImageDestinationFinalize(destination) { icon = data as Data }
          }
        }
        return WidgetLibraryEntry(id: entry.id, title: entry.title, icon: icon)
      }
    }
  }

  public static func widgetSourceVersion(url: URL) async throws -> String {
    let renderer = try runtime.get()
    return try await SlopPreparation.run(on: queue) { try renderer.savedWidgetVersion(path: url.path) }
  }
  public static func widgetCapture(session: DocumentSession) async throws -> DocumentWidgetCapture {
    try await session.withCaptureSnapshot { try await widgetCapture(url: $0) }
  }
  public static func widgetCapture(url: URL) async throws -> DocumentWidgetCapture {
    let renderer = try runtime.get()
    let results = try await SlopPreparation.run(on: queue) {
      try renderer.renderSaved(path: url.path, outputs: [.widgets])
    }
    try Task.checkCancellation()
    return try DocumentWidgetCapture(results[0].result)
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

public struct DocumentWidgetCapture: Sendable {
  public let sourceVersion: String
  public let images: [WidgetImage]
  public init(_ result: RenderedOutput) throws {
    switch result {
    case .widgets(let capture):
      sourceVersion = capture.sourceVersion
      images = capture.images.map { image in
        let family: WidgetImageFamily
        switch image.family {
        case .small: family = .small
        case .medium: family = .medium
        case .large: family = .large
        }
        return WidgetImage(family: family, png: Data(image.png))
      }
    case .failed(let failure): throw CoreError.Failure(failure: failure)
    case .image: throw SlopFailure("Unexpected capture result")
    }
  }
}

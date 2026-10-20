import AppKit
import HitSlopCore
import HitSlopFeatures
import SwiftUI

/// Rounded white tile holding a template or document icon.
struct IconTile: View {
  let artwork: [CatalogArtwork]
  let size: CGFloat
  let radius: CGFloat

  var body: some View {
    CatalogImageView(artwork: artwork, fallback: .applicationIcon)
      .padding(size * 0.1)
      .frame(width: size, height: size)
      .background(.white, in: RoundedRectangle(cornerRadius: radius, style: .continuous))
      .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(CatalogStyle.hairline))
      .shadow(color: .black.opacity(0.06), radius: 1.5, y: 1)
  }
}

enum CatalogImageFallback { case applicationIcon, preview }

struct CatalogImageView: View {
  let artwork: [CatalogArtwork]
  let fallback: CatalogImageFallback
  /// The image loaded for `artwork`; a different artwork list shows its cached image or
  /// the placeholder, never the previous entry's image.
  @State private var loaded: (artwork: [CatalogArtwork], image: NSImage)?
  private let maxPreviewWidth: CGFloat = 560
  private let maxPreviewHeight: CGFloat = 420
  /// Decoded pixels: enough for a preview pane or an icon tile on a 2x display.
  private var maxPixelSize: Int { fallback == .preview ? 1200 : 256 }

  var body: some View {
    let shown = loaded?.artwork == artwork ? loaded?.image : cachedCatalogImage(artwork, maxPixelSize: maxPixelSize)
    Group {
      if let image = shown {
        let preview = Image(nsImage: image).resizable().interpolation(.high).scaledToFit()
        if fallback == .preview {
          preview
            .frame(
              maxWidth: min(maxPreviewWidth, image.size.width), maxHeight: min(maxPreviewHeight, image.size.height)
            )
            .shadow(color: .black.opacity(0.12), radius: 12, y: 6)
        } else {
          preview
        }
      } else if fallback == .applicationIcon {
        Image(nsImage: NSApplication.shared.applicationIconImage).resizable().interpolation(.high).scaledToFit()
      } else {
        VStack(spacing: 8) {
          Image(systemName: "photo").font(.title2)
          Text("Preview unavailable").font(.caption.weight(.medium))
        }
        .foregroundStyle(.tertiary)
        .frame(maxWidth: .infinity, minHeight: 180)
      }
    }
    .task(id: artwork) {
      for file in artwork {
        let image = await loadCatalogImage(file, maxPixelSize: maxPixelSize)
        guard !Task.isCancelled else { return }
        if let image {
          loaded = (artwork, image)
          return
        }
      }
      loaded = nil
    }
  }
}

/// Catalog artwork keyed by file, artwork, modification date, byte count and decoded size,
/// so refreshed previews reload while reselecting reuses decoded images.
@MainActor private let catalogImages: NSCache<NSString, NSImage> = {
  let cache = NSCache<NSString, NSImage>()
  cache.totalCostLimit = 128 * 1024 * 1024
  return cache
}()

private func catalogImageKey(_ artwork: CatalogArtwork, maxPixelSize: Int) -> NSString? {
  guard artwork.file.isFileURL else { return nil }
  let stamp: String
  if let revision = artwork.revision {
    stamp = revision
  } else if let modified = artwork.modifiedAt, let bytes = artwork.byteCount {
    stamp = "\(modified.timeIntervalSinceReferenceDate)|\(bytes)"
  } else {
    return nil
  }
  return "\(artwork.file.absoluteString)|\(artwork.name.rawValue)|\(stamp)|\(maxPixelSize)" as NSString
}

@MainActor private func cachedCatalogImage(_ artwork: [CatalogArtwork], maxPixelSize: Int) -> NSImage? {
  for file in artwork {
    if let key = catalogImageKey(file, maxPixelSize: maxPixelSize), let image = catalogImages.object(forKey: key) {
      return image
    }
  }
  return nil
}

/// Decodes a downsampled image off the main thread.
@MainActor private func loadCatalogImage(_ artwork: CatalogArtwork, maxPixelSize: Int) async -> NSImage? {
  guard let key = catalogImageKey(artwork, maxPixelSize: maxPixelSize) else { return nil }
  if let cached = catalogImages.object(forKey: key) { return cached }
  let (file, name) = (artwork.file, artwork.name)
  let decoded = await Task.detached(priority: .userInitiated) { () -> CGImage? in
    guard let png = SlopArtwork.png(file, name),
      let source = CGImageSourceCreateWithData(png as CFData, nil)
    else { return nil }
    return CGImageSourceCreateThumbnailAtIndex(
      source, 0,
      [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceShouldCacheImmediately: true,
        kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
      ] as CFDictionary)
  }.value
  guard !Task.isCancelled, let decoded else { return nil }
  let image = NSImage(cgImage: decoded, size: NSSize(width: decoded.width, height: decoded.height))
  catalogImages.setObject(image, forKey: key, cost: decoded.bytesPerRow * decoded.height)
  return image
}

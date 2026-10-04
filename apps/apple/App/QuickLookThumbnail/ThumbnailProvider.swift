import AppKit
import HitSlopCore
import QuickLookThumbnailing

/// Finder and the share sheet's icon for a `.slop`: the document's icon artwork, or its
/// preview when it has no icon. A file without artwork, or one mid-recovery, gets the
/// `.slop` document icon.
final class ThumbnailProvider: QLThumbnailProvider {
  override func provideThumbnail(
    for request: QLFileThumbnailRequest, _ handler: @escaping (QLThumbnailReply?, Error?) -> Void
  ) {
    let artwork: Data?
    do { artwork = try SlopArtwork.first(request.fileURL, [.icon, .preview])?.png } catch {
      // Busy or mid-recovery: Quick Look asks again later.
      handler(nil, error)
      return
    }
    guard let png = artwork,
      let source = CGImageSourceCreateWithData(png as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
      handler(nil, CocoaError(.fileReadCorruptFile))
      return
    }
    let scale = min(request.maximumSize.width / CGFloat(image.width), request.maximumSize.height / CGFloat(image.height), 1)
    let size = CGSize(width: CGFloat(image.width) * scale, height: CGFloat(image.height) * scale)
    handler(QLThumbnailReply(contextSize: size) { context in
      context.interpolationQuality = .high
      context.draw(image, in: CGRect(origin: .zero, size: size))
      return true
    }, nil)
  }
}

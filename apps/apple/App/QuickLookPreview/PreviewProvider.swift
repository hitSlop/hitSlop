import Cocoa
import HitSlopCore
import ImageIO
import QuickLookUI
import UniformTypeIdentifiers

/// The spacebar preview of a `.slop`: the document as it last rendered (its preview
/// artwork), or its icon when it has none. Nothing runs the app or opens it for writing.
final class PreviewProvider: QLPreviewProvider, QLPreviewingController {
  func providePreview(for request: QLFilePreviewRequest) async throws -> QLPreviewReply {
    guard let png = try SlopArtwork.first(request.fileURL, [.preview, .icon])?.png,
      let source = CGImageSourceCreateWithData(png as CFData, nil),
      let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
      let width = properties[kCGImagePropertyPixelWidth] as? Int,
      let height = properties[kCGImagePropertyPixelHeight] as? Int
    else { throw CocoaError(.fileReadCorruptFile) }
    return QLPreviewReply(dataOfContentType: .png, contentSize: CGSize(width: width, height: height)) { _ in png }
  }
}

import AppKit
import Foundation
import HitSlopDocument

extension Notification.Name {
  public static let hitSlopPreviewDidChange = Notification.Name("com.hitslop.preview-did-change")
}

/// A document's artwork lives in its file (`artwork`): the build's preview and icon, and
/// what its window rendered as it last closed. Finder and Quick Look show it through the
/// app's Quick Look extensions, and the catalog reads the rows.
@MainActor public enum SlopPreviewWriter {
  /// Tells Finder and the catalog that the artwork of the document at `url` changed, and
  /// copies it into the file's Finder icon.
  public static func announce(_ url: URL) {
    SlopFinderIcon.refresh(url)
    NSWorkspace.shared.noteFileSystemChanged(url.path)
    NotificationCenter.default.post(name: .hitSlopPreviewDidChange, object: url.standardizedFileURL)
  }
}

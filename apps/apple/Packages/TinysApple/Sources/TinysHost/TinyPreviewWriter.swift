import AppKit
import Foundation
import TinysDocument

extension Notification.Name {
  public static let tinysPreviewDidChange = Notification.Name("com.hitslop.preview-did-change")
}

/// A document's artwork lives in its file (`artwork`): the build's preview and icon, and
/// what its window rendered as it last closed. Finder and Quick Look show it through the
/// app's Quick Look extensions, and the catalog reads the rows.
@MainActor public enum TinyPreviewWriter {
  /// Tells Finder and the catalog that the artwork of the document at `url` changed, and
  /// copies it into the file's Finder icon.
  public static func announce(_ url: URL) {
    TinyFinderIcon.refresh(url)
    NSWorkspace.shared.noteFileSystemChanged(url.path)
    NotificationCenter.default.post(name: .tinysPreviewDidChange, object: url.standardizedFileURL)
  }
}

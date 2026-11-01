import AppKit
import Foundation
import PopDocument

extension Notification.Name {
  public static let popPreviewDidChange = Notification.Name("click.dotpop.preview-did-change")
}

/// A document's artwork lives in its file (`artwork`): the build's preview and icon, and
/// what its window rendered as it last closed. Finder and Quick Look show it through the
/// app's Quick Look extensions, and the catalog reads the rows.
@MainActor public enum PopPreviewWriter {
  /// Tells Finder and the catalog that the artwork of the document at `url` changed, and
  /// copies it into the file's Finder icon.
  public static func announce(_ url: URL) {
    PopFinderIcon.refresh(url)
    NSWorkspace.shared.noteFileSystemChanged(url.path)
    NotificationCenter.default.post(name: .popPreviewDidChange, object: url.standardizedFileURL)
  }
}

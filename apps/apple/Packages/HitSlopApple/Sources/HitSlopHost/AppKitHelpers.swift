import AppKit
import HitSlopCore

extension NSSavePanel {
  /// Uses a writable local folder, including when Desktop is synced to iCloud.
  public func startInLocalDocuments() throws {
    directoryURL = try SlopSaveLocation.folder()
  }
}

extension NSPasteboard {
  /// Replaces the pasteboard's contents with `text`.
  public func copy(_ text: String) {
    clearContents()
    setString(text, forType: .string)
  }
}

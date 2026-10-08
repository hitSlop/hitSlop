import AppKit

extension NSSavePanel {
  /// Opens in the user's Desktop folder (open panels too, which inherit this).
  public func startOnDesktop() {
    directoryURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
  }
}

extension NSPasteboard {
  /// Replaces the pasteboard's contents with `text`.
  public func copy(_ text: String) {
    clearContents()
    setString(text, forType: .string)
  }
}

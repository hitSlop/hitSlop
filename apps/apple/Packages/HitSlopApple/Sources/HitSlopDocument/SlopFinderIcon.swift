import AppKit
import HitSlopCore

/// Finder's custom icon for a document: a local copy of the artwork in its file (the icon,
/// or the preview when it has none), so Finder shows it without Quick Look's white tile.
/// The file's artwork stays the source; tools that drop file metadata lose only this copy,
/// and Finder falls back to the Quick Look thumbnail.
public enum SlopFinderIcon {
  /// Copies `url`'s artwork into its custom icon. A file without artwork is left as it is.
  public static func refresh(_ url: URL) {
    guard let png = try? SlopArtwork.first(url, [.icon, .preview])?.png, let image = NSImage(data: png) else { return }
    NSWorkspace.shared.setIcon(image, forFile: url.path, options: [])
  }
}

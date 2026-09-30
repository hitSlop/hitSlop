import Foundation

/// Path containment on fully resolved paths, so a symlink or `..` cannot step outside.
public enum SlopPath {
  /// Whether `url` is `root` itself or inside it.
  public static func contains(_ root: URL, _ url: URL) -> Bool {
    let root = root.standardizedFileURL.resolvingSymlinksInPath().path
    let path = url.standardizedFileURL.resolvingSymlinksInPath().path
    return path == root || path.hasPrefix(root.hasSuffix("/") ? root : root + "/")
  }
}

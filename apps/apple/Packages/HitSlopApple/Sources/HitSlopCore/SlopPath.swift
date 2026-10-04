import Foundation

/// File identity on fully resolved paths, so a symlink or `..` cannot disguise a file.
public enum SlopPath {
  /// `url` with `.` and `..` removed and every symbolic link resolved.
  public static func canonical(_ url: URL) -> URL {
    url.standardizedFileURL.resolvingSymlinksInPath()
  }
  /// Whether `a` and `b` name the same file.
  public static func same(_ a: URL, _ b: URL) -> Bool {
    canonical(a).path == canonical(b).path
  }
}

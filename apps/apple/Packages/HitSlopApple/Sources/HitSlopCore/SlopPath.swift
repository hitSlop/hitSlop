import Foundation

/// Fully resolved paths, so a symlink or `..` cannot disguise a file.
public enum SlopPath {
  /// `url` with `.` and `..` removed and every symbolic link resolved.
  public static func canonical(_ url: URL) -> URL {
    url.standardizedFileURL.resolvingSymlinksInPath()
  }
}

#if os(macOS)
  import Foundation

  /// The part of where a document may live that only Foundation can tell: whether a folder
  /// iCloud syncs (Desktop and Documents, for instance) holds it. The core refuses the rest,
  /// iCloud Drive's own folder included, wherever a document is created, copied or opened
  /// for writing.
  public enum TinyLocalDocument {
    /// Refuses `url`, which need not exist yet, when a folder above it is synced. Aliases
    /// are resolved first.
    public static func requireLocal(_ url: URL) throws {
      var current = resolvingExistingAncestors(url)
      let cloud = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Mobile Documents")
      let cloudPath = resolvingExistingAncestors(cloud).path.lowercased()
      let path = current.path.lowercased()
      if path == cloudPath || path.hasPrefix(cloudPath + "/") {
        throw TinyFailure("Keep active documents outside iCloud. Move this document to ~/tinys to edit it locally.")
      }
      while current.path != "/" {
        if (try? current.resourceValues(forKeys: [.isUbiquitousItemKey]).isUbiquitousItem) == true {
          throw TinyFailure("Keep active documents outside iCloud. Move this document to ~/tinys to edit it locally.")
        }
        current.deleteLastPathComponent()
      }
    }

    private static func resolvingExistingAncestors(_ url: URL) -> URL {
      var ancestor = url.standardizedFileURL
      var missing: [String] = []
      while !FileManager.default.fileExists(atPath: ancestor.path), ancestor.path != "/" {
        missing.append(ancestor.lastPathComponent)
        ancestor.deleteLastPathComponent()
      }
      return missing.reversed().reduce(ancestor.resolvingSymlinksInPath()) { $0.appendingPathComponent($1) }
    }
  }

#endif

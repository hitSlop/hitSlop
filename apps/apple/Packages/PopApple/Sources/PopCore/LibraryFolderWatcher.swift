import Foundation

/// One library watcher uses the same vnode notifications as saved widget captures.
/// Watch both directories and existing files: SQLite can modify a retained WAL in place.
@MainActor final class LibraryFolderWatcher {
  private let root: URL
  private let changed: @MainActor () -> Void
  private var sources: [String: DispatchSourceFileSystemObject] = [:]

  init(root: URL, changed: @escaping @MainActor () -> Void) {
    self.root = root
    self.changed = changed
    refresh()
  }

  func refresh() {
    var paths = Set([root.path])
    if let files = FileManager.default.enumerator(
      at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles])
    {
      for case let url as URL in files {
        if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
          || url.pathExtension == "pop" || url.lastPathComponent.hasPrefix("index.sqlite")
          || url.lastPathComponent.hasSuffix(".pop-wal")
        {
          paths.insert(url.path)
        }
      }
    }
    for path in sources.keys where !paths.contains(path) { sources.removeValue(forKey: path)?.cancel() }
    for path in paths where sources[path] == nil {
      let descriptor = open(path, O_EVTONLY)
      guard descriptor >= 0 else { continue }
      let source = DispatchSource.makeFileSystemObjectSource(
        fileDescriptor: descriptor, eventMask: [.write, .extend, .rename, .delete, .revoke], queue: .main)
      source.setEventHandler { [weak self, weak source] in
        guard let self else { return }
        if let source, !source.data.isDisjoint(with: [.rename, .delete, .revoke]) {
          source.cancel()
          self.sources[path] = nil
        }
        self.changed()
      }
      source.setCancelHandler { close(descriptor) }
      sources[path] = source
      source.resume()
    }
  }

  deinit { for source in sources.values { source.cancel() } }
}

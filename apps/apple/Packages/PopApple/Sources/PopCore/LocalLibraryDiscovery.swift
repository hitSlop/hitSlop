import Foundation

/// Only the prior product's managed cache layout is considered. This is local-file
/// discovery, not a CloudKit importer, recursive disk search or file migration.
enum LocalLibraryDiscovery {
  static var previousRoot: URL {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("pop/Library", isDirectory: true)
  }
  static func roots(at base: URL) throws -> [URL] {
    guard FileManager.default.fileExists(atPath: base.path) else { return [] }
    let accounts = try children(base).filter {
      let name = $0.lastPathComponent
      return name.count == 64 && name.allSatisfy { $0.isHexDigit && !$0.isUppercase }
        && directory($0)
    }
    var roots: [URL] = []
    for account in accounts {
      let library = account.appendingPathComponent("PopLibrary", isDirectory: true)
      if directory(library) { roots.append(library) }
      let development = library.appendingPathComponent("Development", isDirectory: true)
      if directory(development) { roots.append(development) }
      let diagnostics = development.appendingPathComponent("Diagnostics", isDirectory: true)
      if directory(diagnostics) {
        roots += try children(diagnostics).filter { directory($0) }
      }
    }
    return roots.sorted { $0.path < $1.path }
  }
  static func files(in root: URL) throws -> [URL] {
    var result: [URL] = []
    for name in ["documents", "recoveries"] {
      let folder = root.appendingPathComponent(name, isDirectory: true)
      guard directory(folder) else { continue }
      result += try children(folder).filter { url in
        guard url.pathExtension == "pop",
          let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        else { return false }
        return values.isRegularFile == true && values.isSymbolicLink != true
      }
    }
    return result.sorted { $0.path < $1.path }
  }
  private static func children(_ root: URL) throws -> [URL] {
    try FileManager.default.contentsOfDirectory(
      at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey], options: [.skipsHiddenFiles])
  }
  private static func directory(_ url: URL) -> Bool {
    guard let value = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]) else { return false }
    return value.isDirectory == true && value.isSymbolicLink != true
  }
}

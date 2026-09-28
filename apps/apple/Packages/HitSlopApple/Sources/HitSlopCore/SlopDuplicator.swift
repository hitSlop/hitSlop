import Foundation
import Darwin
import SQLite3

public enum SlopDuplicator {
  @discardableResult public static func duplicate(from sourceURL: URL, to requestedDestination: URL)
    throws -> URL
  {
    try SlopLocalDocument.requireLocal(requestedDestination)
    let source = try SlopPackage(rootURL: sourceURL)
    let destination =
      requestedDestination.pathExtension.lowercased() == "slop"
      ? requestedDestination : requestedDestination.appendingPathExtension("slop")
    let fileManager = FileManager.default
    guard !fileManager.fileExists(atPath: destination.path) else {
      throw CocoaError(.fileWriteFileExists)
    }
    do {
      try fileManager.createDirectory(at: destination, withIntermediateDirectories: true)
      for entry in try fileManager.contentsOfDirectory(
        at: source.rootURL, includingPropertiesForKeys: nil)
      where entry.lastPathComponent != "state" {
        try fileManager.copyItem(
          at: entry, to: destination.appendingPathComponent(entry.lastPathComponent))
      }
      let theme = source.rootURL.appendingPathComponent("state/theme.json")
      if fileManager.fileExists(atPath: theme.path) {
        let bytes = try SlopFile.read(theme, within: source.rootURL, maximumBytes: 65536)
        try fileManager.createDirectory(
          at: destination.appendingPathComponent("state"), withIntermediateDirectories: true)
        try bytes.write(
          to: destination.appendingPathComponent("state/theme.json"), options: .atomic)
      }
      let sourceDB = source.rootURL.appendingPathComponent("state/document.sqlite")
      if fileManager.fileExists(atPath: sourceDB.path) {
        let state = destination.appendingPathComponent("state")
        try fileManager.createDirectory(at: state, withIntermediateDirectories: true)
        try backup(source.rootURL, to: state.appendingPathComponent("document.sqlite"))
        try renewIdentity(destination)
      }
      try makeWritable(destination)
      let attachments = try SlopAttachments.list(in: source.rootURL)
      if !attachments.isEmpty {
        try fileManager.createDirectory(at: destination.appendingPathComponent("state"), withIntermediateDirectories: true)
        // This destination was exclusively created above and is not yet published.
        for attachment in attachments {
          let bytes = try SlopAttachments.read(attachment["id"] as! String, in: source.rootURL)
          _ = try SlopAttachments.put(bytes, in: destination)
        }
      }
      // Use the existing package's directory URL, just like Open and Recents.
      return try SlopPackage(rootURL: destination).rootURL
    } catch {
      try? makeWritable(destination)
      try? fileManager.removeItem(at: destination)
      throw error
    }
  }

  // A source validated during package opening can be replaced before this I/O.
  static func backup(_ root: URL, to destination: URL) throws {
    // Foundation retains the system /var alias. Resolve only the package root:
    // NOFOLLOW must still reject replaced state directories and database files.
    guard let resolved = Darwin.realpath(root.path, nil) else {
      throw SlopPackageError.invalid("Cannot resolve document for snapshot")
    }
    let sourcePath = String(cString: resolved) + "/state/document.sqlite"
    free(resolved)
    var input: OpaquePointer?
    var output: OpaquePointer?
    defer {
      sqlite3_close(input)
      sqlite3_close(output)
    }
    guard sqlite3_open_v2(sourcePath, &input, SQLITE_OPEN_READONLY | SQLITE_OPEN_NOFOLLOW, nil) == SQLITE_OK,
      sqlite3_open_v2(destination.path, &output, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, nil)
        == SQLITE_OK
    else { throw SlopPackageError.invalid("Cannot snapshot document") }
    sqlite3_busy_timeout(input, 5000)
    sqlite3_busy_timeout(output, 5000)
    guard let backup = sqlite3_backup_init(output, "main", input, "main") else {
      throw SlopPackageError.invalid("Cannot start document snapshot")
    }
    let result = sqlite3_backup_step(backup, -1)
    let finished = sqlite3_backup_finish(backup)
    guard result == SQLITE_DONE, finished == SQLITE_OK else {
      throw SlopPackageError.invalid("Document snapshot was busy or failed")
    }
  }

  /// A duplicate is a new logical document with the same Loro history and row identities.
  static func renewIdentity(_ root: URL) throws {
    // As in backup: resolve only the package root; NOFOLLOW still guards state entries.
    guard let resolved = Darwin.realpath(root.path, nil) else {
      throw SlopPackageError.invalid("Cannot resolve duplicate for identity assignment")
    }
    let path = String(cString: resolved) + "/state/document.sqlite"
    free(resolved)
    var db: OpaquePointer?
    defer { sqlite3_close(db) }
    guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_NOFOLLOW, nil) == SQLITE_OK,
      sqlite3_exec(db, "UPDATE document SET doc_id=lower(hex(randomblob(16))) WHERE id=1", nil, nil, nil)
        == SQLITE_OK, sqlite3_changes(db) == 1
    else {
      let reason = db.map { String(cString: sqlite3_errmsg($0)) } ?? "cannot open"
      throw SlopPackageError.invalid("Cannot assign the duplicate a document identity: \(reason)")
    }
  }

  public static func makeWritable(_ root: URL) throws {
    try setWriteBits(root, adding: true)
  }

  public static func makeImmutable(_ root: URL) throws {
    try setWriteBits(root, adding: false)
  }

  private static func setWriteBits(_ root: URL, adding: Bool) throws {
    var urls = [root]
    if let enumerator = FileManager.default.enumerator(
      at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey])
    {
      urls += enumerator.compactMap { $0 as? URL }
    }
    for url in adding ? urls : urls.reversed() {
      let values = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
      guard values.isSymbolicLink != true else {
        throw SlopPackageError.invalid("symlinks are not allowed")
      }
      let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
      guard let value = attributes[.posixPermissions] as? NSNumber else { continue }
      let next =
        adding
        ? value.intValue | (values.isDirectory == true ? 0o700 : 0o600)
        : value.intValue & ~0o222
      try FileManager.default.setAttributes([.posixPermissions: next], ofItemAtPath: url.path)
    }
  }
}

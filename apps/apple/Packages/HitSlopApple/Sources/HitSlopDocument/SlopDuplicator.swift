import Foundation
import HitSlopCore
import HitSlopCoreBinding

public enum SlopDuplicator {
  /// The `.slop` package URL for a user-chosen destination.
  public static func packageURL(_ url: URL) -> URL {
    url.pathExtension.lowercased() == "slop" ? url : url.appendingPathExtension("slop")
  }

  /// Copies a package as a new writable document and returns it, validated. A catalog
  /// master (`fromTemplate`) must also satisfy the template rules.
  @discardableResult public static func duplicate(
    from sourceURL: URL, to requestedDestination: URL, fromTemplate: Bool = false
  ) throws -> SlopPackage {
    try SlopLocalDocument.requireLocal(requestedDestination)
    let source = try SlopPackage(rootURL: sourceURL)
    if fromTemplate { try source.validateAsTemplate() }
    let destination = packageURL(requestedDestination)
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
      // A new logical document with the same saved history, row identities and theme.
      if fileManager.fileExists(atPath: source.rootURL.appendingPathComponent("state/document.sqlite").path) {
        try storeCall { try duplicateDocument(sourceRoot: source.rootURL.path, destinationRoot: destination.path) }
      }
      try SlopPermissions.makeWritable(destination)
      let attachments = try SlopAttachments.list(in: source.rootURL)
      if !attachments.isEmpty {
        try fileManager.createDirectory(at: destination.appendingPathComponent("state"), withIntermediateDirectories: true)
        // This destination was exclusively created above and is not yet published.
        for attachment in attachments {
          let bytes = try SlopAttachments.read(attachment.id, in: source.rootURL)
          _ = try SlopAttachments.put(bytes, in: destination)
        }
      }
      // Use the existing package's directory URL, just like Open and Recents.
      return try SlopPackage(rootURL: destination)
    } catch {
      try? SlopPermissions.makeWritable(destination)
      try? fileManager.removeItem(at: destination)
      throw error
    }
  }
}

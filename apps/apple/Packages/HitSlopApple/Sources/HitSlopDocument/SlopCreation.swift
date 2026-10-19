import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// New documents: created from a template here, or copied from a frozen source by
/// `DocumentSession.copy(to:renderArtwork:)`. Rust publishes both kinds of file.
extension SlopFile {
  /// Import preserves the source and creates an independently editable document.
  /// The caller holds security-scoped access and a coordinated source read.
  public static func importCopy(from source: URL, to destination: URL) throws -> URL {
    let destination = try newDocumentURL(destination)
    try storeCall { try importDocument(source: source.path, destination: destination.path) }
    return try resolvedRoot(destination)
  }

  /// Where a new document goes: `url` with the `.slop` extension. Rust checks the
  /// destination and publishes a complete file in iCloud locations.
  public static func newDocumentURL(_ url: URL) throws -> URL {
    let url = url.pathExtension == "slop" ? url : url.appendingPathExtension("slop")
    return url
  }

  /// A new writable document from `template`: the same app, a new identity, no saved state.
  /// The core checks the template and its app before publishing anything, and never
  /// replaces an existing file. Returns the new file's canonical URL (the identity Recents
  /// and live owners use). Finder shows the template's artwork as its icon.
  public static func create(from template: URL, to destination: URL) throws -> URL {
    let destination = try newDocumentURL(destination)
    try storeCall { try createDocument(template: template.path, destination: destination.path) }
    let created = try resolvedRoot(destination)
    SlopFinderIcon.refresh(created)
    return created
  }
}

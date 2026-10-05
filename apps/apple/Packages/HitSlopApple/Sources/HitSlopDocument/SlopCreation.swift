import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// New documents: created from a template here, or copied by an open document's owner
/// (`DocumentOwner.copy(to:)`), so saves wait behind the copy.
extension SlopFile {
  /// Where a new document goes: `url` with the `.slop` extension, on a local volume, and
  /// outside the installed and bundled templates.
  public static func newDocumentURL(_ url: URL) throws -> URL {
    let url = url.pathExtension == "slop" ? url : url.appendingPathExtension("slop")
    try SlopLocalDocument.requireLocal(url)
    guard !SlopTemplateLocation.isMaster(url) else {
      throw failure("A document cannot be created among installed templates")
    }
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

import Foundation

/// What the catalog lists for a document: its manifest, artwork and size. Reads only the
/// manifest and file sizes; opening validates the whole package with `SlopPackage`.
public struct SlopPackageSummary: Sendable {
  public let rootURL: URL
  public let manifest: SlopManifest
  /// Every regular file in the package, state included, in bytes.
  public let byteCount: Int64

  public init(rootURL: URL) throws {
    let root = try SlopPackage.resolvedRoot(rootURL)
    self.rootURL = root
    manifest = try SlopPackage.readManifest(root).manifest
    var total: Int64 = 0
    if let enumerator = FileManager.default.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey, .fileSizeKey]) {
      for case let url as URL in enumerator {
        let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        if values?.isRegularFile == true { total += Int64(values?.fileSize ?? 0) }
      }
    }
    byteCount = total
  }

  public var iconURL: URL { rootURL.appendingPathComponent("QuickLook/Icon.png") }
  public var previewURL: URL { rootURL.appendingPathComponent("QuickLook/Preview.png") }
}

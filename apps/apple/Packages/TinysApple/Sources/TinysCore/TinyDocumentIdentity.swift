import Foundation

/// File identity for native chrome, independent of the template's manifest title.
public struct TinyDocumentIdentity: Equatable, Sendable {
  public let filename: String
  public let path: String
  public let folderPath: String

  public init(url: URL) {
    filename = url.lastPathComponent
    path = url.path
    folderPath = (url.deletingLastPathComponent().path as NSString).abbreviatingWithTildeInPath
  }
}

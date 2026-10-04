import Foundation

/// A refusal that retrying cannot fix. The bridge replies `{ rejected }`, never a save failure.
public protocol SlopRejection: LocalizedError {}

/// A stored attachment, as the page and the CLI see it: its identity (the SHA-256 of its
/// bytes, which the document stores) and its size.
public struct AttachmentRef: Codable, Sendable, Equatable {
  public let id: String
  public let byteLength: Int
  public init(id: String, byteLength: Int) {
    self.id = id
    self.byteLength = byteLength
  }
}

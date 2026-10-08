import Foundation
import HitSlopCoreBinding

/// Catalog metadata; listing a file does not accept its app or interpret its definition.
public struct SlopSummary: Sendable {
  public let url: URL
  public let metadata: AppMetadata
  public let kind: FileKind
  public let byteCount: Int64
  public init(url: URL, template: Bool = false) throws {
    let root = try SlopFile.resolvedRoot(url)
    let summary = try SlopFile.opening {
      try template ? openTemplate(path: root.path) : fileSummary(path: root.path)
    }
    self.url = root
    metadata = summary.metadata
    kind = summary.kind
    byteCount = Int64(summary.bytes)
  }
}

public typealias SlopCategory = HitSlopCoreBinding.SlopCategory
extension SlopCategory: CaseIterable, RawRepresentable {
  public static var allCases: [SlopCategory] { categories() }
  public static var schemaOrder: [SlopCategory] { allCases }
  public var rawValue: String { categoryName(category: self) }
  public init?(rawValue: String) {
    guard let value = parseCategory(name: rawValue) else { return nil }
    self = value
  }
}

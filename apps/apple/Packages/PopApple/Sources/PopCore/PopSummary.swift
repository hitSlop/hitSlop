import Foundation
import PopCoreBinding

/// Catalog metadata; listing a file does not accept its app or interpret its definition.
public struct PopSummary: Sendable {
  public let url: URL
  public let metadata: AppMetadata
  public let kind: FileKind
  public let byteCount: Int64
  public init(url: URL, template: Bool = false) throws {
    let root = try PopFile.resolvedRoot(url)
    let summary = try PopFile.opening {
      try template ? openTemplate(path: root.path) : fileSummary(path: root.path)
    }
    self.url = root
    metadata = summary.metadata
    kind = summary.kind
    byteCount = Int64(summary.bytes)
  }
}

public typealias PopCategory = PopCoreBinding.PopCategory
extension PopCategory: CaseIterable, RawRepresentable {
  public static var allCases: [PopCategory] { categories() }
  public var rawValue: String { categoryName(category: self) }
  public init?(rawValue: String) {
    guard let value = parseCategory(name: rawValue) else { return nil }
    self = value
  }
}

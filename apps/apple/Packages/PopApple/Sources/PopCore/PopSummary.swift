import Foundation
import PopCoreBinding

@_exported import enum PopCoreBinding.PopCategory

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

extension PopCategory: CaseIterable, RawRepresentable {
  public static var allCases: [PopCategory] {
    [
      .popular, .productivity, .utilities, .finance, .media, .games, .developerTools,
      .education, .business, .personal, .health, .creative, .music, .other,
    ]
  }
  public var rawValue: String {
    switch self {
    case .popular: "popular"
    case .productivity: "productivity"
    case .utilities: "utilities"
    case .finance: "finance"
    case .media: "media"
    case .games: "games"
    case .developerTools: "developer-tools"
    case .education: "education"
    case .business: "business"
    case .personal: "personal"
    case .health: "health"
    case .creative: "creative"
    case .music: "music"
    case .other: "other"
    }
  }
  public init?(rawValue: String) {
    guard let value = Self.allCases.first(where: { $0.rawValue == rawValue }) else { return nil }
    self = value
  }
}

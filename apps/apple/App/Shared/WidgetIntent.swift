import AppIntents
import Foundation
import HitSlopWidgets

public struct SlopWidgetEntity: AppEntity {
  public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Document"
  public static let defaultQuery = SlopWidgetQuery()
  public let id: String
  public let title: String
  public var displayRepresentation: DisplayRepresentation {
    let image: DisplayRepresentation.Image
    if let cache = try? WidgetCache.shared(), let record = cache.record(id: id),
      let png = cache.imageData(record, .small)
    {
      image = .init(data: png, isTemplate: false)
    } else {
      image = .init(systemName: "doc")
    }
    return DisplayRepresentation(title: "\(title)", image: image)
  }
  public init(id: String, title: String) {
    self.id = id
    self.title = title
  }
}

public struct SlopWidgetQuery: EntityQuery {
  public init() {}
  public func entities(for identifiers: [String]) async throws -> [SlopWidgetEntity] {
    let cache = try WidgetCache.shared()
    return identifiers.compactMap { cache.record(id: $0) }.map { .init(id: $0.id, title: $0.title) }
  }
  public func suggestedEntities() async throws -> [SlopWidgetEntity] {
    try WidgetCache.shared().records().map { .init(id: $0.id, title: $0.title) }
  }
  // No implicit default: a new placement asks the person which document it represents.
}

public struct SlopWidgetIntent: WidgetConfigurationIntent {
  public static let title: LocalizedStringResource = "Choose a Document"
  public static let description = IntentDescription(
    "Choose the document to show on your desktop.")
  @Parameter(title: "Document") public var slop: SlopWidgetEntity?
  public init() {}
}

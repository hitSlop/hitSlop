import AppIntents
import Foundation
import HitSlopWidgets

public struct SlopWidgetEntity: AppEntity {
  public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Slop"
  public static let defaultQuery = SlopWidgetQuery()
  public let id: String
  public let title: String
  public var displayRepresentation: DisplayRepresentation { DisplayRepresentation(title: "\(title)") }
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
  public static let title: LocalizedStringResource = "Choose a Slop"
  public static let description = IntentDescription(
    "Choose a document with a widget. Open and close it in hitSlop to make it available here.")
  @Parameter(title: "Slop") public var slop: SlopWidgetEntity?
  public init() {}
}

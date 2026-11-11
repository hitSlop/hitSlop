import AppIntents
import Foundation
import PopWidgets
import WidgetKit

public struct PopWidgetEntity: AppEntity {
  public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Document"
  public static let defaultQuery = PopWidgetQuery()
  public let id: String
  public let title: String
  public var displayRepresentation: DisplayRepresentation {
    let image: DisplayRepresentation.Image
    if let icon = try? WidgetCache.shared().libraryIndex().first(where: { $0.id == id })?.icon {
      image = .init(data: icon, isTemplate: false)
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

public struct PopWidgetQuery: EntityQuery {
  public init() {}
  public func entities(for identifiers: [String]) async throws -> [PopWidgetEntity] {
    let cache = try WidgetCache.shared()
    let index = cache.libraryIndex()
    return identifiers.compactMap { id in index.first { $0.id == id } }.map { .init(id: $0.id, title: $0.title) }
  }
  public func suggestedEntities() async throws -> [PopWidgetEntity] {
    try WidgetCache.shared().libraryIndex().map { .init(id: $0.id, title: $0.title) }
  }
  // No implicit default: a new placement asks the person which document it represents.
}

public struct PopWidgetIntent: WidgetConfigurationIntent {
  public static let title: LocalizedStringResource = "Choose a Document"
  #if os(iOS)
    public static let description = IntentDescription("Choose the document to show on your Home Screen.")
  #else
    public static let description = IntentDescription("Choose the document to show on your desktop.")
  #endif
  @Parameter(title: "Document") public var pop: PopWidgetEntity?
  public init() {}
}

extension PopWidgetIntent {
  @MainActor public static func placedDocumentIDs() async -> Set<String> {
    do {
      let configurations: [WidgetInfo] = try await withCheckedThrowingContinuation { continuation in
        WidgetCenter.shared.getCurrentConfigurations { continuation.resume(with: $0) }
      }
      return Set(
        configurations.filter { $0.kind == WidgetCache.kind }
          .compactMap { $0.widgetConfigurationIntent(of: Self.self)?.pop?.id })
    } catch {
      NSLog("pop widget configurations unavailable: %@", error.localizedDescription)
      return []
    }
  }
}

import Foundation

/// Where installed templates live. A template is a kind of file, not a location: this
/// folder decides what the catalog lists, and the core refuses new documents inside it.
public enum SlopTemplateLocation {
  private static var defaultTemplatesRoot: URL {
    FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".hitslop/templates", isDirectory: true)
  }
  /// `HITSLOP_TEMPLATES_ROOT`: the catalog root a development app was given.
  private static var developmentTemplatesRoot: URL? {
    ProcessInfo.processInfo.environment["HITSLOP_TEMPLATES_ROOT"].map { URL(fileURLWithPath: $0) }
  }
  /// The installed templates folder the catalog lists.
  public static var templatesRoot: URL { developmentTemplatesRoot ?? defaultTemplatesRoot }
}

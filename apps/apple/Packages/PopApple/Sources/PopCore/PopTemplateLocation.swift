import Foundation
import PopCoreBinding

/// Where templates live, as the core lists them, so the app and the CLI agree. A
/// template is a kind of file, not a location: these folders decide what the catalog lists,
/// and the core refuses new documents inside them.
public enum PopTemplateLocation {
  /// The installed templates folder the catalog lists: `POP_TEMPLATES_ROOT` (a
  /// development app's) or `~/.pop/templates`. Nil only without an account home folder.
  public static var templatesRoot: URL? { root(.installed) }
  /// The starters bundled with the app, when this process runs inside it.
  public static var bundledRoot: URL? { root(.bundled) }

  private static func root(_ source: TemplateSource) -> URL? {
    templateRoots().first { $0.source == source }.map { URL(fileURLWithPath: $0.path, isDirectory: true) }
  }
}

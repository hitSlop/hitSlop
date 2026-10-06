import Foundation

/// Where installed templates live. A template is a kind of file, not a location: these
/// places only decide what the catalog lists and where documents may not be created.
public enum SlopTemplateLocation {
  static var defaultTemplatesRoot: URL {
    FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".hitslop/templates", isDirectory: true)
  }
  /// `HITSLOP_TEMPLATES_ROOT`: the catalog root a development app was given.
  private static var developmentTemplatesRoot: URL? {
    ProcessInfo.processInfo.environment["HITSLOP_TEMPLATES_ROOT"].map { URL(fileURLWithPath: $0) }
  }
  /// The installed templates folder the catalog lists.
  public static var templatesRoot: URL { developmentTemplatesRoot ?? defaultTemplatesRoot }

  /// Inside the installed or bundled templates, where a document is never created.
  public static func isMaster(_ url: URL) -> Bool {
    if isInTemplates(url) { return true }
    guard let root = developmentTemplatesRoot else { return false }
    return isInTemplates(url, templatesRoot: root)
  }

  static func isInTemplates(_ url: URL, templatesRoot: URL = defaultTemplatesRoot) -> Bool {
    let candidate = SlopPath.canonical(url).pathComponents
    var roots = [templatesRoot]
    if let resources = Bundle.main.resourceURL {
      roots.append(resources.appendingPathComponent("StarterTemplates"))
    }
    // The installed CLI's main bundle is Contents/Helpers, not the enclosing app.
    if let executable = Bundle.main.executableURL?.resolvingSymlinksInPath() {
      let helpers = executable.deletingLastPathComponent()
      let contents = helpers.deletingLastPathComponent()
      if helpers.lastPathComponent.caseInsensitiveCompare("Helpers") == .orderedSame,
        contents.lastPathComponent.caseInsensitiveCompare("Contents") == .orderedSame,
        contents.deletingLastPathComponent().pathExtension.caseInsensitiveCompare("app") == .orderedSame
      {
        roots.append(contents.appendingPathComponent("Resources/StarterTemplates"))
      }
    }
    return roots.contains { url in
      let root = SlopPath.canonical(url).pathComponents
      return candidate.count > root.count
        && zip(root, candidate).allSatisfy {
          $0.caseInsensitiveCompare($1) == .orderedSame
        }
    }
  }
}

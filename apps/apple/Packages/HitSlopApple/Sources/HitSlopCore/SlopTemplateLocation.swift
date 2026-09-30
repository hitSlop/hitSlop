import Foundation

/// Shared catalog-master classification. Opening a master must never create live document state.
public enum SlopTemplateLocation {
  public static var defaultTemplatesRoot: URL {
    FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".hitslop/templates", isDirectory: true)
  }
  /// `HITSLOP_TEMPLATES_ROOT`: the catalog root a development app was given.
  public static var developmentTemplatesRoot: URL? {
    ProcessInfo.processInfo.environment["HITSLOP_TEMPLATES_ROOT"].map { URL(fileURLWithPath: $0) }
  }
  /// The installed templates folder the catalog lists.
  public static var templatesRoot: URL { developmentTemplatesRoot ?? defaultTemplatesRoot }
  /// Why a master cannot be edited, and what to do instead.
  public static let writableCopyRequired = "Create a writable copy of this template first"

  /// A master the CLI must never edit or create documents inside: a managed template, or
  /// one under the development templates root.
  public static func isMaster(_ url: URL) -> Bool {
    if isManagedTemplatePackage(url) { return true }
    guard let root = developmentTemplatesRoot else { return false }
    return isManagedTemplatePackage(url, templatesRoot: root)
  }

  public static func isManagedTemplatePackage(_ url: URL, templatesRoot: URL = defaultTemplatesRoot) -> Bool {
    let candidate = url.standardizedFileURL.resolvingSymlinksInPath().pathComponents
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
         contents.deletingLastPathComponent().pathExtension.caseInsensitiveCompare("app") == .orderedSame {
        roots.append(contents.appendingPathComponent("Resources/StarterTemplates"))
      }
    }
    return roots.contains { url in
      let root = url.standardizedFileURL.resolvingSymlinksInPath().pathComponents
      return candidate.count > root.count && zip(root, candidate).allSatisfy {
        $0.caseInsensitiveCompare($1) == .orderedSame
      }
    }
  }
}

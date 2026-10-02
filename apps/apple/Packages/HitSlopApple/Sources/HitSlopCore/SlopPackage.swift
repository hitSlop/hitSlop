import Foundation
import HitSlopCoreBinding
import ImageIO

public enum SlopPackageError: LocalizedError {
  case missing(String)
  case invalid(String)
  public var errorDescription: String? {
    switch self {
    case .missing(let value): "Missing \(value)"
    case .invalid(let value): "Invalid hitSlop package: \(value)"
    }
  }
}

/// A package, its storage or its document needs a newer hitSlop. Nothing was read past the
/// marker that said so, and nothing was written.
public struct SlopRequiresUpdate: LocalizedError, SlopDiagnosticProviding {
  public init() {}
  public var diagnostic: SlopFailureContext { .init(.rejection, reason: .requiresUpdate) }
  public var errorDescription: String? { "This slop needs a newer version of hitSlop. Update hitSlop to open it." }
  /// Whether the core refused for this reason.
  public static func matches(_ error: Error) -> Bool {
    if case let CoreError.Rejected(code, _, _) = error { return code == CoreErrorCode.requires_update.rawValue }
    return false
  }
}

public struct SlopPackage: Sendable {
  public let rootURL: URL
  private var validatedSkin: (url: URL, image: CGImage)?
  public let schemaKey: String
  /// `assets/theme.json`, validated: every theme color and its default value.
  public let themeDefaults: String
  /// The declared colors in the order the author wrote them.
  public let themeTokens: [ThemeToken]
  public let manifest: SlopManifest
  public let silhouette: SlopSilhouette
  /// Every regular file in the package, state included, in bytes.
  public let byteCount: Int64

  public init(rootURL: URL) throws {
    let fileManager = FileManager.default
    let root = try Self.resolvedRoot(rootURL)
    self.rootURL = root
    let decoded = try Self.readManifest(root)
    manifest = decoded.manifest

    silhouette = SlopSilhouette(parsed: decoded.silhouette)
    do {
      func utf8(_ file: String, maximum: Int) throws -> String {
        let bytes = try SlopFile.read(root.appendingPathComponent(file), within: root, maximumBytes: maximum)
        guard let value = String(data: bytes, encoding: .utf8) else { throw SlopPackageError.invalid("\(file) must be UTF-8") }
        return value
      }
      // `initial.json` is creation-only: storage creation and `validateAsTemplate` check
      // it, so a later, stricter rule never refuses a saved document.
      schemaKey = try documentSchemaKey(schemaJson: utf8("state.schema.json", maximum: 1_048_576))
      themeDefaults = try utf8("assets/theme.json", maximum: Limits.theme)
      themeTokens = try validateThemeDefaults(json: themeDefaults)
    } catch let CoreError.Rejected(_, message, _) {
      throw SlopPackageError.invalid(message)
    } catch let CoreError.Invalidated(message) {
      throw SlopPackageError.invalid(message)
    }

    // The host owns the page; a package supplies only its app module.
    let entry = root.appendingPathComponent("assets/app.js")
    guard fileManager.fileExists(atPath: entry.path) else {
      throw SlopPackageError.missing("assets/app.js")
    }
    guard String(data: try SlopFile.read(entry, within: root), encoding: .utf8) != nil else {
      throw SlopPackageError.invalid("assets/app.js must be UTF-8")
    }
    let topLevel = try fileManager.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
    let allowedTopLevel = Set([
      "manifest.json", "assets", "state", "QuickLook", ".agents", "Icon\r",
      "state.schema.json", "initial.json",
    ])
    if let unknown = topLevel.first(where: { !allowedTopLevel.contains($0.lastPathComponent) }) {
      throw SlopPackageError.invalid("unexpected package entry \(unknown.lastPathComponent)")
    }
    let forbidden = Set([
      "package.json", "bun.lock", "bun.lockb", "node_modules", "source", "src", "build",
      ".build", ".hitslop",
    ])
    var immutableCount = 0
    var immutableBytes = 0
    var totalBytes: Int64 = 0
    if let enumerator = fileManager.enumerator(
      at: root,
      includingPropertiesForKeys: [
        .isSymbolicLinkKey, .isRegularFileKey, .isDirectoryKey, .fileSizeKey,
      ])
    {
      for case let url as URL in enumerator {
        let values = try url.resourceValues(forKeys: [
          .isSymbolicLinkKey, .isRegularFileKey, .isDirectoryKey, .fileSizeKey,
        ])
        guard values.isSymbolicLink != true,
          values.isRegularFile == true || values.isDirectory == true
        else {
          throw SlopPackageError.invalid(
            "packages require regular files and directories, without symlinks")
        }
        // Foundation may enumerate /private/var entries from a /var root. Compare
        // equally resolved paths so mutable state never enters the authored budget.
        let entryPath = url.standardizedFileURL.resolvingSymlinksInPath().path
        guard entryPath.hasPrefix(root.path + "/") else {
          throw SlopPackageError.invalid("file escapes its package")
        }
        let relative = String(entryPath.dropFirst(root.path.count + 1))
        if values.isRegularFile == true { totalBytes += Int64(values.fileSize ?? 0) }
        if !relative.hasPrefix("state/")
          && relative != "state" && relative != "Icon\r"
        {
          immutableCount += 1
          if values.isRegularFile == true {
            let size = values.fileSize ?? 0
            guard size <= SlopFile.maximumBytes else {
              throw SlopPackageError.invalid("package file exceeds 25 MiB")
            }
            immutableBytes += size
          }
          guard immutableCount <= Limits.packageEntries, immutableBytes <= Limits.packageBytes else {
            throw SlopPackageError.invalid("immutable package exceeds 256 entries or 50 MiB")
          }
        }
        if forbidden.contains(url.lastPathComponent.lowercased()) {
          throw SlopPackageError.invalid(
            "packages cannot contain \(url.lastPathComponent)")
        }
      }
    }
    byteCount = totalBytes
    try validateState()
    try validateDocumentSkill()
    try validateQuickLook()
    validatedSkin = try skin()
  }

  public var entryURL: URL { rootURL.appendingPathComponent("assets/app.js") }
  public var previewURL: URL { rootURL.appendingPathComponent("QuickLook/Preview.png") }
  public var iconURL: URL { rootURL.appendingPathComponent("QuickLook/Icon.png") }
  public var stateURL: URL { rootURL.appendingPathComponent("state", isDirectory: true) }
  public var dataSchemaURL: URL { rootURL.appendingPathComponent("state.schema.json") }
  public var initialURL: URL { rootURL.appendingPathComponent("initial.json") }
  /// Mutable state: theme overrides, the live owner's discovery file and the database.
  public var discoveryURL: URL { stateURL.appendingPathComponent("host.lock") }
  public var isSkinned: Bool { manifest.presentation.skin != nil }
  public var usesTransparentBackground: Bool {
    isSkinned || manifest.presentation.background == .transparent
  }
  public var isResizable: Bool { isSkinned ? false : manifest.presentation.resizable ?? true }

  /// Validates and decodes the window skin once for callers that also need its pixels.
  public func skin() throws -> (url: URL, image: CGImage)? {
    if let validatedSkin { return validatedSkin }
    guard let path = manifest.presentation.skin else { return nil }
    let url = try Self.containedURL(root: rootURL, relativePath: path)
    let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
    guard values.isRegularFile == true, values.isSymbolicLink != true else {
      throw SlopPackageError.invalid("window skin must be a regular file")
    }
    guard
      let source = CGImageSourceCreateWithData(
        try SlopFile.read(url, within: rootURL) as CFData, nil),
      CGImageSourceGetType(source) as String? == "public.png"
    else { throw SlopPackageError.invalid("window skin must be a valid PNG") }
    try Self.validateImageDimensions(
      source, label: "window skin", width: manifest.presentation.width,
      height: manifest.presentation.height)
    guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
      throw SlopPackageError.invalid("window skin must be a valid PNG")
    }
    guard image.width == manifest.presentation.width, image.height == manifest.presentation.height
    else {
      throw SlopPackageError.invalid(
        "window skin must be exactly \(manifest.presentation.width)x\(manifest.presentation.height) pixels"
      )
    }
    guard image.colorSpace?.model == .rgb else {
      throw SlopPackageError.invalid("window skin must be an RGBA PNG")
    }
    guard ![.none, .noneSkipFirst, .noneSkipLast].contains(image.alphaInfo) else {
      throw SlopPackageError.invalid("window skin must contain alpha")
    }
    return (url, image)
  }

  public func validateAsTemplate() throws {
    try validateDocumentSkill(strict: true)
    do {
      func read(_ file: String, _ maximum: Int) throws -> String {
        let bytes = try SlopFile.read(rootURL.appendingPathComponent(file), within: rootURL, maximumBytes: maximum)
        guard let value = String(data: bytes, encoding: .utf8) else { throw SlopPackageError.invalid("\(file) must be UTF-8") }
        return value
      }
      _ = try validateDocument(
        schemaJson: read("state.schema.json", 1_048_576), initialJson: read("initial.json", SlopFile.maximumBytes))
    } catch let CoreError.Rejected(_, message, _) {
      throw SlopPackageError.invalid(message)
    }
    if FileManager.default.fileExists(atPath: stateURL.path) {
      throw SlopPackageError.invalid("templates cannot contain state")
    }
    if FileManager.default.fileExists(atPath: rootURL.appendingPathComponent("Icon\r").path) {
      throw SlopPackageError.invalid("templates cannot contain a Finder custom icon")
    }
  }

  public static func isSafeRelativePath(_ path: String) -> Bool {
    guard !path.isEmpty, path.count <= 240, !path.hasPrefix("/"), !path.contains("\\"),
      !path.contains("\0")
    else { return false }
    let normalized = path.hasSuffix("/") ? String(path.dropLast()) : path
    return normalized.split(separator: "/", omittingEmptySubsequences: false).allSatisfy {
      !$0.isEmpty && $0 != "." && $0 != ".."
    }
  }

  private static func validateImageDimensions(
    _ source: CGImageSource, label: String, width: Int? = nil, height: Int? = nil
  ) throws {
    guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
      let imageWidth = properties[kCGImagePropertyPixelWidth] as? Int,
      let imageHeight = properties[kCGImagePropertyPixelHeight] as? Int,
      imageWidth > 0, imageHeight > 0, imageWidth <= Limits.imageSide, imageHeight <= Limits.imageSide,
      imageWidth * imageHeight <= Limits.imagePixels
    else {
      throw SlopPackageError.invalid("\(label) exceeds the PNG dimension limit")
    }
    if let width, let height, imageWidth != width || imageHeight != height {
      throw SlopPackageError.invalid("\(label) must be exactly \(width)x\(height) pixels")
    }
  }

  public static func containedURL(root: URL, relativePath: String) throws -> URL {
    guard isSafeRelativePath(relativePath) else {
      throw SlopPackageError.invalid("unsafe path \(relativePath)")
    }
    let root = root.standardizedFileURL.resolvingSymlinksInPath()
    let url = root.appendingPathComponent(relativePath).standardizedFileURL
    guard SlopPath.contains(root, url) else {
      throw SlopPackageError.invalid("unsafe path \(relativePath)")
    }
    return url
  }

  /// A package directory's canonical URL: a `.slop` directory, never a symlink.
  static func resolvedRoot(_ rootURL: URL) throws -> URL {
    guard try rootURL.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else {
      throw SlopPackageError.invalid("document package cannot be a symlink")
    }
    let root = rootURL.standardizedFileURL.resolvingSymlinksInPath()
    guard root.pathExtension.lowercased() == "slop" else {
      throw SlopPackageError.invalid("document must have a .slop extension")
    }
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: root.path, isDirectory: &isDirectory), isDirectory.boolValue
    else { throw SlopPackageError.invalid("document is not a directory") }
    return root
  }

  static func readManifest(_ root: URL) throws -> (manifest: SlopManifest, silhouette: WindowSilhouette) {
    let url = root.appendingPathComponent("manifest.json")
    guard FileManager.default.fileExists(atPath: url.path) else { throw SlopPackageError.missing("manifest.json") }
    return try decodeManifest(try SlopFile.read(url, within: root, maximumBytes: 64 * 1024))
  }

  /// The core refuses a package above this build's platform level before judging its
  /// other fields; a package at a supported level must match the contract exactly.
  private static func decodeManifest(_ data: Data) throws -> (manifest: SlopManifest, silhouette: WindowSilhouette) {
    guard let json = String(data: data, encoding: .utf8) else {
      throw SlopPackageError.invalid("manifest.json must be UTF-8")
    }
    do {
      let silhouette = try validateManifest(manifestJson: json)
      return (try JSONDecoder().decode(SlopManifest.self, from: data), silhouette)
    } catch let error where SlopRequiresUpdate.matches(error) {
      throw SlopRequiresUpdate()
    } catch let CoreError.Rejected(_, message, _) {
      throw SlopPackageError.invalid(message)
    } catch {
      throw SlopPackageError.invalid(error.localizedDescription)
    }
  }

  private func validateState() throws {
    let fileManager = FileManager.default
    guard fileManager.fileExists(atPath: stateURL.path) else { return }
    let allowed = Set([
      "document.sqlite", "document.sqlite-journal", "writer.lock", "host.lock",
    ])
    for url in try fileManager.contentsOfDirectory(
      at: stateURL, includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey])
    {
      let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
      if url.lastPathComponent == "attachments" {
        _ = try SlopAttachments.list(in: rootURL)
        continue
      }
      guard allowed.contains(url.lastPathComponent), values.isRegularFile == true,
        values.isSymbolicLink != true
      else {
        throw SlopPackageError.invalid("unexpected or unsafe state file")
      }
    }
  }

  private func validateDocumentSkill(strict: Bool = false) throws {
    let fileManager = FileManager.default
    let agents = rootURL.appendingPathComponent(".agents", isDirectory: true)
    guard fileManager.fileExists(atPath: agents.path) else { return }
    let skills = agents.appendingPathComponent("skills", isDirectory: true)
    let folder = skills.appendingPathComponent("hitslop-document", isDirectory: true)
    let skill = folder.appendingPathComponent("SKILL.md")
    try requireDirectory(agents, allowed: ["skills"], label: ".agents")
    if !strict && !fileManager.fileExists(atPath: skills.path) { return }
    try requireDirectory(skills, allowed: ["hitslop-document"], label: ".agents/skills")
    if !strict && !fileManager.fileExists(atPath: folder.path) { return }
    try requireDirectory(
      folder, allowed: ["SKILL.md", "references"], label: ".agents/skills/hitslop-document")
    if strict || fileManager.fileExists(atPath: skill.path) {
      let values = try skill.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
      guard values.isRegularFile == true, values.isSymbolicLink != true else {
        throw SlopPackageError.invalid("document guidance must be a regular file")
      }
    }
    let references = folder.appendingPathComponent("references", isDirectory: true)
    guard fileManager.fileExists(atPath: references.path) else { return }
    try requireDirectory(
      references, allowed: ["app-guide.md"], label: ".agents/skills/hitslop-document/references")
    let guide = references.appendingPathComponent("app-guide.md")
    if !strict && !fileManager.fileExists(atPath: guide.path) { return }
    let guideValues = try guide.resourceValues(forKeys: [
      .isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey,
    ])
    guard guideValues.isRegularFile == true, guideValues.isSymbolicLink != true else {
      throw SlopPackageError.invalid("document app guide must be a regular file")
    }
    if strict {
      guard (guideValues.fileSize ?? 0) <= 32 * 1024,
        String(
          data: try SlopFile.read(guide, within: rootURL, maximumBytes: 32 * 1024), encoding: .utf8)
          != nil
      else {
        throw SlopPackageError.invalid(
          "document app guide must be a UTF-8 Markdown file no larger than 32 KiB")
      }
    }
  }

  private func requireDirectory(_ url: URL, allowed: Set<String>, label: String) throws {
    let values = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
    guard values.isDirectory == true, values.isSymbolicLink != true else {
      throw SlopPackageError.invalid("\(label) must be a directory")
    }
    for child in try FileManager.default.contentsOfDirectory(
      at: url, includingPropertiesForKeys: nil) where !allowed.contains(child.lastPathComponent)
    {
      throw SlopPackageError.invalid("\(label) cannot contain \(child.lastPathComponent)")
    }
  }

  private func validateQuickLook() throws {
    let directory = rootURL.appendingPathComponent("QuickLook", isDirectory: true)
    guard FileManager.default.fileExists(atPath: directory.path) else { return }
    let allowed = Set(["Preview.png", "Icon.png"])
    for url in try FileManager.default.contentsOfDirectory(
      at: directory, includingPropertiesForKeys: nil) where !allowed.contains(url.lastPathComponent)
    {
      throw SlopPackageError.invalid("unexpected QuickLook entry \(url.lastPathComponent)")
    }
  }
}

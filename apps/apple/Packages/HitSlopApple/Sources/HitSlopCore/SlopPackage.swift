import Foundation
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

public struct SlopPackage: Sendable {
  public let rootURL: URL
  public let manifestData: Data
  private var validatedSkin: (url: URL, image: CGImage)?
  public let manifest: SlopManifest

  public init(rootURL: URL) throws {
    let fileManager = FileManager.default
    guard try rootURL.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else {
      throw SlopPackageError.invalid("document package cannot be a symlink")
    }
    let root = rootURL.standardizedFileURL.resolvingSymlinksInPath()
    var isDirectory: ObjCBool = false
    guard root.pathExtension.lowercased() == "slop" else {
      throw SlopPackageError.invalid("document must have a .slop extension")
    }
    guard fileManager.fileExists(atPath: root.path, isDirectory: &isDirectory),
      isDirectory.boolValue
    else { throw SlopPackageError.invalid("document is not a directory") }
    self.rootURL = root
    let manifestURL = root.appendingPathComponent("manifest.json")
    guard fileManager.fileExists(atPath: manifestURL.path) else {
      throw SlopPackageError.missing("manifest.json")
    }
    manifestData = try SlopFile.read(manifestURL, within: root, maximumBytes: 64 * 1024)
    manifest = try JSONDecoder().decode(SlopManifest.self,
      from: JSONSerialization.data(withJSONObject: Self.normalizedManifestObject(manifestData)))

    // The runtime owns the page; a package supplies only its app module.
    guard fileManager.fileExists(atPath: entryURL.path) else {
      throw SlopPackageError.missing("assets/app.js")
    }
    guard String(data: try SlopFile.read(entryURL, within: root), encoding: .utf8) != nil else {
      throw SlopPackageError.invalid("assets/app.js must be UTF-8")
    }
    let topLevel = try fileManager.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
    let allowedTopLevel = Set([
      "manifest.json", "assets", "state", "QuickLook", ".agents", "Icon\r",
      "state.schema.json", "initial.json",
    ])
    if let unknown = topLevel.first(where: { !allowedTopLevel.contains($0.lastPathComponent) }) {
      throw SlopPackageError.invalid("unexpected runtime entry \(unknown.lastPathComponent)")
    }
    let forbidden = Set([
      "package.json", "bun.lock", "bun.lockb", "node_modules", "source", "src", "build",
      "document.json", ".build", ".hitslop", "style.css",
    ])
    var immutableCount = 0
    var immutableBytes = 0
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
            "runtime documents require regular files and directories, without symlinks")
        }
        let relative = String(url.path.dropFirst(root.path.count + 1))
        if !relative.hasPrefix("state/")
          && relative != "state" && relative != "Icon\r"
        {
          immutableCount += 1
          if values.isRegularFile == true {
            let size = values.fileSize ?? 0
            guard size <= SlopFile.maximumBytes else {
              throw SlopPackageError.invalid("runtime file exceeds 25 MiB")
            }
            immutableBytes += size
          }
          guard immutableCount <= 256, immutableBytes <= 50 * 1024 * 1024 else {
            throw SlopPackageError.invalid("immutable package exceeds 256 entries or 50 MiB")
          }
        }
        if forbidden.contains(url.lastPathComponent.lowercased()) {
          throw SlopPackageError.invalid(
            "runtime documents cannot contain \(url.lastPathComponent)")
        }
      }
    }
    try validateState()
    try validateSchemaMetadata()
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
  public var isSkinned: Bool { manifest.presentation.skin != nil }
  public var usesTransparentBackground: Bool {
    isSkinned || manifest.presentation.background == .transparent
  }
  public var isResizable: Bool { isSkinned ? false : manifest.presentation.resizable ?? true }
  public var shape: Shape { manifest.presentation.shape ?? .rounded }

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

  public func validateAsTemplate(requirePreview: Bool = false) throws {
    try validateDocumentSkill(strict: true)
    if FileManager.default.fileExists(atPath: stateURL.path) {
      throw SlopPackageError.invalid("templates cannot contain state")
    }
    if FileManager.default.fileExists(atPath: rootURL.appendingPathComponent("Icon\r").path) {
      throw SlopPackageError.invalid("templates cannot contain a Finder custom icon")
    }
    if requirePreview {
      for url in [previewURL, iconURL] {
        let relativePath = "QuickLook/\(url.lastPathComponent)"
        guard FileManager.default.fileExists(atPath: url.path) else {
          throw SlopPackageError.missing(relativePath)
        }
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        guard values.isRegularFile == true, (values.fileSize ?? 0) <= 5 * 1024 * 1024,
          let source = CGImageSourceCreateWithData(
            try SlopFile.read(url, within: rootURL) as CFData, nil),
          CGImageSourceGetType(source) as String? == "public.png"
        else {
          throw SlopPackageError.invalid("\(relativePath) must be a PNG no larger than 5 MB")
        }
        try Self.validateImageDimensions(
          source, label: relativePath, width: url == iconURL ? 512 : nil,
          height: url == iconURL ? 512 : nil)
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
          throw SlopPackageError.invalid("\(relativePath) must be a valid PNG")
        }
        if url == iconURL, image.width != 512 || image.height != 512 {
          throw SlopPackageError.invalid("\(relativePath) must be exactly 512x512 pixels")
        }
      }
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
      imageWidth > 0, imageHeight > 0, imageWidth <= 16_384, imageHeight <= 16_384,
      imageWidth * imageHeight <= 24_000_000
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
      .resolvingSymlinksInPath()
    let prefix = root.path.hasSuffix("/") ? root.path : root.path + "/"
    guard url.path == root.path || url.path.hasPrefix(prefix) else {
      throw SlopPackageError.invalid("unsafe path \(relativePath)")
    }
    return root.appendingPathComponent(relativePath).standardizedFileURL
  }

  private static func normalizedManifestObject(_ data: Data) throws -> [String: Any] {
    guard String(data: data, encoding: .utf8) != nil else {
      throw SlopPackageError.invalid("manifest.json must be UTF-8")
    }
    guard var object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
      PlatformContract.valid(object, against: manifestReadSchema)
    else { throw SlopPackageError.invalid("unsupported or invalid v1 manifest") }
    // Only this decoding copy is normalized. Opening and copying preserve manifestData bytes.
    var seen = Set<String>()
    object["categories"] = (object["categories"] as! [String]).compactMap { value -> String? in
      let mapped = (SlopCategory(rawValue: value) ?? .other).rawValue
      return seen.insert(mapped).inserted ? mapped : nil
    }
    var presentation = object["presentation"] as! [String: Any]
    if let shape = presentation["shape"] as? String, Shape(rawValue: shape) == nil {
      presentation.removeValue(forKey: "shape")
    }
    if let background = presentation["background"] as? String, Background(rawValue: background) == nil {
      presentation.removeValue(forKey: "background")
    }
    object["presentation"] = presentation
    return object
  }

  private func validateState() throws {
    let fileManager = FileManager.default
    guard fileManager.fileExists(atPath: stateURL.path) else { return }
    let allowed = Set([
      "document.sqlite", "document.sqlite-journal", "writer.lock", "host.lock", "theme.json",
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

  private func validateSchemaMetadata() throws {
    let descriptor = try SlopFile.read(dataSchemaURL, within: rootURL, maximumBytes: 1024 * 1024)
    guard let header = try JSONSerialization.jsonObject(with: descriptor) as? [String: Any],
      header["format"] as? Int == 1
    else { throw SlopPackageError.invalid("unsupported v1 document descriptor") }
    _ = try JSONSerialization.jsonObject(with: SlopFile.read(initialURL, within: rootURL))
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

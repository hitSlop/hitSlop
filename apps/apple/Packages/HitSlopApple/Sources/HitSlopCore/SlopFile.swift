import Foundation
import HitSlopCoreBinding
import ImageIO

/// A file the core refused to open as it was asked to.
public enum SlopError: LocalizedError, SlopDiagnosticProviding {
  case invalid(String)
  /// A template opened as a document: a document is created from it instead.
  case template
  public var errorDescription: String? {
    switch self {
    case .invalid(let value): "Invalid hitSlop file: \(value)"
    case .template: "This is a template; create a document from it first"
    }
  }
  public var diagnostic: SlopFailureContext { .init(.rejection, reason: .invalidFile) }
}

/// An operation that could not finish, said for the person. A file the core refused is a
/// `SlopError` instead.
public struct SlopFailure: LocalizedError, Sendable {
  public let message: String
  public init(_ message: String) { self.message = message }
  public var errorDescription: String? { message }
}

/// A slop, its storage or its document needs a newer hitSlop. Nothing was read past the
/// marker that said so, and nothing was written.
public struct SlopRequiresUpdate: LocalizedError, SlopDiagnosticProviding {
  public init() {}
  public var diagnostic: SlopFailureContext { .init(.rejection, reason: .requiresUpdate) }
  public var errorDescription: String? { "This slop needs a newer version of hitSlop. Update hitSlop to open it." }
  /// Whether the core refused for this reason.
  public static func matches(_ error: Error) -> Bool {
    if case let CoreError.Rejected(code, _, _) = error { return code == CoreErrorCode.requiresUpdate.rawValue }
    return false
  }
}

/// A hitSlop file, checked by the core: a template (the app its author built) or a document
/// (the app and its saved state). Swift decodes the manifest and the window skin; the core
/// owns every other rule.
public struct SlopFile: Sendable {
  /// The `.slop` file.
  public let url: URL
  public let kind: FileKind
  /// What the app expects of `ctx`; the page shell adapts to it.
  public let runtimeABI: Int
  /// The app's document descriptor (JSON), for the page and `slop schema`.
  public let descriptor: String
  /// The declared colors in the order the author wrote them.
  public let themeTokens: [ThemeToken]
  public let manifest: SlopManifest
  public let silhouette: SlopSilhouette
  /// The file's size in bytes.
  public let byteCount: Int64
  private let skinImage: CGImage?

  /// Opens and checks the file at `url` for display (the catalog, a template opened from
  /// Finder), without SQLite's quick check. A document an owner edits comes from its store
  /// instead (`init(url:opened:)`), so it is checked once.
  public init(url: URL) throws {
    let root = try Self.resolvedRoot(url)
    try self.init(url: root, opened: Self.opening { try openFile(path: root.path) })
  }

  /// The file at `url` as the core opened and checked it.
  public init(url root: URL, opened: OpenedFile) throws {
    self.url = root
    kind = opened.kind
    runtimeABI = Int(opened.runtimeAbi)
    descriptor = opened.descriptorJson
    themeTokens = opened.themeTokens
    byteCount = Int64(opened.byteCount)
    silhouette = SlopSilhouette(parsed: opened.silhouette)
    do {
      manifest = try JSONDecoder().decode(SlopManifest.self, from: Data(opened.manifestJson.utf8))
    } catch {
      throw SlopError.invalid(error.localizedDescription)
    }
    skinImage = try opened.skinPng.map(Self.decodeSkin)
  }

  /// Runs a core call that opens a file, reporting a newer file as `SlopRequiresUpdate`, a
  /// template opened as a document as `SlopError.template`, and any other refusal
  /// as `SlopError.invalid`. Storage failures, such as a busy writer lock or a missing
  /// file, pass through with their own message.
  public static func opening<T>(_ open: () throws -> T) throws -> T {
    do { return try open() }
    catch let error where SlopRequiresUpdate.matches(error) { throw SlopRequiresUpdate() }
    catch let CoreError.Rejected(code, message, _) {
      throw code == CoreErrorCode.isTemplate.rawValue ? SlopError.template : SlopError.invalid(message)
    }
  }

  /// A file's kind from its header checks alone, for deciding how to open it.
  public static func kind(of url: URL) throws -> FileKind {
    try opening { try fileKind(path: url.path) }
  }

  public var isSkinned: Bool { manifest.presentation.skin != nil }
  public var usesTransparentBackground: Bool {
    isSkinned || manifest.presentation.background == .transparent
  }
  public var isResizable: Bool { isSkinned ? false : manifest.presentation.resizable ?? true }
  /// The window skin, decoded when the file was opened.
  public var skin: CGImage? { skinImage }

  /// A `.slop` file's canonical URL: a regular file, never a link.
  public static func resolvedRoot(_ url: URL) throws -> URL {
    guard try url.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else {
      throw SlopError.invalid("a slop cannot be a symbolic link")
    }
    let root = SlopPath.canonical(url)
    guard root.pathExtension == "slop" else {
      throw SlopError.invalid("a slop must have a .slop extension")
    }
    guard (try? root.resourceValues(forKeys: [.isRegularFileKey]))?.isRegularFile == true else {
      throw SlopError.invalid("a slop must be a file")
    }
    return root
  }

  /// A document's canonical URL for its owner: `resolvedRoot`, on a local volume.
  public static func documentRoot(_ url: URL) throws -> URL {
    try SlopLocalDocument.requireLocal(url)
    return try resolvedRoot(url)
  }

  /// The skin's pixels. The core checked it is an RGBA PNG of the window's size.
  private static func decodeSkin(_ png: Data) throws -> CGImage {
    guard let source = CGImageSourceCreateWithData(png as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else { throw SlopError.invalid("window skin must be a valid PNG") }
    return image
  }
}

/// A slop's preview or icon artwork, read through the core.
public enum SlopArtwork {
  /// The core's artwork; its raw value is the name `slop screenshot --target` takes.
  public typealias Name = Artwork
  /// The first of `preferred` the file holds, in one read; nil when it holds none. Throws
  /// when the file can't be read now (busy, or mid-recovery), so a caller can tell that
  /// apart from a file without artwork.
  public static func first(_ url: URL, _ preferred: [Name]) throws -> (name: Name, png: Data)? {
    guard let image = try fileArtwork(path: url.path, preferred: preferred) else { return nil }
    return (image.name, image.png)
  }
  /// One artwork, or nil when the file has none or can't be read now.
  public static func png(_ url: URL, _ name: Name) -> Data? {
    (try? first(url, [name]))??.png
  }
}

extension Artwork: CaseIterable, RawRepresentable {
  public static var allCases: [Artwork] { [.preview, .icon] }
  public init?(rawValue: String) {
    guard let artwork = Self.allCases.first(where: { $0.rawValue == rawValue }) else { return nil }
    self = artwork
  }
  /// The name the file and a build's stage give it.
  public var rawValue: String {
    switch self {
    case .preview: "preview"
    case .icon: "icon"
    }
  }
}

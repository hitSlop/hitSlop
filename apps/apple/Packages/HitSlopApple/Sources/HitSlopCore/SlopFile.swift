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
}

/// A failure the core reported, said for the person.
extension OwnerFailure: LocalizedError {
  public var errorDescription: String? { message }
  /// The core error code a refusal names.
  public var refusal: CoreErrorCode? { reason.flatMap(CoreErrorCode.init(rawValue:)) }
}

/// A hitSlop file, checked by the core: a template (the app its author built) or a document
/// (the app and its saved state). Rust supplies typed metadata and window semantics;
/// Swift decodes only the already validated PNG for display.
public struct SlopFile: Sendable {
  /// The `.slop` file.
  public let url: URL
  public let kind: FileKind
  /// The app's document descriptor (JSON), for the page and `slop schema`.
  public let descriptor: String
  /// The declared colors in the order the author wrote them.
  public let themeTokens: [ThemeToken]
  public let metadata: AppMetadata
  public let window: WindowDefinition
  public let views: Views
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
    descriptor = opened.app.descriptorJson
    themeTokens = opened.app.theme
    byteCount = Int64(opened.byteCount)
    metadata = opened.app.metadata
    window = opened.app.window
    views = opened.app.views
    switch window {
    case .standard(_, _, _, _, _, _, _, let shape): silhouette = SlopSilhouette(parsed: shape)
    case .skin:
      silhouette = SlopSilhouette(
        parsed: .radii(horizontal: [.init(value: 0, percent: false)], vertical: [.init(value: 0, percent: false)]))
    }
    skinImage = try opened.skinPng.map(Self.decodeSkin)
  }

  /// Runs a core call that opens a file, reporting a newer file as `SlopRequiresUpdate`, a
  /// template opened as a document as `SlopError.template`, and any other refusal
  /// as `SlopError.invalid`. Storage failures, such as a busy writer lock or a missing
  /// file, pass through with their own message.
  public static func opening<T>(_ open: () throws -> T) throws -> T {
    do { return try open() } catch CoreError.Failure(let failure) {
      guard failure.kind == .rejected else { throw failure }
      switch failure.refusal {
      case .requiresUpdate: throw SlopRequiresUpdate()
      case .isTemplate: throw SlopError.template
      default: throw SlopError.invalid(failure.message)
      }
    }
  }

  /// A file's kind from its header checks alone, for deciding how to open it.
  public static func kind(of url: URL) throws -> FileKind {
    try opening { try fileKind(path: url.path) }
  }

  public var isSkinned: Bool { if case .skin = window { true } else { false } }
  public var width: Int {
    switch window {
    case .standard(let w, _, _, _, _, _, _, _), .skin(let w, _, _, _): Int(w)
    }
  }
  public var height: Int {
    switch window {
    case .standard(_, let h, _, _, _, _, _, _), .skin(_, let h, _, _): Int(h)
    }
  }
  public var lockAspect: Bool { if case .standard(_, _, _, _, _, let lock, _, _) = window { lock } else { true } }
  /// What the window shows behind the page.
  public var backdrop: SlopBackdrop {
    switch window {
    case .skin: .skin
    case .standard(_, _, _, _, _, _, let background, _):
      switch background {
      case nil: .window
      case .transparent: .clear
      case .glass: .glass
      }
    }
  }
  public var isResizable: Bool {
    if case .standard(_, _, _, _, let resizable, _, _, _) = window { resizable } else { false }
  }
  /// Host fullscreen is opt-in and independent of desktop resizing.
  public var isFullscreenable: Bool {
    switch window {
    case .standard(_, _, let enabled, _, _, _, _, _), .skin(_, _, let enabled, _): enabled
    }
  }
  public var fitsFullscreen: Bool {
    switch window {
    case .standard(_, _, _, let fit, _, _, _, _): fit
    case .skin: true
    }
  }
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

/// What a window shows behind its page, from the presentation.
public enum SlopBackdrop: Sendable {
  /// The system window color, under a page that draws its own background.
  case window
  /// Nothing: the desktop shows wherever the page is transparent.
  case clear
  /// A frosted material: the blurred desktop shows through a translucent page.
  case glass
  /// The template's PNG skin.
  case skin
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

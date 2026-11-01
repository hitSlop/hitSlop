import Foundation
import ImageIO
import PopCoreBinding

/// A file the core refused to open as it was asked to.
public enum PopError: LocalizedError, PopDiagnosticProviding {
  case invalid(String)
  /// A template opened as a document: a document is created from it instead.
  case template
  public var errorDescription: String? {
    switch self {
    case .invalid(let value): "Invalid pop file: \(value)"
    case .template: "This is a template; create a document from it first"
    }
  }
  public var diagnostic: PopFailureContext { .init(.rejection, reason: .invalidFile) }
}

/// An operation that could not finish, said for the person. A file the core refused is a
/// `PopError` instead.
public struct PopFailure: LocalizedError, Sendable {
  public let message: String
  public init(_ message: String) { self.message = message }
  public var errorDescription: String? { message }
}

/// A pop, its storage or its document needs a newer pop. Nothing was read past the
/// marker that said so, and nothing was written.
public struct PopRequiresUpdate: LocalizedError, PopDiagnosticProviding {
  public init() {}
  public var diagnostic: PopFailureContext { .init(.rejection, reason: .requiresUpdate) }
  public var errorDescription: String? { "This pop needs a newer version of pop. Update pop to open it." }
}

/// A failure the core reported, said for the person.
extension OwnerFailure: LocalizedError {
  public var errorDescription: String? { message }
}

/// A pop file, checked by the core: a template (the app its author built) or a document
/// (the app and its saved state). Rust supplies typed metadata and window semantics;
/// Swift decodes only the already validated PNG for display.
public struct PopFile: Sendable {
  /// The `.pop` file.
  public let url: URL
  public let kind: FileKind
  public let packageFormat: Int
  public let runtimeABI: Int
  /// The app's document descriptor (JSON), for the page and `pop schema`.
  public let descriptor: String
  /// The declared colors in the order the author wrote them.
  public let themeTokens: [ThemeToken]
  public let metadata: AppMetadata
  public let window: WindowDefinition
  public let views: Views
  /// The devices and host services the app declared that this build implements; the host
  /// grants nothing else.
  public let capabilities: Set<Capability>
  public let outline: PopOutline
  public let lockAspect: Bool
  /// What the window shows behind the page.
  public let backdrop: PopBackdrop
  public let isResizable: Bool
  public let fitsFullscreen: Bool
  /// The file's size in bytes.
  public let byteCount: Int64

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
    packageFormat = Int(opened.packageFormat)
    runtimeABI = Int(opened.runtimeAbi)
    descriptor = opened.app.descriptorJson
    themeTokens = opened.app.theme
    byteCount = Int64(opened.byteCount)
    metadata = opened.app.metadata
    window = opened.app.window
    views = opened.app.views
    capabilities = Set(opened.app.capabilities)
    switch window.frame {
    case .standard(let frame):
      outline = .shape(PopSilhouette(parsed: frame.shape))
      lockAspect = frame.lockAspect
      isResizable = frame.resizable
      fitsFullscreen = frame.fullscreenFit
      backdrop =
        switch frame.background {
        case nil: .window
        case .transparent: .clear
        case .glass: .glass
        }
    case .skin:
      guard let png = opened.skinPng else { throw PopError.invalid("window skin is missing") }
      outline = .skin(try Self.decodeSkin(png))
      lockAspect = true
      isResizable = false
      fitsFullscreen = true
      backdrop = .skin
    }
  }

  /// Runs a core call that opens a file, reporting a newer file as `PopRequiresUpdate`, a
  /// template opened as a document as `PopError.template`, and any other refusal
  /// as `PopError.invalid`. Storage failures, such as a busy writer lock or a missing
  /// file, pass through with their own message.
  public static func opening<T>(_ open: () throws -> T) throws -> T {
    do { return try open() } catch CoreError.Failure(let failure) {
      guard failure.kind == .rejected else { throw failure }
      switch failure.reason {
      case .requiresUpdate: throw PopRequiresUpdate()
      case .isTemplate: throw PopError.template
      default: throw PopError.invalid(failure.message)
      }
    }
  }

  /// A file's kind from its header checks alone, for deciding how to open it.
  public static func kind(of url: URL) throws -> FileKind {
    try opening { try fileKind(path: url.path) }
  }

  /// Deletes a closed local document; the core refuses another writer or a template.
  public static func delete(_ url: URL) throws {
    try opening { try deleteDocument(path: url.path) }
  }

  #if os(macOS)
    /// User-requested relocation. The core holds the source lease through validated copy
    /// and coordinated cleanup; a cleanup warning means both files were retained.
    public static func moveToLocalFolder(_ source: URL) throws -> (url: URL, warning: String?) {
      let folder = try PopSaveLocation.localFolder()
      let result = try opening { try relocateDocument(source: source.path, folder: folder.path) }
      let url = URL(fileURLWithPath: result.path)
      PopSaveLocation.remember(url)
      return (url, result.cleanupError)
    }

  #endif

  public var width: Int { Int(window.width) }
  public var height: Int { Int(window.height) }
  /// Host fullscreen is opt-in and independent of desktop resizing.
  public var isFullscreenable: Bool { window.fullscreenable }

  /// A `.pop` file's canonical URL: a regular file, never a link.
  public static func resolvedRoot(_ url: URL) throws -> URL {
    guard try url.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else {
      throw PopError.invalid("a pop cannot be a symbolic link")
    }
    let root = PopPath.canonical(url)
    guard root.pathExtension == "pop" else {
      throw PopError.invalid("a pop must have a .pop extension")
    }
    guard (try? root.resourceValues(forKeys: [.isRegularFileKey]))?.isRegularFile == true else {
      throw PopError.invalid("a pop must be a file")
    }
    return root
  }

  /// A document's canonical URL for its owner: `resolvedRoot`, on a local volume.
  public static func documentRoot(_ url: URL) throws -> URL {
    #if os(macOS)
      try PopLocalDocument.requireLocal(url)
    #endif
    return try resolvedRoot(url)
  }

  /// The skin's pixels. The core checked it is an RGBA PNG of the window's size.
  private static func decodeSkin(_ png: Data) throws -> CGImage {
    guard let source = CGImageSourceCreateWithData(png as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else { throw PopError.invalid("window skin must be a valid PNG") }
    return image
  }
}

/// The checked window outline: vector geometry or the skin's decoded pixels.
public enum PopOutline: Sendable {
  case shape(PopSilhouette)
  case skin(CGImage)
}

/// What a window shows behind its page, from the presentation.
public enum PopBackdrop: Sendable {
  /// The system window color, under a page that draws its own background.
  case window
  /// Nothing: the desktop shows wherever the page is transparent.
  case clear
  /// A frosted material: the blurred desktop shows through a translucent page.
  case glass
  /// The template's PNG skin.
  case skin
}

/// A pop's preview or icon artwork, read through the core.
public enum PopArtwork {
  /// The core's artwork; its raw value is the `target` the engine's `artwork.export` takes.
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

import AppKit
import ArgumentParser
import Foundation
import HitSlopCore
import HitSlopHost
import HitSlopDocument

@main struct NativeCLI: AsyncParsableCommand {
  /// Select the public adapter before ArgumentParser can execute any document command.
  /// Unversioned callers retain protocol 1 even when a later adapter is introduced.
  static func main() async {
    do {
      var arguments = Array(CommandLine.arguments.dropFirst())
      var version = 1
      if arguments.first == "--client-protocol" {
        guard arguments.count >= 2, let selected = Int(arguments[1]), selected >= 1 else {
          throw ValidationError("--client-protocol requires a positive integer")
        }
        version = selected
        arguments.removeFirst(2)
      }
      switch version {
      case 1:
        var command = try parseAsRoot(arguments)
        if var asynchronous = command as? AsyncParsableCommand {
          try await asynchronous.run()
        } else {
          try command.run()
        }
      default:
        throw ValidationError("Unsupported command protocol \(version); update hitSlop or the calling CLI")
      }
    } catch { exit(withError: error) }
  }

  static let configuration = CommandConfiguration(
    commandName: "hitslop-native", abstract: "Read, edit, open, and export hitSlop documents.",
    subcommands: [
      Theme.self, Attachments.self, Screenshot.self, Export.self,
      Get.self,
      Apply.self, Batch.self, Import.self, Compact.self, Create.self, Open.self,
    ] + debugCommands)

  @Flag(name: .customLong("core-build"), help: "Print the embedded document core build ID.")
  var coreBuild = false
  @Flag(name: .customLong("protocol"), help: "Print the command protocols this helper serves.")
  var commandProtocol = false

  func run() async throws {
    if commandProtocol {
      // The range a CLI checks its own protocol against; any compatible app build serves it.
      print(#"{"version":\#(HelperProtocol.version),"minimum":\#(HelperProtocol.minimum)}"#)
      return
    }
    guard coreBuild else { throw CleanExit.helpRequest(self) }
    print(DocumentOwner.coreBuildID)
  }

  private static var debugCommands: [ParsableCommand.Type] {
    #if DEBUG
    [StorageProbe.self]
    #else
    []
    #endif
  }
}

struct Screenshot: AsyncParsableCommand {
  enum Target: String, ExpressibleByArgument { case preview, icon }
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @Option var target: Target = .preview
  @Flag var ifPresent = false
  @MainActor func run() async throws {
    bootstrapApp()
    let data: Data?
    switch target {
    case .preview: data = try await SlopRenderer.previewPNGData(url: file)
    case .icon: data = try await SlopRenderer.iconPNGData(url: file)
    }
    guard let data else {
      if ifPresent { return }
      throw ValidationError("The slop does not define a \(target.rawValue) render target.")
    }
    try data.write(to: output, options: .atomic)
    print(output.path)
  }
}
extension ExportFormat: ExpressibleByArgument {}
struct Export: AsyncParsableCommand {
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @Option var format: ExportFormat
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    bootstrapApp()
    try await SlopRenderer.exportDocument(
      url: file, format: format, output: output)
    print(output.path)
  }
}
struct DocumentArguments: ParsableArguments {
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
}
/// A windowless app for WebKit rendering. Document commands don't use AppKit, so they
/// never start one.
@MainActor func bootstrapApp() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
}
/// Runs one document command and prints its JSON output.
@MainActor func printDocument(_ document: URL, snapshot: Bool = false, _ make: @escaping @Sendable (_ documentPath: String) -> SocketRequest) async throws {
  let result = try await DocumentCommand.run(url: document, snapshot: snapshot) { make($0) }
  print(String(decoding: result, as: UTF8.self))
}
struct Get: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Flag var snapshot = false
  @MainActor func run() async throws {
    let snapshot = snapshot
    try await printDocument(document.file, snapshot: snapshot) {
      .get(.init(documentPath: $0))
    }
  }
}
struct Apply: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Option var op: String
  @MainActor func run() async throws {
    let op = op
    guard (try? JSONSerialization.jsonObject(with: Data(op.utf8))) is [String: Any]
    else { throw ValidationError("--op must be one JSON object") }
    try await printDocument(document.file) { .batch(.init(documentPath: $0, epoch: "", ops: "[" + op + "]")) }
  }
}
struct Batch: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Option var ops: String
  @MainActor func run() async throws {
    let ops = ops
    try await printDocument(document.file) { .batch(.init(documentPath: $0, epoch: "", ops: ops)) }
  }
}
/// One `replace` operation from a file: the value at `--path` (the whole document by
/// default) becomes the file's JSON, applied like `batch`.
struct Import: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @Option var path = "[]"
  @MainActor func run() async throws {
    let bytes = try CommandInput.read(file, maximumBytes: Limits.socketRequest - 4096)
    // Each is one complete JSON value, so splicing them into the operation cannot change
    // its shape; the core parses and validates the result.
    guard let value = String(data: bytes, encoding: .utf8),
      (try? JSONSerialization.jsonObject(with: bytes, options: .fragmentsAllowed)) != nil
    else { throw ValidationError("The file must hold one JSON value in UTF-8") }
    guard (try? JSONSerialization.jsonObject(with: Data(path.utf8))) is [Any]
    else { throw ValidationError("--path must be a JSON array, such as '[\"rows\"]'") }
    let ops = #"[{"type":"replace","path":"# + path + #","value":"# + value + "}]"
    try await printDocument(document.file) { .batch(.init(documentPath: $0, epoch: "", ops: ops)) }
  }
}
struct Compact: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @MainActor func run() async throws {
    try await printDocument(document.file) { .compact(.init(documentPath: $0, epoch: "")) }
  }
}
struct Create: AsyncParsableCommand {
  @Option(name: .customLong("from"), transform: URL.init(fileURLWithPath:)) var source: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    try FileManager.default.createDirectory(
      at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
    print(try SlopFile.create(from: source, to: output).path)
  }
}

struct Open: AsyncParsableCommand {
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @MainActor func run() async throws {
    // The header decides; the app checks the whole file when it opens it.
    guard try SlopFile.kind(of: file) == .document else {
      throw ValidationError(SlopError.template.localizedDescription)
    }
    guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.hitslop.app")
    else {
      throw ValidationError("Install hitSlop.app to open documents")
    }
    let configuration = NSWorkspace.OpenConfiguration()
    configuration.activates = true
    try await NSWorkspace.shared.open(
      [file], withApplicationAt: app, configuration: configuration)
    print(file.path)
  }
}

#if DEBUG
struct StorageProbe: ParsableCommand {
  static let configuration = CommandConfiguration(shouldDisplay: false)
  @Argument var root: String
  @Argument var phase: String
  @Argument var marker: String
  func run() throws {
    try DebugStorageProbe.run([root, phase, marker])
  }
}

#endif

struct Theme: AsyncParsableCommand {
  static let configuration = CommandConfiguration(subcommands: [
    ThemeGet.self, ThemeSet.self, ThemeReset.self, ThemeExport.self, ThemeImport.self,
  ])
}
struct ThemeGet: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "get")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @MainActor func run() async throws {
    try await printDocument(document) { .themeGet(.init(documentPath: $0)) }
  }
}
struct ThemeSet: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "set")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Option var values: String
  @MainActor func run() async throws {
    guard let values = try? JSONSerialization.jsonObject(with: Data(values.utf8)) as? [String: String] else {
      throw ValidationError("--values must be a JSON object of theme tokens and values")
    }
    try await printDocument(document) { .themeSet(.init(documentPath: $0, epoch: "", values: values)) }
  }
}
struct ThemeReset: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "reset")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Option var token: String?
  @MainActor func run() async throws {
    let token = token
    try await printDocument(document) { .themeReset(.init(documentPath: $0, epoch: "", token: token)) }
  }
}
struct ThemeExport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "export")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL?
  @MainActor func run() async throws {
    let file = try await DocumentCommand.run(url: document) { .themeExport(.init(documentPath: $0)) }
    guard let output else { return print(String(decoding: file, as: UTF8.self)) }
    try (file + Data("\n".utf8)).write(to: output, options: .atomic)
    print(output.path)
  }
}
struct ThemeImport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "import")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @MainActor func run() async throws {
    let handle = try FileHandle(forReadingFrom: file)
    defer { try? handle.close() }
    let bytes = try handle.read(upToCount: Limits.themeFile + 1) ?? Data()
    guard bytes.count <= Limits.themeFile else { throw ValidationError("Theme file is too large") }
    guard let text = String(data: bytes, encoding: .utf8) else { throw ValidationError("Theme file must be UTF-8") }
    try await printDocument(document) { .themeImport(.init(documentPath: $0, epoch: "", file: text)) }
  }
}

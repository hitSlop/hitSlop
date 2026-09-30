import AppKit
import ArgumentParser
import Foundation
import HitSlopCore
import HitSlopHost
import HitSlopDocument

@main struct NativeCLI: AsyncParsableCommand {
  static let configuration = CommandConfiguration(
    commandName: "hitslop-native", abstract: "Read, edit, open, and export hitSlop documents.",
    subcommands: [
      Theme.self, Attachments.self, Screenshot.self, Export.self,
      Get.self, Schema.self,
      Apply.self, Batch.self, Compact.self, Create.self, Open.self,
    ] + debugCommands)

  @Flag(name: .customLong("core-build"), help: "Print the embedded document core build ID.")
  var coreBuild = false

  func run() async throws {
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
  @Argument(transform: URL.init(fileURLWithPath:)) var package: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @Option var target: Target = .preview
  @Flag var ifPresent = false
  @MainActor func run() async throws {
    bootstrapApp()
    let data: Data?
    switch target {
    case .preview: data = try await SlopRenderer.previewPNGData(packageURL: package)
    case .icon: data = try await SlopRenderer.iconPNGData(packageURL: package)
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
  @Argument(transform: URL.init(fileURLWithPath:)) var package: URL
  @Option var format: ExportFormat
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    bootstrapApp()
    try await SlopRenderer.exportDocument(
      packageURL: package, format: format, output: output)
    print(output.path)
  }
}
struct DocumentArguments: ParsableArguments {
  @Argument(transform: URL.init(fileURLWithPath:)) var package: URL
}
/// A windowless app for WebKit rendering. Document commands don't use AppKit, so they
/// never start one.
@MainActor func bootstrapApp() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
}
/// Runs one document command and prints its JSON output.
@MainActor func printDocument(_ document: URL, _ make: @escaping @Sendable (_ id: String, _ documentPath: String) -> SocketRequest) async throws {
  let result = try await DocumentCommand.run(url: document) { make(UUID().uuidString, $0) }
  print(String(decoding: result, as: UTF8.self))
}
struct Get: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Flag var snapshot = false
  @MainActor func run() async throws {
    let snapshot = snapshot
    try await printDocument(document.package) {
      snapshot ? .snapshot(.init(id: $0, documentPath: $1)) : .get(.init(id: $0, documentPath: $1))
    }
  }
}
struct Schema: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  func run() throws {
    let package = try SlopPackage(rootURL: document.package)
    print(String(decoding: try SlopFile.read(package.dataSchemaURL, within: package.rootURL, maximumBytes: 1_048_576), as: UTF8.self))
  }
}
struct Apply: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Option var op: String
  @MainActor func run() async throws {
    let op = op
    try await printDocument(document.package) { .apply(.init(id: $0, documentPath: $1, epoch: "", op: op)) }
  }
}
struct Batch: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @Option var ops: String
  @MainActor func run() async throws {
    let ops = ops
    try await printDocument(document.package) { .batch(.init(id: $0, documentPath: $1, epoch: "", ops: ops)) }
  }
}
struct Compact: AsyncParsableCommand {
  @OptionGroup var document: DocumentArguments
  @MainActor func run() async throws {
    try await printDocument(document.package) { .compact(.init(id: $0, documentPath: $1, epoch: "")) }
  }
}
struct Create: AsyncParsableCommand {
  @Option(name: .customLong("from"), transform: URL.init(fileURLWithPath:)) var source: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    let output = SlopDuplicator.packageURL(self.output)
    guard !SlopTemplateLocation.isMaster(output) else {
      throw ValidationError("A document cannot be created in the template cache")
    }
    try FileManager.default.createDirectory(
      at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
    let package = try SlopDuplicator.duplicate(from: source, to: output, fromTemplate: true)
    SlopPreviewWriter.installAuthoredIcon(for: package)
    print(package.rootURL.path)
  }
}

struct Open: AsyncParsableCommand {
  @Argument(transform: URL.init(fileURLWithPath:)) var package: URL
  @MainActor func run() async throws {
    _ = try SlopPackage(rootURL: package)
    guard !SlopTemplateLocation.isMaster(package) else {
      throw ValidationError(SlopTemplateLocation.writableCopyRequired)
    }
    guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.hitslop.app")
    else {
      throw ValidationError("Install hitSlop.app to open documents")
    }
    let configuration = NSWorkspace.OpenConfiguration()
    configuration.activates = true
    try await NSWorkspace.shared.open(
      [package], withApplicationAt: app, configuration: configuration)
    print(package.path)
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
    ThemeGet.self, ThemeSet.self, ThemeReset.self,
  ])
}
struct ThemeGet: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "get")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @MainActor func run() async throws {
    try await printDocument(document) { .themeGet(.init(id: $0, documentPath: $1)) }
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
    try await printDocument(document) { .themeSet(.init(id: $0, documentPath: $1, epoch: "", values: values)) }
  }
}
struct ThemeReset: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "reset")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Option var token: String?
  @MainActor func run() async throws {
    let token = token
    try await printDocument(document) { .themeReset(.init(id: $0, documentPath: $1, epoch: "", token: token)) }
  }
}

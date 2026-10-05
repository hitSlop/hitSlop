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
      clientProtocol = version
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
    subcommands: [Request.self, Screenshot.self, Create.self, Open.self])

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
}

/// The command protocol the caller named, set once before any command runs; unversioned
/// callers speak protocol 1.
nonisolated(unsafe) var clientProtocol = 1

/// One document request: a `SocketRequest` (JSON) on standard input, without its
/// protocol, and its `SocketReply` on standard output. It reaches the document's live owner, or an owner
/// opened here; `export` of a closed document renders its saved state here.
struct Request: AsyncParsableCommand {
  @MainActor func run() async throws {
    var input = Data()
    while let chunk = try FileHandle.standardInput.read(upToCount: 64 * 1024), !chunk.isEmpty {
      input.append(chunk)
      guard input.count <= Limits.socketAttachment else {
        throw ValidationError("The request exceeds \(Limits.socketAttachment) bytes")
      }
    }
    let method = (try? JSONSerialization.jsonObject(with: input) as? [String: Any])?["method"] as? String
    let export = method == SocketRequest.Method.export.rawValue
    if export { bootstrapApp() }
    let reply = await DocumentCommand.run(json: input, protocol: clientProtocol, export: export ? Self.exportClosed : nil)
    FileHandle.standardOutput.write(reply + [10])
  }
  @MainActor private static func exportClosed(_ root: URL, _ format: ExportFormat, _ output: URL, _ deadline: NativeCommandDeadline) async throws {
    _ = try await SlopRenderer.exportClosed(root, format: format, output: output, deadline: deadline)
  }
}

struct Screenshot: AsyncParsableCommand {
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @Option var target: SlopArtwork.Name = .preview
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
/// A windowless app for WebKit rendering. Document commands don't use AppKit, so they
/// never start one.
@MainActor func bootstrapApp() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
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

extension SlopArtwork.Name: ExpressibleByArgument {}

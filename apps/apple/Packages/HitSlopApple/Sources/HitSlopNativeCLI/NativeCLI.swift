import AppKit
import Foundation
import HitSlopCore
import HitSlopDocument
import HitSlopHost

/// The renderer's private JSON boundary. Protocol refusal precedes stdin and AppKit.
@main struct NativeCLI {
  @MainActor static func main() async {
    var arguments = Array(CommandLine.arguments.dropFirst())
    var protocolVersion: Int?
    if arguments.first == "--client-protocol" {
      guard arguments.count >= 2, let version = Int(arguments[1]) else {
        return bootstrapFailure("--client-protocol requires an integer")
      }
      if version != HelperProtocol.version {
        return bootstrapFailure(
          version > HelperProtocol.version
            ? "This command needs a newer hitSlop app; update hitSlop"
            : "This hitSlop app needs a newer command line; update the hitSlop CLI")
      }
      protocolVersion = version
      arguments.removeFirst(2)
    }
    if arguments == ["--protocol"] {
      print(#"{"version":\#(HelperProtocol.version)}"#)
      return
    }
    if arguments == ["--core-build"] {
      print(DocumentOwner.coreBuildID)
      return
    }
    guard let version = protocolVersion, arguments.isEmpty else {
      return bootstrapFailure("Use --client-protocol N and one JSON request on stdin")
    }
    let reply: Data
    do {
      var input = Data()
      while let chunk = try FileHandle.standardInput.read(upToCount: 64 * 1024), !chunk.isEmpty {
        input.append(chunk)
        guard input.count <= Limits.socketRequest else {
          return write(
            .failure(SocketFailure(error: "Native request is too large", code: .rejected, reason: .tooLarge)))
        }
      }
      guard Envelope.valid(.nativeRequest, input),
        let json = try JSONSerialization.jsonObject(with: input) as? [String: Any]
      else {
        return write(
          .failure(
            SocketFailure(
              error: "Invalid native request; send document edits to slop-engine", code: .rejected,
              reason: .invalidRequest)))
      }
      let request = try NativeRequest(json: json)
      switch request {
      case .export:
        bootstrapApp()
        reply = await DocumentCommand.run(json: input, protocol: version, export: exportClosed)
      case .screenshot(let request):
        bootstrapApp()
        let file = URL(fileURLWithPath: request.documentPath)
        let output = URL(fileURLWithPath: request.output)
        let data: Data?
        switch request.target {
        case .preview: data = try await SlopRenderer.previewPNGData(url: file)
        case .icon: data = try await SlopRenderer.iconPNGData(url: file)
        }
        guard let data else {
          if request.ifPresent { return write(.screenshot(output: nil)) }
          return write(
            .failure(
              SocketFailure(
                error: "The slop does not define a \(request.target.rawValue) render target.", code: .rejected,
                reason: .invalidRequest)))
        }
        try data.write(to: output, options: .atomic)
        reply = NativeReply.screenshot(output: output.path).encoded()
      case .open(let request):
        let file = URL(fileURLWithPath: request.documentPath)
        guard try SlopFile.kind(of: file) == .document else { throw SlopError.template }
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.hitslop.app") else {
          return write(
            .failure(
              SocketFailure(error: "Install hitSlop.app to open documents", code: .rejected, reason: .invalidRequest)))
        }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        try await NSWorkspace.shared.open([file], withApplicationAt: app, configuration: configuration)
        reply = NativeReply.open(documentPath: file.path).encoded()
      }
    } catch is SlopRequiresUpdate {
      return write(
        .failure(
          SocketFailure(error: SlopRequiresUpdate().localizedDescription, code: .rejected, reason: .requiresUpdate)))
    } catch let error as SlopError {
      return write(.failure(SocketFailure(error: error.localizedDescription, code: .rejected, reason: .invalidRequest)))
    } catch {
      return write(.failure(SocketFailure(error: error.localizedDescription, code: .unknownOutcome)))
    }
    FileHandle.standardOutput.write(reply + [10])
  }
  private static func bootstrapFailure(_ message: String) {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    Foundation.exit(2)
  }
  private static func write(_ reply: NativeReply) {
    FileHandle.standardOutput.write(reply.encoded() + [10])
  }
  @MainActor private static func exportClosed(
    _ root: URL, _ format: ExportFormat, _ output: URL, _ deadline: NativeCommandDeadline
  ) async throws {
    _ = try await SlopRenderer.exportClosed(root, format: format, output: output, deadline: deadline)
  }
}
/// A windowless app for WebKit rendering.
@MainActor private func bootstrapApp() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
}

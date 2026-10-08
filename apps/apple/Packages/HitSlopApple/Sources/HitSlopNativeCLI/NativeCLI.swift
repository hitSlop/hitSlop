import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
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
        if input.count > Limits.socketRequest { break }
      }
      let request = try decodeNativeRequest(input: input)
      switch request {
      case .export:
        bootstrapApp()
        reply = await DocumentCommand.run(json: input, protocol: version, export: exportClosed)
      case .screenshot(let documentPath, let outputPath, let target, let ifPresent):
        bootstrapApp()
        let file = URL(fileURLWithPath: documentPath)
        let output = URL(fileURLWithPath: outputPath)
        let destination = try SlopScreenshotDestination(output: output, source: file)
        let data: Data?
        switch target {
        case .preview: data = try await SlopRenderer.previewPNGData(url: file)
        case .icon: data = try await SlopRenderer.iconPNGData(url: file)
        }
        guard let data else {
          if ifPresent { return write(.screenshot(output: nil)) }
          let name = target == .preview ? "preview" : "icon"
          return write(
            .failure(
              error: "The slop does not define a \(name) render target.", code: .rejected,
              reason: .invalidRequest, opIndex: nil))
        }
        try destination.publish(data)
        reply = encoded(.screenshot(output: output.path))
      case .open(let documentPath):
        let file = URL(fileURLWithPath: documentPath)
        guard try SlopFile.kind(of: file) == .document else { throw SlopError.template }
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.hitslop.app") else {
          return write(
            .failure(
              error: "Install hitSlop.app to open documents", code: .rejected,
              reason: .invalidRequest, opIndex: nil))
        }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        try await NSWorkspace.shared.open([file], withApplicationAt: app, configuration: configuration)
        reply = encoded(.open(documentPath: file.path))
      }
    } catch let failure as OwnerFailure where failure.kind == .rejected {
      return write(.failure(error: failure.message, code: .rejected, reason: failure.refusal, opIndex: failure.opIndex))
    } catch NativeRefusal.Refused(let reply) {
      return write(reply)
    } catch is SlopRequiresUpdate {
      return write(
        .failure(
          error: SlopRequiresUpdate().localizedDescription, code: .rejected,
          reason: .requiresUpdate, opIndex: nil))
    } catch let error as SlopError {
      return write(
        .failure(
          error: error.localizedDescription, code: .rejected,
          reason: .invalidRequest, opIndex: nil))
    } catch {
      return write(.failure(error: error.localizedDescription, code: .unknownOutcome, reason: nil, opIndex: nil))
    }
    FileHandle.standardOutput.write(reply + [10])
  }
  private static func bootstrapFailure(_ message: String) {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    Foundation.exit(2)
  }
  private static func encoded(_ reply: NativeWireReply) -> Data {
    Data(encodeNativeReply(reply: reply).utf8)
  }
  private static func write(_ reply: NativeWireReply) {
    FileHandle.standardOutput.write(encoded(reply) + [10])
  }
  @MainActor private static func exportClosed(
    _ root: URL, _ format: ExportFormat, _ output: URL, _ deadline: NativeCommandDeadline
  ) async throws {
    try await SlopRenderer.exportClosed(root, format: format, output: output, deadline: deadline)
  }
}
/// A windowless app for WebKit rendering.
@MainActor private func bootstrapApp() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
}

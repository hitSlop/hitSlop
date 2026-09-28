import AppKit
import ArgumentParser
import Foundation
import HitSlopCore
import HitSlopDocument
import UniformTypeIdentifiers

struct Attachments: AsyncParsableCommand {
  static let configuration = CommandConfiguration(subcommands: [AttachmentList.self, AttachmentImport.self, AttachmentExport.self])
}
@MainActor private func prepareAttachments() {
  _ = NSApplication.shared
  NSApp.setActivationPolicy(.prohibited)
}
struct AttachmentList: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "list")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @MainActor func run() async throws {
    prepareAttachments()
    print(String(decoding: try await DocumentCommand.run(method: "attachments.list", url: document), as: UTF8.self))
  }
}
struct AttachmentImport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "import")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @MainActor func run() async throws {
    prepareAttachments()
    let source = file.standardizedFileURL
    guard source.lastPathComponent.count <= 255 else { throw ValidationError("Filename exceeds 255 characters") }
    let bytes = try SlopFile.read(source, within: source.deletingLastPathComponent(), maximumBytes: SlopAttachments.maximumBytes)
    let data = try await DocumentCommand.run(method: "attachments.put", url: document, attachmentBytes: bytes)
    var ref = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    ref["name"] = source.lastPathComponent
    ref["mimeType"] = UTType(filenameExtension: source.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
    print(String(decoding: try JSONSerialization.data(withJSONObject: ref, options: [.prettyPrinted, .sortedKeys]), as: UTF8.self))
  }
}
struct AttachmentExport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "export")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Argument var id: String
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    prepareAttachments()
    let destination = output.standardizedFileURL.resolvingSymlinksInPath()
    let root = document.standardizedFileURL.resolvingSymlinksInPath().path
    guard destination.path != root, !destination.path.hasPrefix(root + "/") else {
      throw ValidationError("Export destination must be outside the document package")
    }
    let response = try await DocumentCommand.run(method: "attachments.read", url: document, attachmentID: id)
    guard let value = try JSONSerialization.jsonObject(with: response) as? [String: String],
      let encoded = value["bytes"], let bytes = Data(base64Encoded: encoded) else {
      throw ValidationError("Invalid attachment response")
    }
    try bytes.write(to: destination, options: .withoutOverwriting)
    print(destination.path)
  }
}

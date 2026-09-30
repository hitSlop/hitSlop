import AppKit
import ArgumentParser
import Foundation
import HitSlopCore
import HitSlopDocument
import UniformTypeIdentifiers

struct Attachments: AsyncParsableCommand {
  static let configuration = CommandConfiguration(subcommands: [AttachmentList.self, AttachmentImport.self, AttachmentExport.self])
}
struct AttachmentList: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "list")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @MainActor func run() async throws {
    try await printDocument(document) { .attachmentsList(.init(id: $0, documentPath: $1)) }
  }
}
struct AttachmentImport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "import")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Argument(transform: URL.init(fileURLWithPath:)) var file: URL
  @MainActor func run() async throws {
    let source = file.standardizedFileURL
    let mimeType = UTType(filenameExtension: source.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
    guard source.lastPathComponent.utf8.count <= AttachmentLimits.name, mimeType.utf8.count <= AttachmentLimits.name else {
      throw ValidationError("File name or type exceeds \(AttachmentLimits.name) bytes")
    }
    let bytes = try SlopFile.read(source, within: source.deletingLastPathComponent(), maximumBytes: AttachmentLimits.file).base64EncodedString()
    let data = try await DocumentCommand.run(url: document) {
      .attachmentsPut(.init(id: UUID().uuidString, documentPath: $0, epoch: "", bytes: bytes))
    }
    var ref = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    ref["name"] = source.lastPathComponent
    ref["mimeType"] = mimeType
    print(String(decoding: try JSONSerialization.data(withJSONObject: ref, options: [.prettyPrinted, .sortedKeys]), as: UTF8.self))
  }
}
struct AttachmentExport: AsyncParsableCommand {
  static let configuration = CommandConfiguration(commandName: "export")
  @Argument(transform: URL.init(fileURLWithPath:)) var document: URL
  @Argument var id: String
  @Option(transform: URL.init(fileURLWithPath:)) var output: URL
  @MainActor func run() async throws {
    let destination = output.standardizedFileURL.resolvingSymlinksInPath()
    guard !SlopPath.contains(document, destination) else {
      throw ValidationError("Export destination must be outside the document package")
    }
    let id = id
    let response = try await DocumentCommand.run(url: document) {
      .attachmentsRead(.init(id: UUID().uuidString, documentPath: $0, attachmentID: id))
    }
    guard let value = try JSONSerialization.jsonObject(with: response) as? [String: String],
      let encoded = value["bytes"], let bytes = Data(base64Encoded: encoded) else {
      throw ValidationError("Invalid attachment response")
    }
    try bytes.write(to: destination, options: .withoutOverwriting)
    print(destination.path)
  }
}

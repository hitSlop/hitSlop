import AppKit
import Foundation
import CryptoKit
import HitSlopCore
import Testing
@testable import HitSlopWasm

@Suite(.serialized) struct AttachmentTests {
  func fixture() throws -> URL {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/generated/v1/native-fixtures/quick-checklist.slop", toPath: root.path)
    try SlopDuplicator.makeWritable(root)
    return root
  }
  @Test func opaqueFilesAreBoundedDeduplicatedAndVerified() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let store = try Storage(root: root)
    defer { store.close() }
    let data = Data("classic skin".utf8)
    let ref = try store.call(["method": "attachments.put", "bytes": data.base64EncodedString()])
    let id = try #require(ref["id"] as? String)
    #expect(SlopAttachments.validID(id))
    #expect(try SlopAttachments.read(id, in: root) == data)
    _ = try store.call(["method": "attachments.put", "bytes": data.base64EncodedString()])
    #expect(try SlopAttachments.list(in: root).count == 1)
    _ = try SlopPackage(rootURL: root)
    #expect(throws: (any Error).self) { _ = try SlopAttachments.read("../document.sqlite", in: root) }
    #expect(throws: (any Error).self) { _ = try SlopAttachments.put(Data(count: SlopAttachments.maximumBytes + 1), in: root) }
    try Data("corruption".utf8).write(to: root.appendingPathComponent("state/attachments/" + id))
    #expect(throws: (any Error).self) { _ = try SlopAttachments.read(id, in: root) }
  }
  @Test func unsafeEntriesAndSymlinkDirectoriesAreRejected() throws {
    let root = try fixture()
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: outside) }
    let store = try Storage(root: root)
    defer { store.close() }
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    let directory = root.appendingPathComponent("state/attachments")
    try FileManager.default.createSymbolicLink(at: directory, withDestinationURL: outside)
    #expect(throws: (any Error).self) { _ = try SlopAttachments.put(Data([1]), in: root) }
    #expect(throws: (any Error).self) { _ = try SlopPackage(rootURL: root) }
    #expect(try FileManager.default.contentsOfDirectory(atPath: outside.path).isEmpty)
    try FileManager.default.removeItem(at: directory)
    _ = try SlopAttachments.put(Data([1]), in: root)
    try Data([2]).write(to: directory.appendingPathComponent("unexpected"))
    #expect(throws: (any Error).self) { _ = try SlopAttachments.list(in: root) }
  }
  @Test func interruptedStagingIsNotExposedAndQuotaIsEnforced() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let store = try Storage(root: root)
    defer { store.close() }
    _ = try SlopAttachments.put(Data([1]), in: root)
    try Data([9]).write(to: root.appendingPathComponent("state/attachments/.pending"))
    #expect(try SlopAttachments.list(in: root).count == 1)
    _ = try SlopAttachments.put(Data([2]), in: root)
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("state/attachments/.pending").path))
    for index in 2..<SlopAttachments.maximumCount {
      _ = try SlopAttachments.put(Data("blob-\(index)".utf8), in: root)
    }
    #expect(throws: (any Error).self) { _ = try SlopAttachments.put(Data("one too many".utf8), in: root) }
    // Deduplication still succeeds at quota.
    _ = try SlopAttachments.put(Data([1]), in: root)
  }
  @Test @MainActor func liveSocketClosedCLIAndDuplicationPreserveLargeAttachments() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    let copy = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: copy) }
    let engine = try WasmSession(package: SlopPackage(rootURL: root))
    engine.load()
    try await engine.waitUntilReady()
    let data = Data(repeating: 37, count: 2 * 1024 * 1024)
    let response = try await DocumentCommand.run(method: "attachments.put", url: root, attachmentBytes: data)
    let ref = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    let id = try #require(ref["id"] as? String)
    let expectedID = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    #expect(id == expectedID)
    func expectListed() async throws {
      let response = try await DocumentCommand.run(method: "attachments.list", url: root)
      let files = try #require(try JSONSerialization.jsonObject(with: response) as? [[String: Any]])
      #expect(files.count == 1)
      #expect(files.first?["id"] as? String == expectedID)
      #expect(files.first?["byteLength"] as? Int == data.count)
    }
    try await expectListed()
    let read = try await DocumentCommand.run(method: "attachments.read", url: root, attachmentID: id)
    let payload = try #require(try JSONSerialization.jsonObject(with: read) as? [String: String])
    #expect(Data(base64Encoded: payload["bytes"]!) == data)
    try await engine.flush()
    try SlopDuplicator.duplicate(from: root, to: copy)
    #expect(try SlopAttachments.read(id, in: copy) == data)
    try await engine.close()
    // Headless attachment commands must not execute this authored application.
    try Data("throw new Error('authored code must not run');".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let reopened = try await DocumentCommand.run(method: "attachments.read", url: root, attachmentID: id)
    #expect(reopened == read)
    try await expectListed()
  }
}

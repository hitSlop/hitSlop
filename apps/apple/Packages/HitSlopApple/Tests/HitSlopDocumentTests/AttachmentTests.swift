import AppKit
import Foundation
import CryptoKit
import HitSlopCore
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopDocument

// The core owns attachment storage (crates/hitslop-core/tests/file.rs); these prove the
// page, the CLI and Duplicate reach it through the owner.
@Suite(.serialized) struct AttachmentTests {
  @Test @MainActor func liveSocketClosedCLIAndDuplicationPreserveLargeAttachments() async throws {
    _ = NSApplication.shared
    let root = try Fixtures.native()
    let copy = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: copy) }
    let engine = try await DocumentSession.open(url: root)
    engine.load()
    try await engine.waitUntilReady()
    let data = Data(repeating: 37, count: 2 * 1024 * 1024)
    let response = try await command("attachments.put", url: root, attachmentBytes: data)
    let ref = try #require(try JSONSerialization.jsonObject(with: response) as? [String: Any])
    let id = try #require(ref["id"] as? String)
    let expectedID = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    #expect(id == expectedID)
    func expectListed() async throws {
      let response = try await command("attachments.list", url: root)
      let files = try #require(try JSONSerialization.jsonObject(with: response) as? [[String: Any]])
      #expect(files.count == 1)
      #expect(files.first?["id"] as? String == expectedID)
      #expect(files.first?["byteLength"] as? Int == data.count)
    }
    try await expectListed()
    let read = try await command("attachments.read", url: root, attachmentID: id)
    let payload = try #require(try JSONSerialization.jsonObject(with: read) as? [String: String])
    #expect(Data(base64Encoded: payload["bytes"]!) == data)
    // A copy of the open document carries the attachment.
    try await engine.copy(to: copy)
    #expect(try await command("attachments.read", url: copy, attachmentID: id) == read)
    try await engine.close()
    let reopened = try await command("attachments.read", url: root, attachmentID: id)
    #expect(reopened == read)
    try await expectListed()
  }

  // Failure: after admission every owner error became "failed", so the CLI reported
  // refusals that were never applied as an unknown outcome.
  @Test @MainActor func missingAttachmentIsARefusalNotAnUnknownOutcome() async throws {
    let root = try Fixtures.native()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let request = try SocketRequest(json: [
      "method": "attachments.read", "documentPath": root.path,
      "attachmentID": String(repeating: "a", count: 64),
    ])
    let reply = try decodeReply(await owner.request(request))
    #expect(!reply.ok)
    #expect(reply.code == .rejected)
    try await owner.close()
  }
}

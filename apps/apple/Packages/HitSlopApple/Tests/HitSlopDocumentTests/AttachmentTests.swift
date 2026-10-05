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
    // Stored with the agent batch that references it, so closing keeps it.
    let id = try await attach(data, at: ["title"], url: root)
    let expectedID = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    #expect(id == expectedID)
    func expectListed() async throws {
      let response = try await commandState("attachments.list", url: root)
      let files = try #require(try JSONSerialization.jsonObject(with: response) as? [[String: Any]])
      #expect(files.count == 1)
      #expect(files.first?["id"] as? String == expectedID)
      #expect(files.first?["byteLength"] as? Int == data.count)
    }
    func bytes(_ document: URL) async throws -> Data? {
      let read = try await commandState("attachments.read", url: document, ["attachmentID": id])
      let payload = try JSONSerialization.jsonObject(with: read) as? [String: String]
      return payload?["bytes"].flatMap { Data(base64Encoded: $0) }
    }
    try await expectListed()
    #expect(try await bytes(root) == data)
    // A copy of the open document carries the attachment.
    let preview = try Fixtures.png(width: 64, height: 64) { NSColor.systemRed.setFill(); $0.fill() }
    try await engine.copy(to: copy, artwork: SlopRenderedArtwork(preview: preview, icon: nil))
    #expect(try await bytes(copy) == data)
    #expect(Fixtures.hasCustomIcon(copy), "a copy gets its own Finder icon")
    try await engine.close()
    #expect(try await bytes(root) == data)
    try await expectListed()
  }

  // Failure: after admission every owner error became "failed", so the CLI reported
  // refusals that were never applied as an unknown outcome.
  @Test @MainActor func missingAttachmentIsARefusalNotAnUnknownOutcome() async throws {
    let root = try Fixtures.native()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let request = try SocketRequest(json: [
      "protocol": HelperProtocol.version, "method": "attachments.read", "documentPath": root.path,
      "attachmentID": String(repeating: "a", count: 64),
    ])
    let reply = try decodeReply(await owner.request(request))
    #expect(!reply.ok)
    #expect(reply.code == .rejected)
    try await owner.close()
  }
}

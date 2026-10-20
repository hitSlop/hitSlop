import Foundation
import HitSlopCoreBinding
import HitSlopTestSupport
import Testing

@testable import HitSlopCore
@testable import HitSlopDocument

// A cold file-open can beat the asynchronous iCloud connection. A checkpoint must
// still merge through that live writer, retaining its local edit and writer lease.
@Test func aDocumentOpenedBeforeTheLibraryAcceptsCloudCheckpoints() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let entry = try library.create(template: template.path, title: "Cold open")
  let checkpoint = try library.stage(id: entry.id)
  defer { if let asset = checkpoint.assetPath { try? library.releaseUpload(path: asset) } }
  let owner = try DocumentOwner(url: URL(fileURLWithPath: entry.path))
  let owners = LibraryOwners()
  try owners.register(owner.native)
  _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
  try owners.activate(library)
  do {
    try library.receive(
      id: entry.id, title: entry.title, template: entry.template,
      asset: checkpoint.assetPath, version: checkpoint.entry.version,
      fields: Data(), deleted: false, artworkVersion: nil, assetRevision: nil)
    let state = try Fixtures.object(await owner.state())["value"] as? [String: Any]
    #expect(state?["hits"] as? Int == 3)
    let upload = try library.stage(id: entry.id)
    if let asset = upload.assetPath { try library.releaseUpload(path: asset) }
    try await owner.close()
  } catch {
    try? await owner.close()
    throw error
  }
}

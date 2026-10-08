// Failure: accepted Rust edits disappear after save/reopen, or a failed close
// releases ownership. Literal state and a competing OS lock are independent oracles.
import Foundation
import Testing
import NativeOwner
struct OwnerTests {
  func seeded() throws -> (URL, Store, RustOwner) {
    let f = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("fixtures/checklist.json")
    let fixture = try JSONValue.decode(Data(contentsOf: f))
    let schema = String(decoding: try fixture["schema"].encoded(), as: UTF8.self)
    let doc = try NativeDocument.create(schemaJson:schema, initialJson:String(decoding: try fixture["initial"].encoded(), as: UTF8.self))
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("rust-owner-"+UUID().uuidString)
    let store = try Store(root:root)
    _ = try store.write(generation:"0", checkpoint:doc.checkpoint(), schemaKey:schema)
    return (root,store,try RustOwner(store:store))
  }
  func edit(_ owner:RustOwner) throws {
    _ = try owner.apply("{\"intents\":[{\"type\":\"set\",\"path\":[\"rows\",{\"id\":\"00000000000000000000000000000001\"},\"done\"],\"value\":true}]}")
  }
  @Test func failedCloseRetainsLockAndAcceptedStateUntilRetry() throws {
    let (root,store,owner) = try seeded(); defer {store.close();try? FileManager.default.removeItem(at:root)}
    try edit(owner)
    store.beforeCommit = {throw SpikeError("injected disk error")}
    #expect(throws:Error.self){try owner.close()}
    #expect(owner.dirty)
    #expect(throws:Error.self){_ = try Store(root:root)}
    let before = try owner.core.snapshot()
    store.beforeCommit=nil;try owner.close()
    let reopenedStore = try Store(root:root);defer{reopenedStore.close()}
    let reopened = try RustOwner(store:reopenedStore)
    #expect(try JSONValue.decode(Data(reopened.core.snapshot().utf8))["value"] == JSONValue.decode(Data(before.utf8))["value"])
    #expect(try JSONValue.decode(Data(reopened.core.snapshot().utf8))["value"]["rows"].array[0]["done"] == .bool(true))
  }
  @Test func ambiguousCommittedSaveDoesNotAppendTwice() throws {
    let (root,store,owner) = try seeded();defer{store.close();try? FileManager.default.removeItem(at:root)}
    try edit(owner);store.afterCommit={throw SpikeError("reply lost after commit")}
    try owner.flush();let commits=store.commits
    #expect(!owner.dirty);try owner.flush();#expect(store.commits==commits)
    #expect(try store.metadata().updateRows==1)
  }
  @Test func unpublishedOwnerCanReopenWithoutWebKit() throws {
    let (root,store,owner) = try seeded();defer{store.close();try? FileManager.default.removeItem(at:root)}
    try edit(owner);try owner.flush()
    let another = try RustOwner(store:store)
    #expect(try JSONValue.decode(Data(another.core.snapshot().utf8))["value"]["rows"].array[0]["done"] == .bool(true))
  }
}

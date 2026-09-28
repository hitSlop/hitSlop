import Foundation
import HitSlopCore
import HitSlopCoreBinding
import Testing
@testable import HitSlopDocument

// Native gap: the shared Rust semantic tests cannot prove production SQLite
// durability, writer exclusion, or refusal before the package gains state.
@Suite(.serialized) struct DocumentOwnerTests {
  func fixture() throws -> URL {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/4-1/document", toPath: root.path)
    let spec = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: repository + "/crates/hitslop-core/fixtures/checklist.json"))) as! [String: Any]
    for (file, key) in [("state.schema.json", "schema"), ("initial.json", "initial")] {
      try JSONSerialization.data(withJSONObject: spec[key]!).write(to: root.appendingPathComponent(file))
    }
    // Native editing must never evaluate authored JavaScript.
    try Data("throw new Error('authored code must not execute');".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    return root
  }
  func value(_ owner: DocumentOwner) async throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(await owner.state().utf8)) as! [String: Any]
  }
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#

  @Test func nativeBindingExecutesLiteralFixturesAndReplaysUpdates() throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let f = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: repository + "/crates/hitslop-core/fixtures/checklist.json"))) as! [String: Any]
    func json(_ value: Any) throws -> String {
      String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed]), as: UTF8.self)
    }
    for scenario in f["scenarios"] as! [[String: Any]] {
      let schema = try json(f["schema"]!)
      let core = try NativeDocument.create(schemaJson: schema, initialJson: json(scenario["initial"] ?? f["initial"]!))
      let before = try core.snapshot(), seed = try core.checkpoint(), version = try core.version()
      let intents = (scenario["intents"] as! [[String: Any]]).map { value in
        var op = value
        if op["base"] as? String == "$current" { op["base"] = version }
        return op
      }
      let batch = try json(["intents": intents])
      if let expected = scenario["error"] as? String {
        do { _ = try core.apply(batchJson: batch); Issue.record("Accepted invalid fixture") }
        catch { #expect(String(describing: error).contains(expected)) }
        #expect(try core.snapshot() == before)
      } else {
        _ = try core.apply(batchJson: batch)
        let current = try JSONSerialization.jsonObject(with: Data(core.snapshot().utf8)) as! [String: Any]
        #expect(try json(current["value"]!) == json(scenario["after"]!))
        let reopened = try NativeDocument.open(schemaJson: schema, checkpoint: seed)
        _ = try reopened.importUpdates(bytes: core.exportSince(version: version))
        let replay = try JSONSerialization.jsonObject(with: Data(reopened.snapshot().utf8)) as! [String: Any]
        #expect(try json(replay["value"]!) == json(scenario["after"]!))
      }
    }
  }

  @Test func acceptsOnceAndReopensWithoutWebKitOrAuthoredCode() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect(throws: DocumentWriterLock.Busy.self) { _ = try DocumentWriterLock(root: root) }
    let reply = try await owner.apply(id: "one", session: owner.session, batch: increment)
    let duplicate = try await owner.apply(id: "one", session: owner.session, batch: increment)
    #expect(reply == duplicate)
    await #expect(throws: (any Error).self) {
      _ = try await owner.apply(id: "one", session: owner.session, batch: self.increment.replacingOccurrences(of: ":3", with: ":4"))
    }
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect(reopened.session != owner.session)
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    await #expect(throws: (any Error).self) {
      _ = try await reopened.apply(id: "two", session: owner.session, batch: self.increment)
    }
    try await reopened.close()
  }

  // Failure: after a committed write's reply is lost and the recovery read also fails,
  // the next save's generation conflict was mistaken for success and its edits dropped.
  // Oracle: every accepted increment survives close and reopen.
  @Test func lostReplyWithFailedRecoveryReadNeverDropsLaterEdits() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await owner.apply(id: "first", session: owner.session, batch: increment)
    var committed = false
    owner.storage.testingPhase = { phase in
      if phase == "append:committed" || phase == "checkpoint:committed" {
        committed = true
        throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost acknowledgement"])
      }
      if phase == "metadata" && committed {
        throw NSError(domain: "StorageFault", code: 2, userInfo: [NSLocalizedDescriptionKey: "Recovery read failed"])
      }
    }
    await #expect(throws: (any Error).self) { try await owner.flush() }
    owner.storage.testingPhase = nil
    _ = try await owner.apply(id: "second", session: owner.session, batch: increment)
    try await owner.flush()
    try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 6)
    try await reopened.close()
  }

  @Test func failedCloseRetainsLiveStateAndOwnershipUntilRetry() async throws {
    let root = try fixture()
    let moved = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: moved) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await owner.apply(id: "one", session: owner.session, batch: increment)
    // Real I/O boundary: the package temporarily becomes unavailable.
    try FileManager.default.moveItem(at: root, to: moved)
    await #expect(throws: (any Error).self) { try await owner.close() }
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 3)
    #expect(throws: DocumentWriterLock.Busy.self) { _ = try DocumentWriterLock(root: moved) }
    try FileManager.default.moveItem(at: moved, to: root)
    try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await reopened.close()
  }

  @Test func readOnlyOwnerAndOlderStorageReaderCannotWriteTheNewLayout() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    _ = try await owner.apply(id: "one", session: owner.session, batch: increment)
    try await owner.flush()
    let capture = try DocumentOwner(package: SlopPackage(rootURL: root), mode: .snapshot)
    #expect((try await value(capture)["value"] as? [String: Any])?["hits"] as? Int == 3)
    await #expect(throws: (any Error).self) {
      _ = try await capture.apply(id: "capture", session: capture.session, batch: self.increment)
    }
    try await capture.close()
    try await owner.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await reopened.close()
  }

  @Test func expiredRequestCannotApplyAgain() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    for i in 0..<257 {
      _ = try await owner.apply(id: String(i), session: owner.session, batch: increment)
    }
    await #expect(throws: (any Error).self) {
      _ = try await owner.apply(id: "0", session: owner.session, batch: self.increment)
    }
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 771)
    try await owner.close()
  }
  // Gap: direct binding tests cannot prove the production page/ctx bridge or live forwarding.
  // Oracle: the public promise exposes its accepted value, CLI publication reaches the page,
  // and a fresh native owner reads both edits after close without evaluating authored code.
  @Test @MainActor func publicSDKAndLiveCLIShareTheNativeOwner() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("""
      export default { mount(ctx, target) {
        globalThis.consumer = ctx.document;
        const input = document.createElement('textarea');
        target.append(input);
        const binding = ctx.bind.text(input, ctx.document.fields.title);
        return { unmount() { binding.destroy(); input.remove(); } };
      } };
      """.utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let session = try DocumentSession(package: SlopPackage(rootURL: root))
    session.load()
    try await session.waitUntilReady()
    let accepted = try await session.webView.callAsyncJavaScript("""
      const doc = globalThis.consumer;
      const before = doc.current.rows[0];
      await doc.at(before).done.set(true);
      return doc.current.rows[0].done;
      """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(accepted == true)
    _ = try await DocumentCommand.run(method: "apply", url: root,
      operation: Data(#"{"type":"increment","path":["hits"],"by":7}"#.utf8))
    let published = try await session.webView.callAsyncJavaScript("""
      const doc = globalThis.consumer;
      if (doc.current.hits === 7) return true;
      return await new Promise(resolve => {
        const stop = doc.subscribe(() => { if (doc.current.hits === 7) { stop(); resolve(true); } });
      });
      """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(published == true)
    _ = try await session.webView.callAsyncJavaScript("""
      const input = document.querySelector('textarea');
      input.value = 'Saved 😀 draft';
      input.dispatchEvent(new Event('input'));
      return true;
      """, arguments: [:], in: nil, contentWorld: .page)
    try await session.close()
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let current = try await value(owner)["value"] as! [String: Any]
    #expect(current["hits"] as? Int == 7)
    #expect(current["title"] as? String == "Saved 😀 draft")
    #expect((current["rows"] as? [[String: Any]])?.first?["done"] as? Bool == true)
    try await owner.close()
    let result = try await DocumentCommand.run(method: "apply", url: root,
      operation: Data(#"{"type":"increment","path":["hits"],"by":2}"#.utf8))
    #expect((try JSONSerialization.jsonObject(with: result) as? [String: Any])?["hits"] as? Int == 9)
  }

  @Test @MainActor func plainAndSvelteConsumersExerciseCtx() async throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    for source in ["tests/fixtures/4-1/document", "tests/fixtures/4-1-svelte/document", "generated/v1/abi/owner-svelte.slop"] {
      let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
      defer { try? FileManager.default.removeItem(at: root) }
      try FileManager.default.copyItem(atPath: repository + "/" + source, toPath: root.path)
      let session = try DocumentSession(package: SlopPackage(rootURL: root))
      session.load(); try await session.waitUntilReady()
      let passed = try await session.webView.callAsyncJavaScript("return await globalThis.contractTest()", arguments: [:], in: nil, contentWorld: .page) as? Bool
      #expect(passed == true)
      try await session.close()
    }
  }

  // Gap: storage blob tests and SDK barriers separately cannot prove the native
  // ABI persists an attachment and its accepted reference before immediate close.
  @Test @MainActor func attachmentReferenceAndBlobSurviveImmediateClose() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("""
      export default { mount(ctx) { globalThis.attachmentProbe = () => {
        globalThis.importWork = ctx.attachments.import(new File(['native attachment'], 'note.txt', {type:'text/plain'}), {
          commit: ref => ctx.document.fields.title.replace(ref.id)
        });
      }; return {}; } };
      """.utf8).write(to: root.appendingPathComponent("assets/app.js"))
    let session = try DocumentSession(package: SlopPackage(rootURL: root))
    session.load(); try await session.waitUntilReady()
    _ = try await session.webView.callAsyncJavaScript("attachmentProbe(); return true", arguments: [:], in: nil, contentWorld: .page)
    try await session.close()
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let current = try await value(owner)["value"] as! [String: Any]
    let id = try #require(current["title"] as? String)
    #expect(id.count == 64)
    #expect(try Data(contentsOf: root.appendingPathComponent("state/attachments/" + id)) == Data("native attachment".utf8))
    try await owner.close()
  }

}

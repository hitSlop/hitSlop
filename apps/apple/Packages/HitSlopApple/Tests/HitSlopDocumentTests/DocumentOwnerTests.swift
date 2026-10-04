import Foundation
import HitSlopCore
import SQLite3
import HitSlopCoreBinding
import Testing
import HitSlopTestSupport
@testable import HitSlopDocument

// Native gap: the Rust store tests own SQLite semantics; these prove the owner drives
// them through the binding without WebKit or authored code.
@Suite(.serialized) struct DocumentOwnerTests {
  /// The checklist document with `app` as its app. By default the app throws: native editing
  /// must never evaluate authored JavaScript.
  func fixture(app: String = "throw new Error('authored code must not execute');") throws -> URL {
    let stage = try Fixtures.stage()
    let spec = try JSONSerialization.jsonObject(with: Data(contentsOf: Fixtures.repository.appendingPathComponent("crates/hitslop-core/fixtures/checklist.json"))) as! [String: Any]
    try Fixtures.updateApp(stage) { app in
      app["descriptor"] = spec["schema"]
      app["initial"] = spec["initial"]
    }
    try Data(app.utf8).write(to: stage.appendingPathComponent("assets/app.js"))
    return try Fixtures.document(stage: stage)
  }
  func value(_ owner: DocumentOwner) async throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(await owner.state().utf8)) as! [String: Any]
  }
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#

  @Test(arguments: [Optional<String>.none, "another-core"])
  @MainActor func liveOwnerWithWrongCoreIdentityIsRefusedBeforeCommands(identity: String?) async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let forwarded = Locked(0)
    let server = try SocketServer { request, _ in
      if request.method == .hello { return SocketReply(ok: true, epoch: owner.epoch, coreBuildId: identity).encoded() }
      forwarded.modify { $0 += 1 }
      return await owner.request(request)
    }
    defer { server.stop() }
    try owner.publishDiscovery(JSONSerialization.data(withJSONObject: ["socket": server.path, "documentPath": root.path]))
    for method in ["get", "apply"] {
      do {
        _ = try await command(method, url: root,
          operation: method == "apply" ? Data(#"{"type":"increment","path":["hits"],"by":3}"#.utf8) : nil)
        Issue.record("Accepted owner without core identity")
      } catch {
        #expect(error.localizedDescription.contains("Quit and reopen hitSlop"))
      }
    }
    let output = root.deletingPathExtension().appendingPathExtension("png")
    do {
      try await DocumentCommand.exportLive(root: root, socket: server.path, format: .png, output: output)
      Issue.record("Accepted export from owner without matching core identity")
    } catch { #expect(error.localizedDescription.contains("Quit and reopen hitSlop")) }
    #expect(!FileManager.default.fileExists(atPath: output.path))
    #expect(forwarded.value == 0)
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 0)
    try await owner.close()
  }

  @Test func nativeBindingExecutesLiteralFixturesAndReplaysUpdates() throws {
    func json(_ value: Any) throws -> String {
      String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed]), as: UTF8.self)
    }
    // Every literal scenario file (checklist.json, scalars.json, …) runs natively too, in a
    // document whose app carries the scenario's descriptor and initial values.
    let directory = Fixtures.repository.appendingPathComponent("crates/hitslop-core/fixtures")
    let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
      .filter { $0.pathExtension == "json" }
    var ran = 0
    for file in files {
    let f = try JSONSerialization.jsonObject(with: Data(contentsOf: file)) as! [String: Any]
    guard let scenarios = f["scenarios"] as? [[String: Any]] else { continue }
    for scenario in scenarios {
      ran += 1
      let stage = try Fixtures.stage()
      try Fixtures.updateApp(stage) { app in
        app["descriptor"] = f["schema"]
        app["initial"] = scenario["initial"] ?? f["initial"]
      }
      let root = try Fixtures.document(stage: stage)
      defer { try? FileManager.default.removeItem(at: root) }
      let store = try NativeStore.open(path: root.path, mode: .document)
      let core = try store.document()
      let before = try core.state()
      let batch = try json(["intents": scenario["intents"]!])
      if let expected = scenario["error"] as? String {
        do { _ = try core.applyBatch(batchJson: batch, origin: .page); Issue.record("Accepted invalid fixture") }
        catch { #expect(String(describing: error).contains(expected)) }
        #expect(try core.state() == before)
        try store.close()
      } else {
        _ = try core.applyBatch(batchJson: batch, origin: .page)
        let current = try JSONSerialization.jsonObject(with: Data(core.state().utf8)) as! [String: Any]
        #expect(try json(current["value"]!) == json(scenario["after"]!))
        // The saved update replays to the same value; a batch that changed nothing saves nothing.
        if let job = try core.saveJob(store: store, forceCheckpoint: false) { try store.write(job: job) }
        try store.close()
        let reopened = try NativeStore.open(path: root.path, mode: .snapshot).document()
        let replay = try JSONSerialization.jsonObject(with: Data(reopened.state().utf8)) as! [String: Any]
        #expect(try json(replay["value"]!) == json(scenario["after"]!))
      }
    }
    }
    #expect(ran > 20)
  }

  /// An owner shows the document from its store's one check, in either mode: the app a
  /// separate open reports. A file's header gives its kind, and a template is refused as a
  /// document before anything is written.
  @Test func ownersShowTheAppTheirOpenCheckedAndRefuseTemplates() async throws {
    let stage = try Fixtures.minimalStage(theme: ##"{"paper":"#ffffff","accent":"#335577"}"##)
    defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
    let template = try Fixtures.template(stage: stage)
    let root = try Fixtures.document(from: template)
    defer { try? FileManager.default.removeItem(at: root) }
    let opened = try SlopFile(url: root)
    for mode in [StorageMode.document, .snapshot] {
      let owner = try DocumentOwner(url: root, mode: mode)
      #expect(owner.file.kind == .document)
      #expect(owner.file.descriptor == opened.descriptor)
      #expect(owner.file.themeTokens.map(\.name) == ["paper", "accent"])
      #expect(owner.file.manifest.slug == opened.manifest.slug)
      try await owner.close()
    }
    #expect(try SlopFile.kind(of: template) == .template)
    #expect(try SlopFile.kind(of: root) == .document)
    let before = try Data(contentsOf: template)
    do {
      _ = try DocumentOwner(url: template)
      Issue.record("A template opened as a document")
    } catch SlopError.template {}
    #expect(try Data(contentsOf: template) == before)
  }

  /// Only a file the core refuses reads as an invalid hitSlop file; a storage failure,
  /// such as a document that is no longer there, keeps its own message.
  @Test func storageFailuresAreNotReportedAsInvalidFiles() throws {
    let missing = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    do {
      _ = try DocumentOwner(url: missing)
      Issue.record("A missing document opened")
    } catch let error as SlopError {
      Issue.record("A storage failure was reported as an invalid file: \(error.localizedDescription)")
    } catch {}
  }

  @Test func savesAndReopensWithoutWebKitOrAuthoredCode() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    #expect(throws: DocumentLocked.self) { _ = try WriterLock.acquire(root) }
    _ = try await owner.apply(batch: increment)
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await owner.close()
    let reopened = try DocumentOwner(url: root)
    #expect(reopened.epoch != owner.epoch)
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    await #expect(throws: (any Error).self) {
      _ = try await reopened.apply(batch: self.increment, epoch: owner.epoch)
    }
    try await reopened.close()
  }

  // A document a newer hitSlop saved asks for an update from every owner mode, through
  // the binding, and its file is left exactly as that build wrote it.
  @Test func newerStorageAsksForAnUpdateAndIsLeftUnchanged() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    try await owner.close()
    var connection: OpaquePointer?
    #expect(sqlite3_open(root.path, &connection) == SQLITE_OK)
    #expect(sqlite3_exec(connection, "PRAGMA user_version=2", nil, nil, nil) == SQLITE_OK)
    sqlite3_close(connection)
    let before = try Data(contentsOf: root)
    for mode in [StorageMode.document, .snapshot] {
      #expect(throws: SlopRequiresUpdate.self) { _ = try DocumentOwner(url: root, mode: mode) }
    }
    #expect(try Data(contentsOf: root) == before)
  }

  // Failure: a lost commit acknowledgement must retain both the original edit and edits
  // accepted before the next retry. Oracle: every accepted increment survives reopen.
  @Test func lostReplyWithFailedRecoveryReadNeverDropsLaterEdits() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    owner.testingPhase = { phase in
      if phase == "append:committed" || phase == "checkpoint:committed" {
        throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Lost acknowledgement"])
      }
    }
    await #expect(throws: (any Error).self) { try await owner.flush() }
    await #expect(throws: (any Error).self) { try await owner.flush() }
    owner.testingPhase = nil
    _ = try await owner.apply(batch: increment)
    try await owner.flush()
    try await owner.close()
    let reopened = try DocumentOwner(url: root)
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 6)
    try await reopened.close()
  }

  @Test func failedCloseRetainsLiveStateAndOwnershipUntilRetry() async throws {
    let root = try fixture()
    let moved = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: moved) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    // Real I/O boundary: the file temporarily moves away.
    try FileManager.default.moveItem(at: root, to: moved)
    await #expect(throws: (any Error).self) { try await owner.close() }
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 3)
    #expect(throws: DocumentLocked.self) { _ = try WriterLock.acquire(moved) }
    try FileManager.default.moveItem(at: moved, to: root)
    try await owner.close()
    let reopened = try DocumentOwner(url: root)
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await reopened.close()
  }

  // A snapshot reads the theme with its document and can change neither the theme nor
  // the attachments.
  @Test func snapshotOwnerFreezesTheThemeAndOwnsNothing() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let first = try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#111111"}"##))
    try await owner.flush()
    let snapshot = try DocumentOwner(url: root, mode: .snapshot)
    _ = try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#333333"}"##))
    try await owner.flush()
    #expect(try await snapshot.loadTheme().state.effective == first.effective)
    await #expect(throws: (any Error).self) { _ = try await snapshot.applyTheme(.set(valuesJson: ##"{"accent":"#222222"}"##)) }
    await #expect(throws: (any Error).self) { _ = try await snapshot.putAttachment(base64: "AQ==") }
    try await snapshot.close()
    try await owner.close()
  }

  @Test func snapshotOwnerReadsSavedStateWithoutWriting() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    try await owner.flush()
    let capture = try DocumentOwner(url: root, mode: .snapshot)
    #expect((try await value(capture)["value"] as? [String: Any])?["hits"] as? Int == 3)
    await #expect(throws: (any Error).self) {
      _ = try await capture.apply(batch: self.increment)
    }
    try await capture.close()
    try await owner.close()
    let reopened = try DocumentOwner(url: root)
    #expect((try await value(reopened)["value"] as? [String: Any])?["hits"] as? Int == 3)
    try await reopened.close()
  }

  // Gap: direct binding tests cannot prove the production page/ctx bridge or live forwarding.
  // Oracle: the public promise exposes its accepted value, CLI publication reaches the page,
  // and a fresh native owner reads both edits after close without evaluating authored code.
  @Test @MainActor func publicSDKAndLiveCLIShareTheNativeOwner() async throws {
    let root = try fixture(app: """
      export default { mount(ctx, target) {
        globalThis.consumer = ctx.document;
        const input = document.createElement('textarea');
        target.append(input);
        const binding = ctx.bind.text(input, ctx.document.fields.title);
        return { unmount() { binding.destroy(); input.remove(); } };
      } };
      """)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let accepted = try await session.webView.callAsyncJavaScript("""
      const doc = globalThis.consumer;
      const before = doc.current.rows[0];
      await doc.at(before).done.set(true);
      return doc.current.rows[0].done;
      """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(accepted == true)
    _ = try await command("apply", url: root,
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
    let owner = try DocumentOwner(url: root)
    let current = try await value(owner)["value"] as! [String: Any]
    #expect(current["hits"] as? Int == 7)
    #expect(current["title"] as? String == "Saved 😀 draft")
    #expect((current["rows"] as? [[String: Any]])?.first?["done"] as? Bool == true)
    try await owner.close()
    let result = try await command("apply", url: root,
      operation: Data(#"{"type":"increment","path":["hits"],"by":2}"#.utf8))
    let reply = try #require(try JSONSerialization.jsonObject(with: result) as? [String: Any])
    #expect((reply["value"] as? [String: Any])?["hits"] as? Int == 9)
    // A minted row ID is reported, so a caller can address the row it just created.
    let inserted = try JSONSerialization.jsonObject(with: await command("apply", url: root,
      operation: Data(#"{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}"#.utf8))) as! [String: Any]
    let ids = try #require(inserted["ids"] as? [String])
    let rows = (inserted["value"] as? [String: Any])?["rows"] as? [[String: Any]] ?? []
    #expect(ids.count == 1 && rows.contains { $0["$id"] as? String == ids[0] })
  }

  @Test @MainActor func plainAndSvelteConsumersExerciseCtx() async throws {
    let svelte = Fixtures.repository.appendingPathComponent("generated/abi/owner-svelte.slop")
    for root in [try Fixtures.document(), try Fixtures.document(from: svelte)] {
      defer { try? FileManager.default.removeItem(at: root) }
      let session = try await DocumentSession.open(url: root)
      session.load(); try await session.waitUntilReady()
      let passed = try await session.webView.callAsyncJavaScript("return await globalThis.contractTest()", arguments: [:], in: nil, contentWorld: .page) as? Bool
      #expect(passed == true)
      try await session.close()
    }
  }

  // Gap: storage blob tests and SDK barriers separately cannot prove the native
  // ABI persists an attachment and its accepted reference before immediate close.
  @Test @MainActor func attachmentReferenceAndBlobSurviveImmediateClose() async throws {
    let root = try fixture(app: """
      export default { mount(ctx) { globalThis.attachmentProbe = () => {
        globalThis.importWork = ctx.attachments.import(new File(['native attachment'], 'note.txt', {type:'text/plain'}),
          (tx, ref) => tx.fields.title.set(ref.id));
      }; return {}; } };
      """)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load(); try await session.waitUntilReady()
    _ = try await session.webView.callAsyncJavaScript("attachmentProbe(); return true", arguments: [:], in: nil, contentWorld: .page)
    try await session.close()
    let owner = try DocumentOwner(url: root)
    let current = try await value(owner)["value"] as! [String: Any]
    let id = try #require(current["title"] as? String)
    #expect(id.count == 64)
    #expect(try await owner.readAttachment(id) == Data("native attachment".utf8).base64EncodedString())
    try await owner.close()
  }


  // Failure: the same `theme set` was accepted or refused depending on whether a window
  // was open. Oracle: the owner refuses each invalid value and keeps the saved theme.
  @Test func themeRulesHoldForEveryWriter() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let set = { (values: [String: String]) in
      ThemeChange.set(valuesJson: String(decoding: try JSONSerialization.data(withJSONObject: values), as: UTF8.self))
    }
    _ = try await owner.applyTheme(set(["accent": "#abcdef"]))
    for values in [
      ["missing": "#000000"], ["accent": " "], ["accent": "red"], ["accent": "#fff"], ["accent": "#ABCDEF"],
      ["accent": "#abcdefff"], ["accent": "var(--slop-accent)"], ["accent": "#000000;display:none"],
    ] {
      await #expect(throws: (any Error).self) { _ = try await owner.applyTheme(set(values)) }
    }
    await #expect(throws: (any Error).self) { _ = try await owner.applyTheme(.reset(token: "missing")) }
    #expect(try await accent(owner) == "#abcdef")
    try await owner.close()
  }

  func accent(_ owner: DocumentOwner) async throws -> String? {
    try JSONDecoder().decode([String: String].self, from: Data(try await owner.loadTheme().state.effective.utf8))["accent"]
  }
  /// The saved accent, read without the owner (snapshot mode takes no lock).
  func savedAccent(_ root: URL) async throws -> String? {
    let snapshot = try DocumentOwner(url: root, mode: .snapshot)
    defer { Task { try? await snapshot.close() } }
    return try await accent(snapshot)
  }

  // A theme change is an edit: accepted in memory, saved by the owner's jobs, waited for
  // by flush and close, and kept for a retry when its save fails.
  @Test func themeChangesAreSavedLikeEdits() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.testingPhase = { phase in
      if phase.hasPrefix("theme:") {
        throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Disk unavailable"])
      }
    }
    #expect(try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#111111"}"##)).changed)
    #expect(try await accent(owner) == "#111111")
    await #expect(throws: (any Error).self) { try await owner.flush() }
    #expect(try await savedAccent(root) == "#335577")
    owner.testingPhase = nil
    try await owner.flush()
    #expect(try await savedAccent(root) == "#111111")
    // Setting the template's color changes nothing; a later change is saved by close.
    #expect(try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#111111"}"##)).changed == false)
    _ = try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#222222"}"##))
    try await owner.close()
    #expect(try await savedAccent(root) == "#222222")
  }

  // Failure: export serialized the palette in memory, so it succeeded while the save
  // that should back it was pending or failing.
  @Test func themeExportWaitsForTheSave() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.testingPhase = { phase in
      if phase.hasPrefix("theme:") {
        throw NSError(domain: "StorageFault", code: 1, userInfo: [NSLocalizedDescriptionKey: "Disk unavailable"])
      }
    }
    _ = try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#444444"}"##))
    await #expect(throws: (any Error).self) { _ = try await owner.exportTheme() }
    owner.testingPhase = nil
    #expect(try await owner.exportTheme().contains("#444444"))
    #expect(try await savedAccent(root) == "#444444")
    try await owner.close()
  }

  @Test func themeImportReplacesOverridesForItsTemplateOnly() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.applyTheme(.set(valuesJson: ##"{"accent":"#111111"}"##))
    let file = try await owner.exportTheme()
    #expect(file.contains(#""template":"runtime-conformance""#))
    _ = try await owner.applyTheme(.reset(token: nil))
    let other = file.replacingOccurrences(of: "runtime-conformance", with: "habit-heatmap")
    await #expect(throws: (any Error).self) { _ = try await owner.applyTheme(owner.importTheme(other)) }
    await #expect(throws: (any Error).self) { _ = try await owner.applyTheme(owner.importTheme("not a theme")) }
    #expect(try await accent(owner) == "#335577")
    #expect(try await owner.applyTheme(owner.importTheme(file)).changed)
    #expect(try await accent(owner) == "#111111")
    try await owner.close()
    #expect(try await savedAccent(root) == "#111111")
  }

  // Spike S-D. Failure: work queued by a replaced page, or captured before a discard,
  // applied to state it never saw. Oracle: `owner_replaced` and an unchanged document.
  @Test func requestsFromAReplacedViewOrEpochAreRefused() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.attach(view: "first")
    _ = try await owner.open(view: "first")
    owner.attach(view: "second")
    _ = try await owner.open(view: "second")
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.open(view: "first") }
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.apply(batch: self.increment, view: "first") }
    _ = try await owner.apply(batch: increment, view: "second")
    let epoch = owner.epoch
    try await owner.discardPending()
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.apply(batch: self.increment, epoch: epoch) }
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.apply(batch: self.increment, view: "second") }
    #expect((try await value(owner)["value"] as? [String: Any])?["hits"] as? Int == 0)
    try await owner.close()
  }
}

extension DocumentOwnerTests {
  @Test @MainActor func failedDeliveryResyncsTheLivePageWithoutReplayingItsEdit() async throws {
    let root = try fixture(app: "export default {mount(ctx) {globalThis.doc = ctx.document; return {}}}")
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load(); try await session.waitUntilReady()
    var injected = false
    session.testingDeliveryFailure = {
      if !injected { injected = true; throw failure("injected JavaScript delivery failure") }
    }
    let hits = try await session.webView.callAsyncJavaScript("""
      await Promise.race([doc.fields.hits.increment(4),
        new Promise((_, reject) => setTimeout(() => reject(Error('publication did not recover')), 3000))]);
      return doc.current.hits;
      """, arguments: [:], in: nil, contentWorld: .page) as? Int
    #expect(injected)
    #expect(hits == 4)
    #expect((try await value(session.owner)["value"] as? [String: Any])?["hits"] as? Int == 4)
    try await session.close()
  }

  @Test @MainActor func pageAdmissionOrdersMixedEditsAndFencesOldOpenAndFlush() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.attach(view: "first")
    let initial = try await value(owner)
    let title = (initial["value"] as! [String: Any])["title"] as! String
    let base = initial["version"] as! String
    let sequences: [Int] = await withCheckedContinuation { continuation in
      var values: [Int] = []
      let receive: @MainActor @Sendable ([String: Any]) -> Void = { reply in
        values.append(reply["sequence"] as? Int ?? -1)
        if values.count == 2 { continuation.resume(returning: values) }
      }
      admitPage(owner, ["method": "apply",
        "batch": json(["intents": [["type": "increment", "path": ["hits"], "by": 1]]])], reply: receive)
      admitPage(owner, ["method": "text",
        "request": json(["base": base, "path": ["title"], "from": title, "to": title + "!",
          "selectionStart": title.utf16.count + 1, "selectionEnd": title.utf16.count + 1])], reply: receive)
    }
    #expect(sequences == [1, 2])
    owner.attach(view: "second")
    for method in ["open", "flush"] {
      let code: String? = await withCheckedContinuation { continuation in
        admitPage(owner, ["method": method]) {
          continuation.resume(returning: $0["code"] as? String)
        }
      }
      #expect(code == "owner_replaced")
    }
    try await owner.close()
  }

  // Swift checks only the envelope; the core parses the payload. Either way a malformed
  // or oversized request is a definite refusal that applies nothing.
  @Test @MainActor func malformedPageRequestsAreRefusedNotUncertain() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.attach(view: "page")
    let increment = json(["intents": [["type": "increment", "path": ["hits"], "by": 1]]])
    let requests: [([String: Any], String)] = [
      (["method": "apply"], "invalid_request"),
      (["method": "apply", "batch": increment, "extra": true], "invalid_request"),
      (["method": "flush", "batch": increment], "invalid_request"),
      (["view": "", "method": "apply", "batch": increment], "invalid_request"),
      (["method": "apply", "batch": json(["intents": [["type": "increment", "path": ["hits"], "by": 1, "extra": 1]]])], "invalid_request"),
      (["method": "text", "request": json(["base": "x"])], "invalid_request"),
      (["method": "apply", "batch": ["intents": []]], "invalid_request"),
      (["method": "apply", "batch": json(["intents": [["type": "set", "path": ["title"], "value": String(repeating: "x", count: 4 * 1024 * 1024)]]])], "too_large"),
      // UTF-8 bytes, rather than characters, bound opaque document payloads.
      (["method": "text", "request": String(repeating: "😀", count: 1_048_577)], "too_large"),
    ]
    for (request, reason) in requests {
      let reply: [String?] = await withCheckedContinuation { continuation in
        admitPage(owner, request, view: "page") { continuation.resume(returning: [$0["code"] as? String, $0["reason"] as? String]) }
      }
      #expect(reply == ["rejected", reason])
    }
    #expect(try await value(owner)["sequence"] as? Int == 0)
    try await owner.close()
  }
}

/// A page payload, as the page sends it: JSON text.
private func json(_ value: Any) -> String {
  String(decoding: try! JSONSerialization.data(withJSONObject: value), as: UTF8.self)
}

// Exercise the same validated envelope and owner admission used by DocumentSession.
@MainActor private func admitPage(_ owner: DocumentOwner, _ args: [String: Any], view: String = "first",
  reply: @escaping @MainActor @Sendable ([String: Any]) -> Void
) {
  do { owner.admitPage(try PageRequest(args), view: view, reply: reply) }
  catch { reply(DocumentOwner.pageFailure(error)) }
}

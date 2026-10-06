import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument

// Native gap: the Rust store tests own SQLite semantics; these prove the owner drives
// them through the binding without WebKit or authored code.
@Suite(.serialized) struct DocumentOwnerTests {
  /// The checklist document with `app` as its app. By default the app throws: native editing
  /// must never evaluate authored JavaScript.
  func fixture(app: String = "throw new Error('authored code must not execute');") throws -> URL {
    try Fixtures.checklistDocument(app: app)
  }
  func value(_ owner: DocumentOwner) async throws -> [String: Any] {
    try Fixtures.object(await owner.state())
  }
  func hits(_ owner: DocumentOwner) async throws -> Int? {
    (try await value(owner)["value"] as? [String: Any])?["hits"] as? Int
  }
  let increment = #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#

  // Failure: the helper validated a live reply, and the validator refuses JSON over 48 MiB,
  // while a closed reply went unchecked, so a large document read closed but not live.
  // Oracle: near-limit batches each apply once, and the live and closed reads of a reply
  // over 48 MiB agree.
  @Test @MainActor func largeDocumentsReadTheSameLiveAndClosed() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let server = try owner.startServer(
      exporter: NativeExports { _, _, _, _ in throw OwnerError.rejected("No renderer") })
    defer { server.stop() }
    // U+0001 is stored as one byte and written to JSON as six (`\u0001`): the reply outgrows
    // 48 MiB while the document stays far inside its 32 MiB.
    let text = String(repeating: "\\u0001", count: 140_000)
    let batches = 62
    for _ in 0..<batches {
      let ops =
        #"[{"type":"insert","path":["rows"],"value":{"text":""# + text
        + #"","done":false}},{"type":"increment","path":["hits"],"by":1}]"#
      #expect(try await command("batch", url: root, ["ops": ops]).ok)
    }
    let live = try await commandState("get", url: root)
    #expect(live.count > 48 * 1024 * 1024)
    try await owner.close()
    let closed = try await commandState("get", url: root)
    func value(_ state: Data) throws -> NSDictionary? {
      (try JSONSerialization.jsonObject(with: state) as? [String: Any])?["value"] as? NSDictionary
    }
    #expect(try value(live) == value(closed))
    #expect(try value(closed)?["hits"] as? Int == batches, "each batch applied once")
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
    for mode in [StoreMode.document, .snapshot] {
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
    #expect(Fixtures.isLocked(root))
    _ = try await owner.apply(batch: increment)
    #expect(try await hits(owner) == 3)
    try await owner.close()
    let reopened = try DocumentOwner(url: root)
    #expect(try await hits(reopened) == 3)
    try await reopened.close()
  }

  // An owner that fails to open releases the file: its store and the asset reader opened
  // with it close before the error reaches the caller.
  @Test func anOwnerThatFailsToOpenReleasesTheLock() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    try await owner.close()
    try Fixtures.sql(root, "UPDATE checkpoint SET bytes = x'00'")
    #expect(throws: (any Error).self) { _ = try DocumentOwner(url: root) }
    #expect(!Fixtures.isLocked(root))
  }

  @Test func failedCloseRetainsLiveStateAndOwnershipUntilRetry() async throws {
    let root = try fixture()
    let moved = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".slop")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: moved)
    }
    let owner = try DocumentOwner(url: root)
    _ = try await owner.apply(batch: increment)
    // Real I/O boundary: the file temporarily moves away.
    try FileManager.default.moveItem(at: root, to: moved)
    await #expect(throws: (any Error).self) { try await owner.close() }
    #expect(try await hits(owner) == 3)
    #expect(Fixtures.isLocked(moved))
    try FileManager.default.moveItem(at: moved, to: root)
    _ = try await owner.apply(batch: increment)
    try await owner.close()
    #expect(!Fixtures.isLocked(root))
    let reopened = try DocumentOwner(url: root)
    #expect(try await hits(reopened) == 6)
    try await reopened.close()
  }

  // Gap: direct binding tests cannot prove the production page/ctx bridge or live forwarding.
  // Oracle: the public promise exposes its accepted value, CLI publication reaches the page,
  // and a fresh native owner reads both edits after close without evaluating authored code.
  @Test @MainActor func publicSDKAndLiveCLIShareTheNativeOwner() async throws {
    let root = try fixture(
      app: """
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
    let accepted =
      try await session.webView.callAsyncJavaScript(
        """
        const doc = globalThis.consumer;
        const before = doc.current.rows[0];
        await doc.at(before).done.set(true);
        return doc.current.rows[0].done;
        """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(accepted == true)
    #expect(try await command("batch", url: root, ["ops": #"[{"type":"increment","path":["hits"],"by":7}]"#]).ok)
    let published =
      try await session.webView.callAsyncJavaScript(
        """
        const doc = globalThis.consumer;
        if (doc.current.hits === 7) return true;
        return await new Promise(resolve => {
          const stop = doc.subscribe(() => { if (doc.current.hits === 7) { stop(); resolve(true); } });
        });
        """, arguments: [:], in: nil, contentWorld: .page) as? Bool
    #expect(published == true)
    _ = try await session.webView.callAsyncJavaScript(
      """
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
    // A closed document: the command's own owner applies and saves it. A minted row ID is
    // reported, so a caller can address the row it just created.
    let inserted = try await command(
      "batch", url: root,
      [
        "ops":
          #"[{"type":"increment","path":["hits"],"by":2},{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}]"#
      ])
    let ids = try #require(inserted.ids)
    let saved = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as! [String: Any]
    let state = saved["value"] as? [String: Any] ?? [:]
    #expect(state["hits"] as? Int == 9)
    #expect(ids.count == 1 && (state["rows"] as? [[String: Any]] ?? []).contains { $0["$id"] as? String == ids[0] })
  }

  @Test @MainActor func plainAndSvelteConsumersExerciseCtx() async throws {
    let svelte = Fixtures.repository.appendingPathComponent("generated/abi/owner-svelte.slop")
    for root in [try Fixtures.document(), try Fixtures.document(from: svelte)] {
      defer { try? FileManager.default.removeItem(at: root) }
      let session = try await DocumentSession.open(url: root)
      session.load()
      try await session.waitUntilReady()
      let passed =
        try await session.webView.callAsyncJavaScript(
          "return await globalThis.contractTest()", arguments: [:], in: nil, contentWorld: .page) as? Bool
      #expect(passed == true)
      try await session.close()
    }
  }

  // Gap: storage blob tests and SDK barriers separately cannot prove the native
  // ABI persists an attachment and its accepted reference before immediate close.
  @Test @MainActor func attachmentReferenceAndBlobSurviveImmediateClose() async throws {
    let root = try fixture(
      app: """
        export default { mount(ctx) { globalThis.attachmentProbe = () => {
          globalThis.importWork = ctx.attachments.import(new File(['native attachment'], 'note.txt', {type:'text/plain'}),
            (tx, ref) => tx.fields.title.set(ref.id));
        }; return {}; } };
        """)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    _ = try await session.webView.callAsyncJavaScript(
      "attachmentProbe(); return true", arguments: [:], in: nil, contentWorld: .page)
    try await session.close()
    let owner = try DocumentOwner(url: root)
    let current = try await value(owner)["value"] as! [String: Any]
    try await owner.close()
    let id = try #require(current["title"] as? String)
    #expect(id.count == 64)
    let read = try await command("attachments.read", url: root, ["attachmentID": id])
    #expect((read.state as? [String: Any])?["bytes"] as? String == Data("native attachment".utf8).base64EncodedString())
  }

  /// A theme panel change; the sequence it was accepted at.
  @discardableResult
  func theme(_ owner: DocumentOwner, _ change: SlopThemeChange) async throws -> Int {
    try await withCheckedThrowingContinuation { done in owner.enqueueTheme(change) { done.resume(with: $0) } }
  }
  func accent(_ owner: DocumentOwner) async throws -> String? {
    try JSONDecoder().decode([String: String].self, from: Data(try await owner.loadTheme().state.effective.utf8))[
      "accent"]
  }
  /// The saved accent, read without the owner (snapshot mode takes no lock).
  func savedAccent(_ root: URL) async throws -> String? {
    let snapshot = try DocumentOwner(url: root, mode: .snapshot)
    defer { Task { try? await snapshot.close() } }
    return try await accent(snapshot)
  }

  @Test func themeImportReplacesOverridesForItsTemplateOnly() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    try await theme(owner, .set(["accent": "#111111"]))
    let file = try await owner.exportTheme()
    #expect(file.contains(#""template":"runtime-conformance""#))
    #expect(file.hasSuffix("\n"))
    try await theme(owner, .resetAll)
    let other = file.replacingOccurrences(of: "runtime-conformance", with: "habit-heatmap")
    await #expect(throws: (any Error).self) { try await self.theme(owner, .importFile(other)) }
    await #expect(throws: (any Error).self) { try await self.theme(owner, .importFile("not a theme")) }
    #expect(try await accent(owner) == "#335577")
    try await theme(owner, .importFile(file))
    #expect(try await accent(owner) == "#111111")
    try await owner.close()
    #expect(try await savedAccent(root) == "#111111")
  }

  @Test func aThemeDragIsOneDocumentUndoStep() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    try await theme(owner, .set(["accent": "#111111"]))
    try await theme(owner, .set(["accent": "#222222"]))
    _ = try await owner.apply(batch: increment)
    _ = try await owner.undo()
    #expect(try await hits(owner) == 0)
    #expect(try await accent(owner) == "#222222")
    _ = try await owner.undo()
    #expect(try await accent(owner) == "#335577")
    _ = try await owner.undo(redo: true)
    #expect(try await accent(owner) == "#222222")
    try await owner.close()
    #expect(try await savedAccent(root) == "#222222")
  }

  // Spike S-D. Failure: work queued by a replaced page applied to state it never saw.
  // Oracle: `owner_replaced` and an unchanged document.
  @Test func requestsFromAReplacedViewAreRefused() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    owner.attach(view: "first")
    owner.attach(view: "second")
    #expect(await page(owner, #"{"method":"open"}"#, view: "second") == nil)
    #expect(await page(owner, #"{"method":"open"}"#, view: "first")?.kind == .replaced)
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.apply(batch: self.increment, view: "first") }
    _ = try await owner.apply(batch: increment, view: "second")
    try await owner.discardPending()
    owner.attach(view: "third")
    await #expect(throws: OwnerReplaced.self) { _ = try await owner.apply(batch: self.increment, view: "second") }
    #expect(await page(owner, #"{"method":"attachments.put","bytes":"AQ=="}"#, view: "second")?.kind == .replaced)
    #expect(try await hits(owner) == 0)
    try await owner.close()
  }
  /// A page request from `view`; the owner's failure, if it refused.
  func page(_ owner: DocumentOwner, _ json: String, view: String) async -> OwnerFailure? {
    await withCheckedContinuation { done in
      owner.page(json: json, view: view) { _, failure in done.resume(returning: failure) }
    }
  }
}

extension DocumentOwnerTests {
  // A delivery the page refuses (its publish call throws once) resyncs the page from the
  // owner; the edit is never replayed.
  @Test @MainActor func failedDeliveryResyncsTheLivePageWithoutReplayingItsEdit() async throws {
    let root = try fixture(app: "export default {mount(ctx) {globalThis.doc = ctx.document; return {}}}")
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let hits =
      try await session.webView.callAsyncJavaScript(
        """
        const publish = globalThis.__slop.publish;
        globalThis.refused = false;
        globalThis.__slop.publish = () => { globalThis.__slop.publish = publish; globalThis.refused = true; throw Error('refused'); };
        await Promise.race([doc.fields.hits.increment(4),
          new Promise((_, reject) => setTimeout(() => reject(Error('publication did not recover')), 3000))]);
        return globalThis.refused ? doc.current.hits : -1;
        """, arguments: [:], in: nil, contentWorld: .page) as? Int
    #expect(hits == 4)
    #expect(try await self.hits(session.owner) == 4)
    try await session.close()
  }
}

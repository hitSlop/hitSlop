import Foundation
import PopCore
import PopCoreBinding
import PopTestSupport
import Testing

@testable import PopDocument

// Native integration: file failures map to host errors, and the public SDK shares its
// owner with CLI edits and persisted attachments.
@Suite(.serialized) struct DocumentOwnerTests {
  /// The checklist document with `app` as its app. By default the app throws: native editing
  /// must never evaluate authored JavaScript.
  func fixture(app: String = "throw new Error('authored code must not execute');") throws -> URL {
    try Fixtures.checklistDocument(app: app)
  }
  func value(_ owner: DocumentOwner, isolation: isolated (any Actor)? = #isolation) async throws -> [String: Any] {
    try Fixtures.object(await owner.state())
  }
  func hits(_ owner: DocumentOwner) async throws -> Int? {
    (try await value(owner)["value"] as? [String: Any])?["hits"] as? Int
  }

  // Failure: a live reply was validated, and the validator refuses JSON over 48 MiB,
  // while a closed reply went unchecked, so a large document read closed but not live.
  // Oracle: near-limit batches each apply once, and the live and closed reads of a reply
  // over 48 MiB agree.
  @Test(.nightly) @MainActor func largeDocumentsReadTheSameLiveAndClosed() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    let server = try owner.startServer(
      page: PageBarrier(settle: { _ = $0.complete(failure: .rejected("No page")) }, resume: {}))
    defer { server.stop() }
    // U+0001 is stored as one byte and written to JSON as six (`\u0001`): the reply outgrows
    // 48 MiB while the document stays far inside its 32 MiB.
    let text = String(repeating: "\\u0001", count: 140_000)
    let batches = 62
    for _ in 0..<batches {
      let ops =
        #"[{"type":"insert","path":["rows"],"value":{"text":""# + text
        + #"","done":false}},{"type":"increment","path":["hits"],"by":1}]"#
      #expect(
        try await command(
          "batch", url: root, ["batch": ["intents": try JSONSerialization.jsonObject(with: Data((ops).utf8))]]
        ).ok)
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
    let stage = try Fixtures.minimalStage(theme: [("paper", "#ffffff"), ("accent", "#335577")])
    defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
    let template = try Fixtures.template(stage: stage)
    let root = try Fixtures.document(from: template)
    defer { try? FileManager.default.removeItem(at: root) }
    let opened = try PopFile(url: root)
    for mode in [StoreMode.document, .snapshot] {
      let owner = try DocumentOwner(url: root, mode: mode)
      #expect(owner.file.kind == .document)
      #expect(owner.file.descriptor == opened.descriptor)
      #expect(owner.file.themeTokens.map(\.name) == ["paper", "accent"])
      #expect(owner.file.metadata.slug == opened.metadata.slug)
      try await owner.close()
    }
    #expect(try PopFile.kind(of: template) == .template)
    #expect(try PopFile.kind(of: root) == .document)
    let before = try Data(contentsOf: template)
    do {
      _ = try DocumentOwner(url: template)
      Issue.record("A template opened as a document")
    } catch PopError.template {}
    #expect(try Data(contentsOf: template) == before)
  }

  /// A busy writer is a storage failure, not a refused file.
  @Test func storageFailuresAreNotReportedAsInvalidFiles() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let owner = try DocumentOwner(url: root)
    do {
      _ = try DocumentOwner(url: root)
      Issue.record("A second writer opened")
    } catch let error as PopError {
      Issue.record("A storage failure was reported as an invalid file: \(error.localizedDescription)")
    } catch let failure as OwnerFailure {
      #expect(failure.kind == .locked)
    }
    try await owner.close()
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
    #expect(
      try await command(
        "batch", url: root,
        [
          "batch": [
            "intents": try JSONSerialization.jsonObject(
              with: Data((#"[{"type":"increment","path":["hits"],"by":7}]"#).utf8))
          ]
        ]
      ).ok)
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
        "batch": [
          "intents": try JSONSerialization.jsonObject(
            with: Data(
              (#"[{"type":"increment","path":["hits"],"by":2},{"type":"insert","path":["rows"],"value":{"text":"new","done":false}}]"#)
                .utf8))
        ]
      ])
    let ids = try #require(inserted.ids)
    let saved = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as! [String: Any]
    let state = saved["value"] as? [String: Any] ?? [:]
    #expect(state["hits"] as? Int == 9)
    #expect(ids.count == 1 && (state["rows"] as? [[String: Any]] ?? []).contains { $0["$id"] as? String == ids[0] })
  }

  @Test @MainActor func plainAndSvelteConsumersExerciseCtx() async throws {
    let svelte = Fixtures.repository.appendingPathComponent("generated/abi/owner-svelte.pop")
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
        const publish = globalThis.__pop.publish;
        globalThis.refused = false;
        globalThis.__pop.publish = () => { globalThis.__pop.publish = publish; globalThis.refused = true; throw Error('refused'); };
        await Promise.race([doc.fields.hits.increment(4),
          new Promise((_, reject) => setTimeout(() => reject(Error('publication did not recover')), 3000))]);
        return globalThis.refused ? doc.current.hits : -1;
        """, arguments: [:], in: nil, contentWorld: .page) as? Int
    #expect(hits == 4)
    #expect(try await self.hits(session.owner) == 4)
    try await session.close()
  }
}

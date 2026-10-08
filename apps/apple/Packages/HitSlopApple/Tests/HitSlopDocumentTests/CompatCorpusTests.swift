import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing
import WebKit

@testable import HitSlopDocument

/// The compatibility corpus (tests/compat): every document a release saved must open in
/// this build, run its own old app's edits in a real page, save, close and reopen to the
/// result that release recorded. `HITSLOP_COMPAT_RECORD=<entry>` records that entry's
/// results instead (`bun run compat:capture`); a frozen entry is never recorded again.
@Suite(.serialized) struct CompatCorpusTests {
  static let corpus = URL(
    fileURLWithPath: ProcessInfo.processInfo.environment["HITSLOP_COMPAT_ROOT"]
      ?? Fixtures.repository.appendingPathComponent("tests/compat").path)
  static let recording = ProcessInfo.processInfo.environment["HITSLOP_COMPAT_RECORD"]

  /// Every page scenario: `<entry>/pages/<document>.json`.
  static func cases() -> [String] {
    let entries = (try? FileManager.default.contentsOfDirectory(atPath: corpus.path)) ?? []
    return entries.sorted().filter { recording == nil || $0 == recording }.flatMap { entry in
      ((try? FileManager.default.contentsOfDirectory(atPath: corpus.appendingPathComponent("\(entry)/pages").path))
        ?? [])
        .filter { $0.hasSuffix(".json") }.sorted().map { "\(entry)/\(($0 as NSString).deletingPathExtension)" }
    }
  }

  @Test func theCorpusHasPageScenarios() {
    #expect(!Self.cases().isEmpty, "tests/compat has no page scenarios to replay")
  }

  @Test(arguments: cases()) @MainActor func oldAppsEditTheirSavedDocuments(_ name: String) async throws {
    let parts = name.split(separator: "/").map(String.init)
    let entry = Self.corpus.appendingPathComponent(parts[0])
    let pageURL = entry.appendingPathComponent("pages/\(parts[1]).json")
    var page = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: pageURL)) as? [String: Any])
    let release = try #require(
      try JSONSerialization.jsonObject(with: Data(contentsOf: entry.appendingPathComponent("release.json")))
        as? [String: Any])
    if Self.recording != nil && release["frozen"] as? Bool == true {
      throw SlopFailure("Cannot record a frozen compatibility entry")
    }
    if Self.recording == nil { #expect(!(page["value"] is NSNull), "No recorded result for \(name)") }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(at: entry.appendingPathComponent("documents/\(parts[1]).slop"), to: root)
    defer { try? FileManager.default.removeItem(at: root) }
    let before = try await value(root)

    let session = try await DocumentSession.open(url: root)
    session.webView.configuration.userContentController.addUserScript(
      WKUserScript(
        source: Self.pinned(clock: release["clock"] as? Double ?? 0), injectionTime: .atDocumentStart,
        forMainFrameOnly: true))
    session.load()
    do {
      try await session.waitUntilReady()
      switch page["script"] as? String {
      case "contractTest":
        let passed =
          try await session.webView.callAsyncJavaScript(
            "return await globalThis.contractTest()", arguments: [:], in: nil, contentWorld: .page) as? Bool
        #expect(passed == true, "\(name): the old app's contract test failed")
        // An agent's edit to the open document reaches the old app's page.
        _ = try await command(
          "batch", url: root,
          [
            "batch": [
              "intents": try JSONSerialization.jsonObject(
                with: Data((#"[{"type":"set","path":["title"],"value":"Live ✓"}]"#).utf8))
            ]
          ])
        let shown = try await eventually(timeout: .seconds(2)) {
          try await session.webView.callAsyncJavaScript(
            "return document.body.textContent.includes('Live ✓')", arguments: [:], in: nil, contentWorld: .page)
            as? Bool == true
        }
        #expect(shown, "\(name): a live CLI edit did not reach the page")
      case "actions":
        let actions = try #require(page["actions"] as? [[String: Any]])
        #expect(!actions.isEmpty, "No actions for \(name)")
        _ = try await session.webView.callAsyncJavaScript(
          Self.actions, arguments: ["actions": actions], in: nil, contentWorld: .page)
      default: throw SlopFailure("Unknown page scenario for \(name)")
      }
      _ = try await session.webView.callAsyncJavaScript(
        "await globalThis.__slop.flush(); return true", arguments: [:], in: nil, contentWorld: .page)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }

    let saved = Self.normalized(try await value(root), keeping: Self.ids(in: before))
    #expect(Self.canonical(saved) != Self.canonical(before), "\(name): the page scenario made no saved edit")
    if Self.recording != nil {
      page["value"] = saved
      try JSONSerialization.data(withJSONObject: page, options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes])
        .write(to: pageURL)
      // The bytes the page path saved (text splices against older versions, page-minted
      // IDs), kept beside the CLI-written documents so later builds replay them too.
      let pageSaved = entry.appendingPathComponent("documents/\(parts[1]).page.slop")
      try? FileManager.default.removeItem(at: pageSaved)
      try FileManager.default.copyItem(at: root, to: pageSaved)
    } else {
      #expect(
        Self.canonical(saved) == Self.canonical(page["value"] as Any),
        "\(name): the saved result differs from its release's")
    }
  }

  @MainActor private func value(_ root: URL) async throws -> Any {
    let owner = try DocumentOwner(url: root)
    let state = try Fixtures.object(await owner.state())
    try await owner.close()
    return state["value"] as Any
  }

  /// A clock and randomness that start where the release's did, so apps that read today's
  /// date or draw random values save the same document.
  static func pinned(clock: Double) -> String {
    """
    (() => {
      const RealDate = Date, start = RealDate.now(), now = () => \(clock) + (RealDate.now() - start);
      function PinnedDate(...args) {
        if (!new.target) return new RealDate(now()).toString();
        return args.length ? new RealDate(...args) : new RealDate(now());
      }
      PinnedDate.prototype = RealDate.prototype;
      PinnedDate.now = now; PinnedDate.UTC = RealDate.UTC; PinnedDate.parse = RealDate.parse;
      globalThis.Date = PinnedDate;
      let seed = 0x2545f491;
      const next = () => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return (seed >>> 0) / 4294967296; };
      Math.random = next;
      crypto.getRandomValues = (array) => {
        const bytes = new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
        for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(next() * 256);
        return array;
      };
      crypto.randomUUID = () => [8, 4, 4, 4, 12].map((n) => Array.from({ length: n }, () => Math.floor(next() * 16).toString(16)).join("")).join("-");
    })();
    """
  }

  /// Frozen, explicit public UI actions. Missing controls fail instead of recording no-op edits.
  static let actions = """
    for (const action of actions) {
      const matches = document.querySelectorAll(action.selector);
      if (matches.length !== 1) throw new Error(`Expected one control: ${action.selector}`);
      const field = matches[0];
      if (field.disabled || field.readOnly || !field.getClientRects().length) throw new Error(`Unavailable control: ${action.selector}`);
      if (action.click) { field.click(); await globalThis.__slop.flush(); continue; }
      field.focus();
      field.value = action.value;
      field.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: action.value }));
      field.dispatchEvent(new Event("change", { bubbles: true }));
      if (action.enter) {
        field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
        field.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", bubbles: true }));
      }
      field.blur();
      await globalThis.__slop.flush();
      if (field.value !== action.value) throw new Error(`Control did not retain the edit: ${action.selector}`);
    }
    return true;
    """

  static func ids(in value: Any) -> Set<String> {
    switch value {
    case let object as [String: Any]:
      return object.reduce(into: Set((object["$id"] as? String).map { [$0] } ?? [])) { $0.formUnion(ids(in: $1.value)) }
    case let array as [Any]: return array.reduce(into: Set()) { $0.formUnion(ids(in: $1)) }
    default: return []
    }
  }
  /// Row IDs a page mints are random, and a later shell may mint them differently; they
  /// compare as `minted-N` in document order. IDs the saved document had compare exactly.
  static func normalized(_ value: Any, keeping known: Set<String>) -> Any {
    var minted: [String: String] = [:]
    func walk(_ value: Any) -> Any {
      switch value {
      case let object as [String: Any]:
        var result: [String: Any] = [:]
        for key in object.keys.sorted() {
          if key == "$id", let id = object[key] as? String, !known.contains(id) {
            result[key] =
              minted[id]
              ?? {
                minted[id] = "minted-\(minted.count + 1)"
                return minted[id]!
              }()
          } else {
            result[key] = walk(object[key]!)
          }
        }
        return result
      case let array as [Any]: return array.map(walk)
      default: return value
      }
    }
    return walk(value)
  }
  static func canonical(_ value: Any) -> Data {
    (try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])) ?? Data()
  }
}

import Foundation
import HitSlopCore
import SQLite3
import Testing
import WebKit

@testable import HitSlopDocument

@Suite(.serialized) struct DocumentSessionTests {
  // Per-field schema bounds do not bound an array's combined payload. The native
  // dispatch boundary must reject it before base64 decoding or touching SQLite.
  @Test func bridgeBoundsAggregatePayloadAndRejectsUnknownFields() {
    #expect(StorageRequest(["method": "attachments.list"]) != nil)
    #expect(StorageRequest(["method": "attachments.list", "extra": "unexpected"]) == nil)
    let values = Dictionary(uniqueKeysWithValues: (0..<16).map { ("token\($0)", String(repeating: "A", count: 4096)) })
    #expect(StorageRequest(["method": "theme.save", "values": values]) != nil)
    let oversized = Dictionary(uniqueKeysWithValues: (0..<20_000).map { ("token\($0)", String(repeating: "A", count: 4096)) })
    #expect(StorageRequest(["method": "theme.save", "values": oversized]) == nil)
  }

  // Lost-reply recovery depends on this: only a committed write records its attempt.
  @Test func writeAttemptIsRecordedOnlyWithCommittedBytes() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let storage = try Storage(root: root)
    let bytes = Data([1, 2, 3])
    _ = try storage.write(.checkpoint(bytes, schemaKey: "key"), generation: 0, attempt: "first")
    #expect(try storage.metadata().lastAttempt == "first")
    #expect(throws: (any Error).self) { try storage.write(.append(bytes), generation: 7, attempt: "refused") }
    #expect(try storage.metadata().lastAttempt == "first")
    _ = try storage.write(.append(bytes), generation: 1, attempt: "second")
    #expect(try storage.metadata().lastAttempt == "second")
    storage.close()
  }

  @Test @MainActor func socketRejectsMalformedEnvelopesBeforeDispatch() async throws {
    var handled = false
    let server = try SocketServer { _, _, reply in
      handled = true
      reply(.init(ok: true, epoch: "test"))
    }
    defer { server.stop() }
    let path = server.path
    for payload in [
      "not json", "{}",
      #"{"id":"x","method":"export","documentPath":"/tmp/a.slop","format":"pdf","output":"/tmp/a.pdf"}"#,
    ] {
      let data = try await Task.detached {
        try SocketClient.call(path: path, request: Data(payload.utf8))
      }.value
      let reply = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
      #expect(reply["ok"] as? Bool == false)
      #expect(reply["error"] as? String == "Invalid socket request")
    }
    #expect(!handled)
    await #expect(throws: (any Error).self) {
      _ = try await Task.detached {
        try SocketClient.call(path: path, request: Data(repeating: 65, count: 1_048_577))
      }.value
    }
    #expect(!handled)
  }
  @Test @MainActor func socketBoundsConcurrentClientsWithoutBlockingMainActor() async throws {
    var replies: [@MainActor @Sendable (SocketReply) -> Void] = []
    let server = try SocketServer { _, _, reply in replies.append(reply) }
    defer { server.stop() }
    let path = server.path
    let request = Data(#"{"id":"x","method":"get","documentPath":"/tmp/a.slop"}"#.utf8)
    // The synchronous client must not occupy Swift's cooperative executor.
    func call() async throws -> Data {
      try await withCheckedThrowingContinuation { continuation in
        DispatchQueue.global().async {
          continuation.resume(with: Result { try SocketClient.call(path: path, request: request) })
        }
      }
    }
    let tasks = (0..<16).map { _ in Task { try await call() } }
    for _ in 0..<200 where replies.count < 16 { try await Task.sleep(for: .milliseconds(10)) }
    #expect(replies.count == 16)
    await #expect(throws: (any Error).self) {
      _ = try await call()
    }
    for reply in replies { reply(.init(ok: true, state: [:])) }
    for task in tasks { _ = try await task.value }
  }

  @Test func writerExclusion() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let storage = try Storage(root: root)
    defer { storage.close() }
    #expect(throws: (any Error).self) { try Storage(root: root) }
  }
  @Test func snapshotStorageReadsSavedDocumentWithoutTouchingPackage() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let writer = try Storage(root: root)
    let saved = try writer.write(.append(Data([1, 2, 3])), generation: 0, attempt: UUID().uuidString)
    try writer.saveTheme(["accent": "#111111"])
    writer.close()
    let database = root.appendingPathComponent("state/document.sqlite")
    let theme = root.appendingPathComponent("state/theme.json")
    let before = try [Data(contentsOf: database), Data(contentsOf: theme)]
    let entries = try FileManager.default.contentsOfDirectory(atPath: root.appendingPathComponent("state").path).sorted()

    let snapshot = try Storage(root: root, mode: .snapshot)
    // Snapshots take no ownership: the document can still be opened for writing.
    let owner = try Storage(root: root)
    owner.close()
    let loaded = try snapshot.load()
    #expect(loaded.generation == saved)
    // The document identity is minted with the database and read unchanged by snapshots.
    #expect(loaded.docID.count == 32)
    #expect(loaded.updates == [Data([1, 2, 3])])
    // Renderer writes succeed in memory and are discarded.
    _ = try snapshot.write(.append(Data([4])), generation: loaded.generation, attempt: UUID().uuidString)
    try snapshot.saveTheme(["accent": "#222222"])
    #expect(throws: (any Error).self) { try snapshot.putAttachment(Data([9])) }
    snapshot.close()

    #expect(try [Data(contentsOf: database), Data(contentsOf: theme)] == before)
    #expect(try FileManager.default.contentsOfDirectory(atPath: root.appendingPathComponent("state").path).sorted() == entries)
  }
  @Test func snapshotStorageFreezesThemeWithDocument() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let writer = try Storage(root: root)
    try writer.saveTheme(["accent": "#111111"])
    let snapshot = try Storage(root: root, mode: .snapshot)
    defer { snapshot.close() }
    try writer.saveTheme(["accent": "#333333"])
    writer.close()
    #expect(try snapshot.loadTheme() == ["accent": "#111111"])
    try snapshot.saveTheme(["accent": "#222222"])
    #expect(try snapshot.loadTheme() == ["accent": "#222222"])
    let saved = try Data(contentsOf: root.appendingPathComponent("state/theme.json"))
    #expect(try JSONSerialization.jsonObject(with: saved) as? [String: String] == ["accent": "#333333"])
  }
  @Test func snapshotStorageCopiesLiveRowsOnly() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let writer = try Storage(root: root)
    let appended = try writer.write(.append(Data(repeating: 7, count: 8 * 1024 * 1024)), generation: 0, attempt: UUID().uuidString)
    // A checkpoint deletes the log; the file keeps those pages free.
    let checkpoint = Data([1, 2, 3])
    let saved = try writer.write(.checkpoint(checkpoint, schemaKey: "key"), generation: appended, attempt: UUID().uuidString)
    writer.close()
    let size = try FileManager.default.attributesOfItem(
      atPath: root.appendingPathComponent("state/document.sqlite").path)[.size] as? Int ?? 0
    #expect(size > 8 * 1024 * 1024)
    let snapshot = try Storage(root: root, mode: .snapshot)
    defer { snapshot.close() }
    let loaded = try snapshot.load()
    #expect(loaded.checkpoint == checkpoint)
    #expect(loaded.schemaKey == "key")
    #expect(loaded.generation == saved)
    #expect(loaded.updates.isEmpty)
  }
  @Test func snapshotStorageCreatesNoStateForMasters() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let snapshot = try Storage(root: root, mode: .snapshot)
    let loaded = try snapshot.load()
    #expect(loaded.updates.isEmpty)
    _ = try snapshot.write(.append(Data([1])), generation: loaded.generation, attempt: UUID().uuidString)
    snapshot.close()
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("state").path))
  }
  @Test(arguments: ["rows", "bytes"])
  func storageRefusesOversizedLogWithoutMaterializingIt(limit: String) throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".slop")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let storage = try Storage(root: root)
    storage.close()
    var db: OpaquePointer?
    #expect(
      sqlite3_open(root.appendingPathComponent("state/document.sqlite").path, &db) == SQLITE_OK)
    let sql =
      limit == "rows"
      ? "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<4097) INSERT INTO updates(bytes) SELECT x'01' FROM n"
      : "INSERT INTO updates(bytes) VALUES(zeroblob(17 * 1024 * 1024)),(zeroblob(17 * 1024 * 1024))"
    #expect(sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK)
    sqlite3_close_v2(db)
    // Snapshots validate limits before copying anything into memory.
    #expect(throws: (any Error).self) { try Storage(root: root, mode: .snapshot) }
    let reopened = try Storage(root: root)
    defer { reopened.close() }
    #expect(throws: (any Error).self) { try reopened.load() }
  }

  @Test @MainActor func schemeRejectsSymlinkReplacedAfterOpen() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(
      at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let asset = root.appendingPathComponent("assets/test.js")
    try Data("safe".utf8).write(to: asset)
    let handler = SchemeHandler(root: root, shell: root)
    try FileManager.default.removeItem(at: asset)
    try FileManager.default.createSymbolicLink(
      at: asset, withDestinationURL: root.appendingPathComponent("initial.json"))
    try Data("secret".utf8).write(to: root.appendingPathComponent("initial.json"))
    let task = SchemeTask(URL(string: "slop://app/assets/test.js")!)
    handler.webView(WKWebView(), start: task)
    #expect(task.error != nil)
    #expect(task.data.isEmpty)
  }

  // Decoding and normalization must never turn an authored asset into a private resource.
  @Test @MainActor func schemeRejectsEncodedTraversalAndPreservesAllowedResources() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    for directory in ["assets", "state", "shell/loro"] {
      try FileManager.default.createDirectory(at: root.appendingPathComponent(directory), withIntermediateDirectories: true)
    }
    for path in ["assets/test.js", "shell/loro/test.js", "shell/secret.js", "state/host.lock", "state/document.sqlite"] {
      try Data(path.utf8).write(to: root.appendingPathComponent(path))
    }
    let shell = root.appendingPathComponent("shell")
    let handler = SchemeHandler(root: root, shell: shell)
    let view = WKWebView()
    for path in [
      "assets%2F..%2Fstate%2Fhost.lock", "assets/..%2Fstate/document.sqlite",
      "assets/%2e%2e/state/host.lock", "assets//test.js", "assets/%2e/test.js",
      "__shell__/loro%2F..%2Fsecret.js", "__shell__/loro//test.js",
    ] {
      let task = SchemeTask(URL(string: "slop://app/" + path)!)
      handler.webView(view, start: task)
      #expect(task.error != nil, "Accepted unsafe resource: \(path)")
      #expect(task.data.isEmpty)
    }
    for (path, expected) in [("assets/test.js?v=1", "assets/test.js"), ("__shell__/loro/test.js", "shell/loro/test.js")] {
      let task = SchemeTask(URL(string: "slop://app/" + path)!)
      handler.webView(view, start: task)
      #expect(task.error == nil)
      #expect(task.data == Data(expected.utf8))
    }

  }

  // Manifest sizing must govern actual bridge requests, not just native window chrome.
  @Test(arguments: [false, true]) @MainActor
  func resizeBridgeHonorsManifest(resizable: Bool) async throws {
    let repository = String(#filePath.components(separatedBy: "/apps/apple/")[0])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/4-1/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    let manifestURL = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: manifestURL)) as? [String: Any])
    manifest["presentation"] = ["width": 480, "height": 480, "resizable": resizable]
    try JSONSerialization.data(withJSONObject: manifest).write(to: manifestURL)
    let session = try DocumentSession(package: SlopPackage(rootURL: root))
    var resized = false
    session.onResize = { size in resized = true; return size }
    session.load()
    do {
      try await session.waitUntilReady()
      let accepted = try await session.webView.callAsyncJavaScript("""
        try {
          const size = await webkit.messageHandlers.hitslop.postMessage({method:'window.resize',width:600,height:500});
          return size.width === 600 && size.height === 500;
        } catch { return false; }
        """, arguments: [:], in: nil, contentWorld: .page)
      #expect(accepted as? Bool == resizable)
      #expect(resized == resizable)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }

}

private final class SchemeTask: NSObject, WKURLSchemeTask {
  let request: URLRequest
  var data = Data()
  var error: Error?
  init(_ url: URL) { request = URLRequest(url: url) }
  func didReceive(_ response: URLResponse) {}
  func didReceive(_ data: Data) { self.data.append(data) }
  func didFinish() {}
  func didFailWithError(_ error: Error) { self.error = error }
}

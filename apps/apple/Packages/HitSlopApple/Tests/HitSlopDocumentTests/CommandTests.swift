import Foundation
import HitSlopCore
import HitSlopTestSupport
import Testing

@testable import HitSlopDocument

// Agent commands reach the live native owner and publish one undoable edit to its page.
@Test @MainActor func cliCommandsPublishToTheNativeWindow() async throws {
  let root = try Fixtures.native()
  defer { try? FileManager.default.removeItem(at: root) }
  let session = try await DocumentSession.open(url: root)
  session.load()
  do {
    try await session.waitUntilReady()
    let before = try await commandState("get", url: root)
    let invalidArguments: [[String: Any]] = [["text": 3], ["text": "   "]]
    for args in invalidArguments {
      let refused = try await command("call", url: root, ["command": "addRow", "args": args])
      #expect(!refused.ok)
      #expect(try await commandState("get", url: root) == before)
    }
    let cli = try await command("call", url: root, ["command": "addRow", "args": ["text": "CLI command"]])
    #expect(cli.ok && cli.ids?.count == 1)
    func containsCommand() async throws -> Bool {
      try await session.webView.evaluateJavaScript(
        "[...document.querySelectorAll('[role=textbox]')].some(e => e.textContent === 'CLI command')") as? Bool == true
    }
    #expect(try await eventually(timeout: .seconds(5)) { try await containsCommand() })
    try await session.undo()
    #expect(try await eventually(timeout: .seconds(5)) { try await !containsCommand() })
    try await session.undo(redo: true)
    #expect(try await eventually(timeout: .seconds(5)) { try await containsCommand() })
    try await session.close()
    let value = try JSONSerialization.jsonObject(with: await commandState("get", url: root)) as! [String: Any]
    let tasks = (value["value"] as! [String: Any])["tasks"] as! [[String: Any]]
    #expect(tasks.last?["text"] as? String == "CLI command")
  } catch {
    try? await session.close()
    throw error
  }
}

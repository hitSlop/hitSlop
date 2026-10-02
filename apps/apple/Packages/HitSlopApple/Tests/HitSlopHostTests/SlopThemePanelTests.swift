import AppKit
import Foundation
import HitSlopCore
import HitSlopDocument
import Testing
@testable import HitSlopHost

@Test func themeColorsUseThePalettesOneSpelling() throws {
  for (typed, color) in [
    ("#ABC", "#aabbcc"), ("abc123", "#abc123"), (" #AbC12380 ", "#abc12380"), ("#abcd", "#aabbccdd"),
    ("#aabbccff", "#aabbcc"), ("#0a0c10d9", "#0a0c10d9"),
  ] { #expect(SlopThemeColor.normalized(typed) == color, "\(typed)") }
  for typed in ["", "#", "red", "#abcde", "#abcdefg", "rgba(1, 2, 3, 0.5)", "#12345"] {
    #expect(SlopThemeColor.normalized(typed) == nil, "\(typed)")
  }
  for color in ["#000000", "#ffffff", "#335577", "#c4dc332e", "#00000000"] {
    #expect(SlopThemeColor.hex(try #require(SlopThemeColor.color(color))) == color)
  }
  // A picker's color in another space arrives in sRGB.
  let p3 = try #require(CGColor(colorSpace: CGColorSpace(name: CGColorSpace.displayP3)!, components: [1, 0, 0, 1]))
  #expect(SlopThemeColor.hex(p3)?.count == 7)
  #expect(SlopThemeColor.label("graphiteDeep") == "Graphite Deep")
  #expect(SlopThemeColor.label("accent-ink") == "Accent Ink")
  #expect(SlopThemeColor.label("surface") == "Surface")
}

@Test func themePanelSitsBesideTheDocumentOnScreen() {
  let screen = NSRect(x: 0, y: 0, width: 1440, height: 900)
  let document = NSRect(x: 200, y: 300, width: 400, height: 500)
  let right = slopThemePanelFrame(document: document, visible: screen)
  #expect(right.minX == document.maxX + 8 && right.maxY == document.maxY && right.height == 500)
  // At the screen's right edge it moves to the left of the document.
  let atEdge = NSRect(x: 1100, y: 300, width: 300, height: 500)
  let left = slopThemePanelFrame(document: atEdge, visible: screen)
  #expect(left.maxX == atEdge.minX - 8 && !left.intersects(atEdge))
  // The panel is exactly as tall as the document, and a tall one never leaves the screen.
  #expect(slopThemePanelFrame(document: NSRect(x: 200, y: 600, width: 240, height: 180), visible: screen).height == 180)
  let tall = slopThemePanelFrame(document: NSRect(x: 200, y: -100, width: 400, height: 1200), visible: screen)
  #expect(screen.contains(tall))
  // A document filling the screen leaves the panel on screen, over its edge.
  #expect(screen.contains(slopThemePanelFrame(document: screen, visible: screen)))
}

@Test @MainActor func themePanelOpensBesideTheWindowAndRecolorsThePage() async throws {
  let root = try themeWindowFixture()
  defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
  let controller = try await SlopDocumentWindowController.open(packageURL: root)
  controller.showWindow(nil)
  await controller.waitForPresentation()
  let window = try #require(controller.window)
  #expect(controller.session.canEditTheme)

  _ = try await controller.perform(.theme(true))
  let panel = try #require(controller.themePanel)
  #expect(controller.isThemeShown)
  #expect(panel.parent === window, "the panel moves and minimizes with its document")
  #expect(controller.owns(panel), "menu commands reach the document while the panel is key")
  #expect(!panel.frame.intersects(window.frame))
  // The panel keeps the slop's height and scrolls its colors; its content never sizes it.
  try await Task.sleep(for: .milliseconds(200))
  #expect(panel.frame.height == window.frame.height && panel.frame.maxY == window.frame.maxY,
    "panel \(panel.frame) beside window \(window.frame)")
  let editor = try #require(controller.themeEditor)
  #expect(Array(editor.rows.map(\.id).prefix(2)) == ["paper", "accent"], "authored order")

  let accent = try #require(editor.rows.first { $0.id == "accent" })
  editor.set(accent, "#123456")
  try await controller.session.flush()
  let painted = try await controller.session.webView.callAsyncJavaScript(
    "return document.documentElement.style.getPropertyValue('--slop-accent')", arguments: [:], in: nil,
    contentWorld: .page) as? String
  #expect(painted == "#123456")
  #expect(editor.isChanged(accent) && editor.hasChanges)

  _ = try await controller.perform(.theme(false))
  #expect(controller.themePanel == nil && window.childWindows?.contains(panel) != true)
  _ = try await controller.perform(.theme(true))
  #expect(controller.isThemeShown)
  try await controller.closeDocument()
  #expect(controller.themePanel == nil)
}

private func themeWindowFixture() throws -> URL {
  let parent = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-theme-\(UUID().uuidString)", isDirectory: true)
  let root = parent.appendingPathComponent("theme.slop", isDirectory: true)
  try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
  try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
  try Data(#"{"kind":"object","properties":{}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
  try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
  // More colors than fit beside the window, so the list must scroll.
  let extra = (0..<30).map { ",\"color\($0)\":\"#000000\"" }.joined()
  try Data((##"{"paper":"#ffffff","accent":"#335577""## + extra + "}").utf8).write(to: root.appendingPathComponent("assets/theme.json"))
  let manifest = #"{"$schema":"https://api.hitslop.com/schemas/manifest.schema.json","author":{"name":"Fixture Author"},"slug":"theme-fixture","title":"Theme Fixture","description":"Tests the theme panel.","categories":["utilities"],"presentation":{"width":320,"height":240}}"#
  try Data(manifest.utf8).write(to: root.appendingPathComponent("manifest.json"))
  let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
  try FileManager.default.createDirectory(at: skill.deletingLastPathComponent(), withIntermediateDirectories: true)
  try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .appendingPathComponent("../../../../../../packages/cli/skills/hitslop-document/SKILL.md").standardizedFileURL).write(to: skill)
  return root
}

// Failure: drafts retired only when a delivered color matched them, so a change that
// overtook a local edit before delivery left the panel showing the superseded color.
@Test @MainActor func panelDraftsRetireByTheOwnersRevision() {
  var replies: [@MainActor (Result<Int, Error>) -> Void] = []
  let model = SlopThemeEditorModel(tokens: [(name: "accent", value: "#335577")]) { _, reply in replies.append(reply) }
  let accent = model.rows[0]
  model.apply(SlopThemeState(overrides: [:], effective: ["accent": "#335577"], revision: 0))
  // Accepted at revision 1; the CLI's change at revision 2 is delivered before the reply.
  model.set(accent, "#111111")
  #expect(model.value(accent) == "#111111", "a change shows while it is pending")
  model.apply(SlopThemeState(overrides: ["accent": "#222222"], effective: ["accent": "#222222"], revision: 2))
  replies.removeFirst()(.success(1))
  #expect(model.value(accent) == "#222222")
  // A delivery read before the change was accepted keeps the change showing.
  model.set(accent, "#333333")
  replies.removeFirst()(.success(3))
  model.apply(SlopThemeState(overrides: ["accent": "#222222"], effective: ["accent": "#222222"], revision: 2))
  #expect(model.value(accent) == "#333333")
  model.apply(SlopThemeState(overrides: ["accent": "#333333"], effective: ["accent": "#333333"], revision: 3))
  #expect(model.value(accent) == "#333333")
  // The CLI set the same color first, so the panel's change is a no-op with no delivery of
  // its own; its acceptance retires it, and a later change shows.
  model.set(accent, "#555555")
  model.apply(SlopThemeState(overrides: ["accent": "#555555"], effective: ["accent": "#555555"], revision: 4))
  replies.removeFirst()(.success(4))
  model.apply(SlopThemeState(overrides: ["accent": "#666666"], effective: ["accent": "#666666"], revision: 5))
  #expect(model.value(accent) == "#666666")
}

// Failure: a focused hex field ignored incoming colors, then committed on blur, so tabbing
// out of an untouched field wrote the color it had shown back over a CLI change.
@Test @MainActor func anUntouchedHexFieldAdoptsTheAcceptedColor() {
  var sent: [SlopThemeChange] = []
  let model = SlopThemeEditorModel(tokens: [(name: "accent", value: "#335577")]) { change, _ in sent.append(change) }
  let accent = model.rows[0]
  model.apply(SlopThemeState(overrides: ["accent": "#444444"], effective: ["accent": "#444444"], revision: 1))
  #expect(model.finishTyping(accent, text: "#335577", edited: false) == "#444444")
  #expect(sent.isEmpty)
  #expect(model.finishTyping(accent, text: "ABC", edited: true) == "#aabbcc")
  #expect(sent == [.set(["accent": "#aabbcc"])])
}

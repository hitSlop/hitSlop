import AppKit
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument
import SwiftUI

/// The theme panel's frame: beside the document, on the right unless the screen ends
/// first, exactly as tall as the document (within the visible screen); its colors scroll.
func slopThemePanelFrame(document: NSRect, visible: NSRect?) -> NSRect {
  let width: CGFloat = 320, gap: CGFloat = 8
  var height = document.height
  if let visible { height = min(height, visible.height) }
  var frame = NSRect(x: document.maxX + gap, y: document.maxY - height, width: width, height: height)
  guard let visible else { return frame }
  if frame.maxX > visible.maxX {
    let left = document.minX - gap - width
    frame.origin.x = left >= visible.minX ? left : visible.maxX - width
  }
  frame.origin.y = min(max(frame.minY, visible.minY), visible.maxY - height)
  return frame
}

/// The theme panel: a color for each color the template declares. It is a child of the
/// document window, so it moves, hides and minimizes with it, and it sits outside the
/// slop's own content and shape. A change is an edit: the owner accepts it, the page
/// restyles, and it saves with the document.
extension SlopDocumentWindowController {
  public var isThemeShown: Bool { themePanel != nil }

  func setThemeShown(_ shown: Bool) {
    guard shown, let window, isContentReady, session.canEditTheme else { return closeThemePanel() }
    let editor = themeEditor ?? SlopThemeEditorModel(
      tokens: session.package.themeTokens.map { (name: $0.name, value: $0.value) },
      send: { [weak self] change, reply in self?.session.changeTheme(change, reply: reply) })
    themeEditor = editor
    if themePanel == nil {
      themePanel = makeThemePanel(editor)
      telemetry.send(.themeEditorOpened)
    }
    layoutThemePanel()
    if let panel = themePanel, panel.parent == nil { window.addChildWindow(panel, ordered: .above) }
    toolbarHost?.rootView = toolbarView()
    Task { [weak self, session] in
      if let theme = try? await session.currentTheme() { self?.themeEditor?.apply(theme) }
    }
  }

  func layoutThemePanel() {
    guard let window, let panel = themePanel else { return }
    panel.level = window.level
    panel.setFrame(slopThemePanelFrame(document: window.frame, visible: window.screen?.visibleFrame), display: true)
  }

  func closeThemePanel() {
    guard let panel = themePanel else { return }
    themePanel = nil
    themeEditor = nil
    // A color picker left open must never write into a document whose panel is gone.
    panel.contentView = nil
    if NSColorPanel.sharedColorPanelExists { NSColorPanel.shared.orderOut(nil) }
    panel.parent?.removeChildWindow(panel)
    panel.close()
    toolbarHost?.rootView = toolbarView()
  }

  public func pageSession(_ session: DocumentSession, themeChanged theme: SlopThemeState) {
    themeEditor?.apply(theme)
  }

  private func makeThemePanel(_ editor: SlopThemeEditorModel) -> SlopThemePanel {
    let panel = SlopThemePanel(
      contentRect: NSRect(x: 0, y: 0, width: 320, height: 320),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.hidesOnDeactivate = false
    panel.isReleasedWhenClosed = false
    panel.isExcludedFromWindowsMenu = true
    panel.becomesKeyOnlyIfNeeded = true
    panel.title = "Theme"
    let content = NSHostingView(rootView: SlopThemeEditor(
      model: editor, title: session.package.manifest.title,
      close: { [weak self] in self?.request(.theme(false)) },
      importTheme: { [weak self] in self?.request(.importTheme) },
      exportTheme: { [weak self] in self?.request(.exportTheme) }))
    // The document sets the panel's size; a long palette scrolls instead of growing it.
    content.sizingOptions = []
    panel.contentView = content
    return panel
  }

  // MARK: Theme files

  func exportTheme() async throws {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.json]
    panel.nameFieldStringValue = packageURL.deletingPathExtension().lastPathComponent + " Theme.json"
    guard let output = await runSheet(panel) else {
      telemetry.send(.breadcrumb(.themeExport, .cancelled))
      return
    }
    telemetry.send(.breadcrumb(.themeExport, .started))
    do {
      try await session.exportTheme().write(to: output, options: .atomic)
      telemetry.send(.breadcrumb(.themeExport, .completed))
      telemetry.send(.themeExported)
    } catch { telemetry.failure(.themeExport, error: error); throw error }
  }

  func importTheme() async throws {
    let panel = NSOpenPanel()
    panel.allowedContentTypes = [.json]
    panel.allowsMultipleSelection = false
    panel.canChooseDirectories = false
    guard let file = await runSheet(panel) else {
      telemetry.send(.breadcrumb(.themeImport, .cancelled))
      return
    }
    telemetry.send(.breadcrumb(.themeImport, .started))
    do {
      let text = try Self.readThemeFile(file)
      if try await !session.currentTheme().overrides.isEmpty, await !confirmReplacingTheme() {
        telemetry.send(.breadcrumb(.themeImport, .cancelled))
        return
      }
      try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, Error>) in
        session.changeTheme(.importFile(text)) { done.resume(with: $0.map { _ in }) }
      }
      telemetry.send(.breadcrumb(.themeImport, .completed))
      telemetry.send(.themeImported)
    } catch { telemetry.failure(.themeImport, error: error); throw error }
  }

  /// A theme file's text, bounded; the core decides whether it is a theme for this document.
  static func readThemeFile(_ url: URL) throws -> String {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    let bytes = try handle.read(upToCount: Limits.themeFile + 1) ?? Data()
    guard bytes.count <= Limits.themeFile, let text = String(data: bytes, encoding: .utf8) else {
      throw SlopPackageError.invalid("Not a hitSlop theme file")
    }
    return text
  }

  private func confirmReplacingTheme() async -> Bool {
    let alert = NSAlert()
    alert.messageText = "Replace this document's colors?"
    alert.informativeText = "Importing replaces every color you changed. Reset to Original can return to the template's colors, not to yours."
    alert.addButton(withTitle: "Replace")
    alert.addButton(withTitle: "Cancel")
    guard let window else { return alert.runModal() == .alertFirstButtonReturn }
    return await withCheckedContinuation { done in
      alert.beginSheetModal(for: window) { done.resume(returning: $0 == .alertFirstButtonReturn) }
    }
  }
}

final class SlopThemePanel: NSPanel {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { false }
}

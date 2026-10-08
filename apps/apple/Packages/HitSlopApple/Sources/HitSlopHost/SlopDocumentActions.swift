import AppKit
import HitSlopCore
import HitSlopDocument

/// Toolbar and menu actions. Document operations (duplicate, export, share, theme files,
/// retry, close) go to the app, which runs them one at a time; pinning, the theme panel and
/// the file's location only change the window, so the window does them at once.
extension SlopDocumentWindowController {
  /// Whether `command` can run now: the one rule the toolbar, the menu bar and requests
  /// follow. Closing, retrying and the save-failure sheet's choices always can; anything
  /// else waits for the app to accept commands and the page to show its content, and
  /// importing a palette needs a document whose theme can change.
  public func isAvailable(_ command: SlopDocumentCommand) -> Bool {
    switch command {
    case .close, .retry, .retrySave, .discardUnsaved: true
    case .importTheme: commandsEnabled && isContentReady && session.canEditTheme
    default: commandsEnabled && isContentReady
    }
  }
  func request(_ command: SlopDocumentCommand) {
    guard isAvailable(command) else { return }
    routing.command(command)
  }
  /// Pinning keeps the window above others; it needs the page's content.
  public var canPin: Bool { isContentReady && fullscreenRestore == nil && !fullscreenTransition }
  public func togglePin() {
    guard canPin else { return }
    setPinned(!isPinned)
  }
  /// The theme panel opens for a page whose palette can change, and always closes.
  public var canToggleTheme: Bool { isThemeShown || (isContentReady && session.canEditTheme) }
  public func toggleTheme() { setThemeShown(!isThemeShown) }
  public func perform(_ command: SlopDocumentCommand) async throws -> URL? {
    switch command {
    case .importTheme: try await importTheme()
    case .exportTheme: try await exportTheme()
    case .duplicate:
      let panel = NSSavePanel()
      panel.allowedContentTypes = [.slop]
      panel.nameFieldStringValue =
        url.deletingPathExtension().lastPathComponent + " copy.slop"
      panel.startOnDesktop()
      return try await duplicateDocument(to: await runSheet(panel))
    case .share: try await shareCopy()

    case .exportPNG: try await export(.png)
    case .exportPDF: try await export(.pdf)
    case .retry:
      telemetry.send(.breadcrumb(.recovery, .started))
      do {
        if session.isReady && !session.rendererDead {
          try await session.reloadInterface()
          updatePageFailure(nil)
          startLoading()
        } else {
          // Replacing the renderer attaches and loads the new page (rendererReplaced).
          try await session.reopenSavedDocument()
        }
        telemetry.send(.breadcrumb(.recovery, .completed))
      } catch {
        reportLifecycleFailure(.recovery, error: error)
        throw error
      }
    case .close: try await closeDocument()
    case .retrySave: try await session.retrySave()
    case .discardUnsaved:
      try await session.discardPending()
      try await session.retrySave()
    }
    return nil
  }

  /// A save or open panel as a sheet on the document window; nil when cancelled.
  func runSheet(_ panel: NSSavePanel) async -> URL? {
    let response = if let window { await panel.beginSheetModal(for: window) } else { await panel.begin() }
    return response == .OK ? panel.url : nil
  }

  func duplicateDocument(to target: URL?) async throws -> URL? {
    guard let target else {
      telemetry.send(.breadcrumb(.duplicate, .cancelled))
      return nil
    }
    telemetry.send(.breadcrumb(.duplicate, .started))
    do {
      let copied = try await copyDocument(to: try SlopFile.newDocumentURL(target))
      telemetry.send(.breadcrumb(.duplicate, .completed))
      return copied
    } catch {
      reportLifecycleFailure(.duplicate, error: error)
      throw error
    }
  }

  /// Shares a copy of the document as a new logical document: everything the page has
  /// accepted, saved and copied through the owner, never the live file.
  func shareCopy() async throws {
    telemetry.send(.breadcrumb(.share, .started))
    do {
      let operation = try await SlopShareOperation.prepare(filename: url.lastPathComponent, telemetry: telemetry) {
        target in
        _ = try await self.copyDocument(to: target)
      }
      try operation.present(in: isHiddenForClose ? nil : window?.contentView)
    } catch {
      reportLifecycleFailure(.share, error: error)
      throw error
    }
  }

  /// The copy and its artwork consume one saved source, even if the editor changes or closes.
  private func copyDocument(to target: URL) async throws -> URL {
    let canRender = session.isReady && !session.rendererDead
    let telemetry = telemetry
    return try await session.copy(to: target) { source in
      guard canRender else { return nil }
      return await SlopRenderer.artwork(url: source, telemetry: telemetry)
    }
  }

  /// Save status and renderer callbacks own their incidents; outer operations add only context.
  func reportLifecycleFailure(
    _ operation: SlopTelemetryEvent.Failure, error: Error,
    format: ExportFormat? = nil
  ) {
    if SlopFailureContext.isCancellation(error) {
      telemetry.send(.breadcrumb(operation, .cancelled))
      return
    }
    if reportedSaveFailure || reportedRendererFailure {
      telemetry.send(.breadcrumb(operation, .failed))
      return
    }
    telemetry.failure(operation, error: error, format: format)
  }

  private func export(_ format: ExportFormat) async throws {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [format == .png ? .png : .pdf]
    panel.nameFieldStringValue = url.deletingPathExtension().lastPathComponent + "." + format.fileExtension
    try await exportDocument(format: format, to: await runSheet(panel))
  }

  /// The one export path, for the menu and for `slop export` of this open document. A
  /// cancelled picker has no output and emits no success event.
  func exportDocument(format: ExportFormat, to output: URL?, deadline: NativeCommandDeadline = NativeCommandDeadline())
    async throws
  {
    guard let output else {
      telemetry.send(.breadcrumb(.export, .cancelled))
      return
    }
    telemetry.send(.breadcrumb(.export, .started))
    await waitForPresentation()
    do {
      guard isContentReady, session.isReady, presentedPageError == nil else {
        throw SlopFailure("The document is not ready to export")
      }
      try await SlopRenderer.exportDocument(session: session, format: format, output: output, deadline: deadline)
      telemetry.send(.breadcrumb(.export, .completed))
      telemetry.send(.exported(format))
    } catch {
      reportLifecycleFailure(.export, error: error, format: format)
      throw error
    }
  }
  func reveal() { NSWorkspace.shared.activateFileViewerSelecting([url]) }
  func copyPath() { NSPasteboard.general.copy(url.path) }
  /// Opens the folder that holds the document, where an agent or a terminal runs `slop`
  /// commands on it. The document itself is a database, not something to edit as text.
  func openInEditor(_ app: URL) {
    NSWorkspace.shared.open(
      [url.deletingLastPathComponent()], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration(),
      completionHandler: nil)
  }
}

extension SlopDocumentFailure {
  /// How the coordinator treats a failed window command. Flush and close report a failed
  /// save as the owner's `SaveFailure`, which the save-failure sheet already shows.
  public init(command error: Error) {
    if error is SaveFailure {
      self = .save
    } else if SlopFailureContext.isCancellation(error) {
      self = .cancelled
    } else {
      self.init(error)
    }
  }
}

/// Installed editors, found once per process.
enum SlopEditors {
  static let installed: [(String, URL)] = slopOpenInCatalog().compactMap { title, bundleID, fallbackPath in
    if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
      return (title, url)
    }
    return FileManager.default.fileExists(atPath: fallbackPath)
      ? (title, URL(fileURLWithPath: fallbackPath)) : nil
  }
}

/// Launch Services first, then these fallback paths, so non-/Applications installs still appear.
func slopOpenInCatalog() -> [(String, String, String)] {
  [
    ("Open in Cursor", "com.todesktop.230313mzl4w4u92", "/Applications/Cursor.app"),
    ("Open in Visual Studio Code", "com.microsoft.VSCode", "/Applications/Visual Studio Code.app"),
    (
      "Open in VS Code Insiders", "com.microsoft.VSCodeInsiders",
      "/Applications/Visual Studio Code - Insiders.app"
    ),
    ("Open in Terminal", "com.apple.Terminal", "/System/Applications/Utilities/Terminal.app"),
    ("Open in iTerm", "com.googlecode.iterm2", "/Applications/iTerm.app"),
    ("Open in Warp", "dev.warp.Warp-Stable", "/Applications/Warp.app"),
    ("Open in Wave", "dev.commandline.waveterm", "/Applications/Wave.app"),
    ("Open in Ghostty", "com.mitchellh.ghostty", "/Applications/Ghostty.app"),
  ]
}

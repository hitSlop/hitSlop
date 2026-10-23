import AppKit
import HitSlopCore
import HitSlopDocument

/// Toolbar and menu actions. Document operations (duplicate, export, share, theme files,
/// retry, close) go to the app, which runs them one at a time; pinning, the theme panel and
/// the file's location only change the window, so the window does them at once.
extension SlopDocumentWindowController {
  public var canExport: Bool { session.file.views.export }
  /// Whether this is a shared document, which can save an unshared copy.
  public var isShared: Bool { session.retainsHistory }
  /// A local slop can move into the library while iCloud is connected. The app checks this
  /// again when the move runs, after the queue has stopped accepting other commands.
  public var canMoveToCloud: Bool {
    SlopLibrary.shared.isReady && SlopLibrary.shared.entry(at: session.file.url) == nil
  }

  /// Whether `command` can run now: the one rule the toolbar, the menu bar and requests
  /// follow. Closing, retrying and the save-failure sheet's choices always can; anything
  /// else waits for the app to accept commands and the page to show its content, and
  /// importing a palette needs a page that accepts work.
  public func isAvailable(_ command: SlopDocumentCommand) -> Bool {
    switch command {
    case .moveToCloud: commandsEnabled && isContentReady && canMoveToCloud
    case .close, .retry, .retrySave, .discardUnsaved: true
    case .duplicate: commandsEnabled && isContentReady
    case .saveUnsharedCopy:
      commandsEnabled && isShared && (isContentReady || saveAttention.failure == .full)
    case .sharePNG, .sharePDF, .exportPNG, .exportPDF: commandsEnabled && isContentReady && canExport
    case .importTheme: commandsEnabled && isContentReady && session.acceptsPageWork
    default: commandsEnabled && isContentReady
    }
  }
  func request(_ command: SlopDocumentCommand) {
    guard isAvailable(command) else { return }
    routing.command(command)
  }
  /// Pinning keeps the window above others; it needs the page's content.
  public var canPin: Bool { isContentReady && fullscreen.saved == nil && !fullscreen.transitioning }
  public func togglePin() {
    guard canPin else { return }
    setPinned(!isPinned)
  }
  /// The theme panel opens for a page that accepts work, and always closes.
  public var canToggleTheme: Bool { isThemeShown || (isContentReady && session.acceptsPageWork) }
  public func toggleTheme() { setThemeShown(!isThemeShown) }
  public func perform(_ command: SlopDocumentCommand) async throws -> URL? {
    switch command {
    case .sharePNG, .sharePDF, .exportPNG, .exportPDF:
      guard canExport else { throw SlopFailure("This slop does not support PNG or PDF export") }
    default: break
    }
    switch command {
    case .importTheme: try await importTheme()
    case .exportTheme: try await exportTheme()
    case .duplicate: return try await saveCopy(unshared: false)
    case .saveUnsharedCopy: return try await saveCopy(unshared: true)
    case .share:
      toolbar.willHide = { [weak self] in self?.sharePopover.dismiss() }
      toolbar.show()
      toolbar.panel?.contentView?.layoutSubtreeIfNeeded()
      if let anchor = toolbar.shareAnchor { sharePopover.toggle(at: anchor) }
    case .sharePDF: try await share(format: .pdf)
    case .sharePNG: try await share(format: .png)
    case .shareSlop: try await share(format: nil)

    case .exportPNG: try await export(.png)
    case .exportPDF: try await export(.pdf)
    case .retry:
      telemetry.send(.breadcrumb(.recovery, .started))
      do {
        if session.isReady {
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
    case .moveToCloud: throw SlopFailure("Move to iCloud must run through the app.")
    case .close: try await closeDocument()
    case .retrySave: try await session.retrySave()
    case .discardUnsaved:
      try await session.discardPending()
      try await session.retrySave()
    }
    return nil
  }

  /// Asks where the copy goes, then makes it: the one picker flow for both copy commands.
  private func saveCopy(unshared: Bool) async throws -> URL? {
    sharePopover.dismiss()
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.slop]
    panel.nameFieldStringValue = url.deletingPathExtension().lastPathComponent + " copy.slop"
    try panel.startInLocalDocuments()
    return try await duplicateDocument(to: await SlopPanels.select(panel, in: window), unshared: unshared)
  }

  /// `unshared` is Save Unshared Copy: a full shared document cannot save, so it would never
  /// close. The owner hands it off to the copy, and its window closes behind the command.
  /// Duplicate always copies.
  func duplicateDocument(to target: URL?, unshared: Bool = false) async throws -> URL? {
    guard let target else {
      telemetry.send(.breadcrumb(.duplicate, .cancelled))
      return nil
    }
    telemetry.send(.breadcrumb(.duplicate, .started))
    do {
      let destination = try SlopFile.newDocumentURL(target)
      let handOff = unshared && session.isFullShared
      let copied =
        handOff
        ? try await handOffDocument(to: destination)
        : try await copyDocument(to: destination)
      if handOff { routing.command(.close) }
      SlopSaveLocation.remember(copied)
      telemetry.send(.breadcrumb(.duplicate, .completed))
      return copied
    } catch {
      reportLifecycleFailure(.duplicate, error: error)
      throw error
    }
  }

  /// Prepares a saved-state export, or an independently editable slop, in staging owned
  /// by the share service. Neither path shares the live database.
  func prepareShare(format: ExportFormat?) async throws -> SlopShareOperation {
    if format != nil, !canExport { throw SlopFailure("This slop does not support PNG or PDF export") }
    let filename =
      format.map { url.deletingPathExtension().lastPathComponent + "." + $0.fileExtension }
      ?? url.lastPathComponent
    return try await SlopShareOperation.prepare(filename: filename, telemetry: telemetry) { target in
      if let format {
        await self.waitForPresentation()
        guard self.isContentReady, self.session.isReady, self.presentedPageError == nil else {
          throw SlopFailure("The document is not ready to share")
        }
        try await SlopRenderer.exportDocument(session: self.session, format: format, output: target)
      } else {
        _ = try await self.copyDocument(to: target)
      }
    }
  }

  private func share(format: ExportFormat?) async throws {
    telemetry.send(.breadcrumb(.share, .started))
    sharePopover.beginPreparing(
      format == .pdf ? "Preparing PDF…" : format == .png ? "Preparing image…" : "Preparing copy…")
    do {
      let operation = try await prepareShare(format: format)
      guard sharePopover.isShown, !isHiddenForClose, window?.isVisible == true else { throw CancellationError() }
      sharePopover.handOffToPicker()
      try operation.present(
        in: toolbar.shareAnchor,
        pickerDismissed: { [weak self] in
          self?.sharePopover.pickerDismissed()
        })
    } catch {
      sharePopover.dismiss()
      sharePopover.pickerDismissed()
      reportLifecycleFailure(.share, error: error, format: format)
      throw error
    }
  }

  /// The copy and its artwork consume one saved source, even if the editor changes or closes.
  private func copyDocument(to target: URL) async throws -> URL {
    let canRender = session.isReady
    let telemetry = telemetry
    return try await session.copy(to: target) { source in
      guard canRender else { return nil }
      return await SlopRenderer.artwork(url: source, telemetry: telemetry)
    }
  }

  /// A full shared document's way out: the copy and its artwork come from the frozen live
  /// state, and the owner publishes the copy and releases the original in one operation.
  private func handOffDocument(to target: URL) async throws -> URL {
    let canRender = session.isReady
    let telemetry = telemetry
    return try await session.handOff(
      to: target,
      resolveDrafts: { drafts in
        guard !drafts.isEmpty else { return }
        guard let window = self.window else { throw CancellationError() }
        try await self.draftRecovery.resolve(
          drafts, document: self.url.lastPathComponent, in: window, copy: TextDraftRecovery.copy)
      },
      renderArtwork: { source in
        guard canRender else { return nil }
        return await SlopRenderer.artwork(url: source, telemetry: telemetry)
      })
  }

  /// Save status and renderer callbacks own their incidents; outer operations add only context.
  func reportLifecycleFailure(
    _ operation: SlopTelemetryEvent.Failure, error: Error,
    format: ExportFormat? = nil
  ) {
    failures.lifecycle(operation, error: error, format: format, telemetry: telemetry)
  }

  private func export(_ format: ExportFormat) async throws {
    guard canExport else { throw SlopFailure("This slop does not support PNG or PDF export") }
    sharePopover.dismiss()
    let panel = NSSavePanel()
    panel.allowedContentTypes = [format == .png ? .png : .pdf]
    panel.nameFieldStringValue = url.deletingPathExtension().lastPathComponent + "." + format.fileExtension
    try await exportDocument(format: format, to: await SlopPanels.select(panel, in: window))
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

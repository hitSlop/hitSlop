import AppKit
import HitSlopCore
import HitSlopDocument

/// Toolbar and menu commands: pin, theme, duplicate, export, reveal, open in editor, retry,
/// close.
extension SlopDocumentWindowController {
  func request(_ command: SlopDocumentCommand) {
    if !isContentReady {
      switch command {
      case .exportPNG, .exportPDF, .duplicate, .theme(true), .importTheme, .exportTheme: return
      default: break
      }
    }
    if let onCommand {
      onCommand(command)
      return
    }
    Task {
      do { _ = try await perform(command) } catch {
        present("Could not complete command", error)
      }
    }
  }
  public func perform(_ command: SlopDocumentCommand) async throws -> URL? {
    switch command {
    case .pin(let pinned): setPinned(pinned)
    case .theme(let shown): setThemeShown(shown)
    case .importTheme: try await importTheme()
    case .exportTheme: try await exportTheme()
    case .duplicate:
      let panel = NSSavePanel()
      panel.allowedContentTypes = [.slop]
      panel.nameFieldStringValue =
        packageURL.deletingPathExtension().lastPathComponent + " copy.slop"
      panel.startOnDesktop()
      return try await duplicateDocument(to: await runSheet(panel))

    case .exportPNG: try await export(.png)
    case .exportPDF: try await export(.pdf)
    case .reveal: reveal()
    case .copyPath: copyPath()
    case .openEditor(let app): try await openInEditor(app)
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
      } catch { reportLifecycleFailure(.recovery, error: error); throw error }
    case .close: try await closeDocument()
    }
    return nil
  }

  /// A save or open panel as a sheet on the document window; nil when cancelled.
  func runSheet(_ panel: NSSavePanel) async -> URL? {
    let response = await withCheckedContinuation { continuation in
      if let window {
        panel.beginSheetModal(for: window) { continuation.resume(returning: $0) }
      } else {
        panel.begin { continuation.resume(returning: $0) }
      }
    }
    return response == .OK ? panel.url : nil
  }

  func duplicateDocument(to target: URL?) async throws -> URL? {
    guard let target else { telemetry.send(.breadcrumb(.duplicate, .cancelled)); return nil }
    telemetry.send(.breadcrumb(.duplicate, .started))
    // A live page sends unsent text first; without one, the owner saves what it accepted.
    do {
      if session.isReady && !session.rendererDead { try await session.flush() } else { try await session.retrySave() }
    } catch { reportLifecycleFailure(.duplicate, error: error); throw error }
    do {
      let source = session.package.rootURL
      let destination = try await SlopPreparation.run { try SlopDuplicator.duplicate(from: source, to: target).rootURL }
      telemetry.send(.breadcrumb(.duplicate, .completed))
      return destination
    } catch { telemetry.failure(.duplicate, error: error); throw error }
  }

  /// Save status and renderer callbacks own their incidents; outer operations add only context.
  func reportLifecycleFailure(_ operation: SlopTelemetryEvent.Failure, error: Error,
                                      format: ExportFormat? = nil) {
    if SlopFailureContext.isCancellation(error) { telemetry.send(.breadcrumb(operation, .cancelled)); return }
    if reportedSaveFailure || reportedRendererFailure { telemetry.send(.breadcrumb(operation, .failed)); return }
    telemetry.failure(operation, error: error, format: format)
  }

  private func export(_ format: ExportFormat) async throws {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [format == .png ? .png : .pdf]
    panel.nameFieldStringValue = packageURL.deletingPathExtension().lastPathComponent + "." + format.rawValue
    let output = panel.runModal() == .OK ? panel.url : nil
    try await exportDocument(format: format, to: output)
  }

  /// A cancelled picker has no output and emits no success event.
  func exportDocument(format: ExportFormat, to output: URL?) async throws {
    guard let output else { telemetry.send(.breadcrumb(.export, .cancelled)); return }
    telemetry.send(.breadcrumb(.export, .started))
    await waitForPresentation()
    do {
      guard isContentReady, session.isReady, presentedPageError == nil else {
        throw SlopPackageError.invalid("The document is not ready to export")
      }
      try await SlopRenderer.exportDocument(session: session, format: format, output: output)
      telemetry.send(.breadcrumb(.export, .completed))
      telemetry.send(.exported(format))
    } catch {
      reportLifecycleFailure(.export, error: error, format: format)
      throw error
    }
  }
  private func reveal() { NSWorkspace.shared.activateFileViewerSelecting([packageURL]) }
  private func copyPath() {
    NSPasteboard.general.copy(packageURL.path)
  }
  private func openInEditor(_ app: URL) async throws {
    let directory = URL(fileURLWithPath: packageURL.path, isDirectory: true)
    _ = try await NSWorkspace.shared.open(
      [directory], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration())
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

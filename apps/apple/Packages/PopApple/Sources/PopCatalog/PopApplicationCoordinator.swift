import AppKit
import Combine
import PopCore
import PopDocument
import PopFeatures
import PopHost
import PopLaunch
import SwiftUI

/// The macOS composition root. The models own decisions; `DocumentWindows` owns the native
/// documents.
@MainActor public final class PopApplicationCoordinator {
  let model: AppModel
  private let presentsWindows: Bool
  private let windows: DocumentWindows
  private let catalogServices: CatalogServices
  private var recoveryObservers: [AnyCancellable] = []
  private var catalogWindow: NSWindowController?
  /// Where tests see the alerts the app would show.
  private let alertOverride: (@MainActor (AppAlert, UUID?) -> Void)?

  /// The app's coordinator, reporting through the app's `telemetry`.
  public convenience init(templatesURL: URL? = PopTemplateLocation.templatesRoot, telemetry: PopTelemetry) {
    self.init(templatesURL: templatesURL, presentsWindows: true, telemetry: telemetry)
  }

  /// Native integration tests use hidden windows and avoid modifying the user's recents;
  /// they may supply the catalog's client and receive the alerts.
  init(
    templatesURL: URL?, presentsWindows: Bool, telemetry: PopTelemetry = .disabled,
    catalogClient: CatalogClient? = nil, historyURL: URL? = nil,
    alert: (@MainActor (AppAlert, UUID?) -> Void)? = nil
  ) {
    self.presentsWindows = presentsWindows
    alertOverride = alert
    let windows = DocumentWindows(presentsWindows: presentsWindows, telemetry: telemetry)
    let catalogServices = CatalogServices(
      templatesURL: templatesURL, telemetry: telemetry,
      historyURL: historyURL ?? (presentsWindows ? RecentDocumentHistory.defaultURL : nil),
      systemRecents: { presentsWindows ? CatalogServices.systemRecentDocuments() : [] })
    self.windows = windows
    self.catalogServices = catalogServices
    let catalog = CatalogModel(
      client: catalogClient ?? (presentsWindows || historyURL != nil ? catalogServices.client : .empty))
    model = AppModel(client: windows.client, catalog: catalog)
    windows.coordinator = self
    let recoveryNames: [Notification.Name] = [.popSharingRecoveryAvailable, .popOpenSharingRecovery]
    recoveryObservers = recoveryNames.map { name in
      NotificationCenter.default.publisher(for: name).sink { [weak self] notification in
        guard let url = notification.object as? URL else { return }
        Task { @MainActor [weak self] in
          guard let self else { return }
          await self.catalogServices.recordOpened(url)
          if name == .popOpenSharingRecovery { self.openDocument(url) }
        }
      }
    }
    if presentsWindows {
      Task { @MainActor [weak self] in
        do { try await SharingLinks.discoverRecoveries() } catch {
          self?.present(.failure(sharingMessage(error), title: "Could not find recovered copies"), for: nil)
        }
      }
    }

  }

  public var hasOpenDocuments: Bool { !model.documents.isEmpty }
  public var documentControllers: [PopDocumentWindowController] { Array(windows.controllers.values) }
  /// The key or main document window's controller: which commands it can run
  /// (`isAvailable`) and its state.
  public var activeController: PopDocumentWindowController? { activeID.flatMap { windows.controllers[$0] } }
  private var activeID: UUID? {
    // Resolve from AppKit at invocation; modal panels cannot retarget an existing operation.
    let candidate = NSApp.keyWindow ?? NSApp.mainWindow
    return windows.controllers.first { $0.value.owns(candidate) }?.key
  }

  public func showCatalog(launchGreeting: Bool = false) {
    guard presentsWindows, model.quitPhase == .running else { return }
    let catalogWindow = preparedCatalog(launchGreeting: launchGreeting)
    catalogWindow.showWindow(nil)
    if catalogWindow.window?.isMiniaturized == true { catalogWindow.window?.deminiaturize(nil) }
    catalogWindow.window?.makeKeyAndOrderFront(nil)
    NSApp.activate()
  }
  /// The catalog window, built once and reused.
  private func preparedCatalog(launchGreeting: Bool = false) -> NSWindowController {
    if let catalogWindow { return catalogWindow }
    let host = NSHostingController(rootView: CatalogView(model: model.catalog))
    // The window owns its size. The sidebar's full category list must scroll rather
    // than become the hosting controller's minimum window height.
    host.sizingOptions = []
    let size = NSSize(width: 1040, height: 680)
    let window = NSWindow(
      contentRect: NSRect(origin: .zero, size: size),
      styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered,
      defer: false)
    window.title = ""
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = true
    window.isReleasedWhenClosed = false
    window.minSize = NSSize(width: 900, height: 560)
    window.contentViewController = host
    window.setContentSize(size)
    if launchGreeting {
      // A removable native sibling keeps the greeting out of the catalog's size negotiation.
      let cover = NSHostingView(rootView: CatalogLaunchCover(model: model.catalog))
      cover.sizingOptions = []
      cover.rootView.onFinished = { [weak cover] in cover?.removeFromSuperview() }
      cover.frame = host.view.bounds
      cover.autoresizingMask = [.width, .height]
      host.view.addSubview(cover)
    }
    window.center()
    let controller = NSWindowController(window: window)
    catalogWindow = controller
    return controller
  }
  /// An app opened straight into a document has not built its catalog. Build it hidden
  /// once that document is showing, so closing the last document returns to the catalog
  /// without the first-show cost.
  func prewarmCatalog() {
    guard presentsWindows, catalogWindow == nil, model.quitPhase == .running else { return }
    Task { @MainActor [weak self] in
      try? await Task.sleep(for: .milliseconds(500))
      guard let self, self.catalogWindow == nil, self.model.quitPhase == .running else { return }
      self.model.catalog.start()
      self.preparedCatalog().window?.contentView?.layoutSubtreeIfNeeded()
    }
  }
  /// A document's page is ready; the catalog that opened it steps aside.
  func hideCatalog() { catalogWindow?.window?.orderOut(nil) }

  public func openDocument(_ url: URL) {
    guard model.quitPhase == .running else { return }
    // A template opens by creating a document from it, wherever it lives. Its header
    // decides, so a document is checked once, by its owner's open. Anything else,
    // including a file that fails its checks, goes to the document path, which reports.
    Task {
      do {
        let (canonical, template) = try await PopPreparation.run { () throws -> (URL, CatalogEntry?) in
          let canonical = try PopFile.resolvedRoot(url)
          let template =
            try PopFile.kind(of: canonical) == .template
            ? CatalogScanner.entry(template: try PopSummary(url: canonical)) : nil
          return (canonical, template)
        }
        guard model.quitPhase == .running else { return }
        if let template {
          showCatalog()
          model.catalog.primaryAction(template)
          return
        }
        let local = try await PopPreparation.run { (try? PopLocalDocument.requireLocal(canonical)) != nil }
        if local { return model.open(canonical) }
        guard presentsWindows else {
          return present(.failure("Move this document to ~/pop to edit it locally."), for: nil)
        }
        let alert = NSAlert()
        alert.messageText = "Move this document to local storage?"
        alert.informativeText =
          "pop keeps active documents outside iCloud. A verified local copy will be saved in ~/pop, then the original will be moved to Trash."
        alert.addButton(withTitle: "Move to ~/pop and Open")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn, model.quitPhase == .running else { return }
        let moved = try await PopPreparation.run { try PopFile.moveToLocalFolder(canonical) }
        if let warning = moved.warning {
          present(
            .failure("The local document was saved at \(moved.url.path). The original was retained: \(warning)"),
            for: nil)
        }
        guard model.quitPhase == .running else { return }
        model.open(moved.url)
      } catch {
        present(.failure(error.localizedDescription), for: nil)
      }
    }
  }
  public func revealDocuments() {
    let documents = model.documents.filter {
      !$0.isHiddenForClose && windows.controllers[$0.id]?.isHiddenForClose != true
    }
    if documents.isEmpty { showCatalog() }
    for document in documents { windows.focus(document.id) }
    NSApp.activate()
  }
  public func sendToActiveDocument(_ command: PopDocumentCommand) {
    guard let id = activeID else { return }
    model.send(command, to: id)
  }
  public func clearRecentDocuments() {
    Task {
      await catalogServices.clearRecents()
      if presentsWindows, RecentDocumentHistory.usesSystemRecents {
        NSDocumentController.shared.clearRecentDocuments(nil)
      }
      await model.catalog.reloadRecents()
    }
  }
  /// The menu uses the catalog's history and the same active-account filtering.
  public var recentDocuments: [CatalogEntry] { model.catalog.recents }
  public func refreshRecentDocuments() async { await model.catalog.reloadRecents() }
  public func openRecentDocument(id: String) {
    guard let entry = recentDocuments.first(where: { $0.id == id }) else { return }
    model.catalog.primaryAction(entry)
  }
  fileprivate func recordRecentDocument(_ url: URL) async { await catalogServices.recordOpened(url) }
  public func requestQuit() { model.requestQuit() }

  /// Runs the app's update check. The app installs it; Sparkle lives in the app target.
  public var checkForUpdates: @MainActor () -> Void = {}

  /// An alert on the document's window, or over the app.
  func present(_ alert: AppAlert, for id: UUID?) {
    if let alertOverride { return alertOverride(alert, id) }
    guard presentsWindows else { return }
    let window = id.flatMap { windows.controllers[$0]?.window } ?? NSApp.keyWindow ?? catalogWindow?.window
    NativeAlertPresenter.present(alert, on: window) { [weak self] in self?.checkForUpdates() }
  }
}

private struct CatalogLaunchCover: View {
  @Bindable var model: CatalogModel
  var onFinished: @MainActor () -> Void = {}
  var body: some View {
    Color.clear.popLaunchGreeting(
      ready: model.hasLoadedLocal && model.hasLoadedRecents, onFinished: onFinished)
  }
}

/// The open documents' windows, and the native side of `AppModel`'s client.
@MainActor private final class DocumentWindows {
  private let presentsWindows: Bool
  private let telemetry: PopTelemetry
  init(presentsWindows: Bool, telemetry: PopTelemetry) {
    self.presentsWindows = presentsWindows
    self.telemetry = telemetry
  }
  weak var coordinator: PopApplicationCoordinator?
  var controllers: [UUID: PopDocumentWindowController] = [:]
  /// Each document still opening's progress panel, which focus brings forward.
  private var openings: [UUID: PopOpeningProgress] = [:]
  private var quitCopies = QuitDraftCopies()
  private var adoptedSources: [URL: (URL, String)] = [:]
  var client: AppClient {
    AppClient(
      open: { [self] id, url in
        telemetry.send(.breadcrumb(.open, .started))
        do { try await open(id, url: url) } catch {
          telemetry.failure(.open, error: error)
          throw error
        }
      },
      focus: { [self] id in focus(id) },
      perform: { [self] id, command in
        do { return try await perform(id, command: command) } catch { throw PopDocumentFailure(error) }
      },
      prepareToQuit: { [self] id in
        do {
          try await controller(id).prepareToClose(operation: .quit) { [self] text in
            quitCopies.copy(text, write: writeDrafts)
          }
        } catch {
          throw PopDocumentFailure(error)
        }
      },
      finishQuit: { [self] id in
        do { try await finishQuit(id) } catch { throw PopDocumentFailure(error) }
      },
      cancelQuit: { [self] id in
        quitCopies = QuitDraftCopies()
        await controllers[id]?.cancelPreparedClose()
      },
      replyToQuit: { allowed in NSApp.reply(toApplicationShouldTerminate: allowed) },
      alert: { [self] alert, id in coordinator?.present(alert, for: id) },
      commandsEnabled: { [self] id, enabled in controllers[id]?.setCommandsEnabled(enabled) },
      showCatalog: { [self] in
        coordinator?.showCatalog()
        if presentsWindows { PopCloseTrace.browserHandoff() }
      }
    )
  }
  func controller(_ id: UUID) throws -> PopDocumentWindowController {
    guard let controller = controllers[id] else { throw PopFailure("The document is no longer open.") }
    return controller
  }
  private func open(_ id: UUID, url: URL) async throws {
    try Task.checkCancellation()
    let routing = PopDocumentRouting(
      command: { [weak self] command in self?.coordinator?.model.send(command, to: id) },
      pageReady: { [weak self] in
        self?.coordinator?.hideCatalog()
        self?.coordinator?.prewarmCatalog()
      },
      closeHidden: { [weak self] in self?.coordinator?.model.documentHiddenForClose(id) },
      checkForUpdates: { [weak self] in self?.coordinator?.checkForUpdates() })
    let progress = presentsWindows ? PopOpeningProgress() : nil
    openings[id] = progress
    defer { openings[id] = nil }
    let controller = try await PopDocumentWindowController.open(
      url: url, routing: routing, progress: progress, telemetry: telemetry)
    controllers[id] = controller
    if let (source, documentID) = adoptedSources.removeValue(forKey: url) {
      do { try await PopLibrary.shared.trashAdopted(source: source, id: documentID) } catch {
        coordinator?.present(
          .failure(
            "Your iCloud pop is at \(url.path). The original is still at \(source.path). \(error.localizedDescription)"
          ), for: id)
      }
    }
    if presentsWindows {
      if RecentDocumentHistory.usesSystemRecents { NSDocumentController.shared.noteNewRecentDocumentURL(url) }
      NSApp.activate()
    }
    await coordinator?.recordRecentDocument(url)
    telemetry.send(.breadcrumb(.open, .completed))
    telemetry.send(.opened)
  }
  private func finishQuit(_ id: UUID) async throws {
    // A failed later pasteboard write may have cleared an earlier copy. Re-establish
    // all successful copies before the first window in this quit is destroyed.
    guard quitCopies.restore(write: writeDrafts) else {
      throw PopFailure("The unresolved text could not be copied. Your documents are still open.")
    }
    try await controller(id).finishClose(operation: .quit)
    quitCopies = QuitDraftCopies()
    controllers.removeValue(forKey: id)
  }
  private func writeDrafts(_ text: String) -> Bool {
    NSPasteboard.general.clearContents()
    return NSPasteboard.general.setString(text, forType: .string)
  }
  func focus(_ id: UUID) {
    if presentsWindows {
      if let controller = controllers[id] {
        controller.revealFromDock()
        let url = controller.session.file.url
        Task { [weak coordinator] in
          await coordinator?.recordRecentDocument(url)
          await coordinator?.refreshRecentDocuments()
        }
      } else {
        openings[id]?.focus()
      }
      NSApp.activate()
    }
  }
  private func perform(_ id: UUID, command: PopDocumentCommand) async throws -> URL? {
    let controller = try controller(id)
    if command == .moveToCloud {
      guard controller.canMoveToCloud else { throw PopFailure("This pop cannot be moved right now.") }
      let source = controller.session.file.url
      // The normal close path flushes text, saves artwork and releases the writer lock.
      _ = try await controller.perform(.close)
      controllers.removeValue(forKey: id)
      do {
        let entry = try await PopLibrary.shared.adopt(
          source: source, title: source.deletingPathExtension().lastPathComponent)
        let destination = URL(fileURLWithPath: entry.path)
        adoptedSources[destination] = (source, entry.id)
        return destination
      } catch {
        coordinator?.present(
          .failure("The original is still at \(source.path). \(error.localizedDescription)"), for: nil)
        return source
      }
    }
    let result = try await controller.perform(command)
    if command == .duplicate || command == .saveUnsharedCopy, result != nil { telemetry.send(.duplicated) }
    if command == .close { controllers.removeValue(forKey: id) }
    return result
  }
}

/// Each Copy during one quit preserves the text copied from its earlier windows.
struct QuitDraftCopies {
  private var copies: [String] = []

  mutating func copy(_ text: String, write: (String) -> Bool) -> Bool {
    let combined = (copies + [text]).joined(separator: "\n\n---\n\n")
    guard write(combined) else { return false }
    copies.append(text)
    return true
  }

  func restore(write: (String) -> Bool) -> Bool {
    copies.isEmpty || write(copies.joined(separator: "\n\n---\n\n"))
  }
}

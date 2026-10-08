import AppKit
import HitSlopCore
import HitSlopDocument
import HitSlopFeatures
import HitSlopHost
import SwiftUI

/// The macOS composition root. The models own decisions; `DocumentWindows` owns the native
/// documents.
@MainActor public final class SlopApplicationCoordinator {
  let model: AppModel
  private let presentsWindows: Bool
  private let windows: DocumentWindows
  private let catalogServices: CatalogServices
  private var catalogWindow: NSWindowController?
  /// Where tests see the alerts the app would show.
  private let alertOverride: (@MainActor (AppAlert, UUID?) -> Void)?

  /// The app's coordinator, reporting through the app's `telemetry`.
  public convenience init(templatesURL: URL? = SlopTemplateLocation.templatesRoot, telemetry: SlopTelemetry) {
    self.init(templatesURL: templatesURL, presentsWindows: true, telemetry: telemetry)
  }

  /// Native integration tests use hidden windows and avoid modifying the user's recents;
  /// they may supply the catalog's client and receive the alerts.
  init(
    templatesURL: URL?, presentsWindows: Bool, telemetry: SlopTelemetry = .disabled,
    catalogClient: CatalogClient? = nil, alert: (@MainActor (AppAlert, UUID?) -> Void)? = nil
  ) {
    self.presentsWindows = presentsWindows
    alertOverride = alert
    let windows = DocumentWindows(presentsWindows: presentsWindows, telemetry: telemetry)
    let catalogServices = CatalogServices(templatesURL: templatesURL, telemetry: telemetry)
    self.windows = windows
    self.catalogServices = catalogServices
    let catalog = CatalogModel(client: catalogClient ?? (presentsWindows ? catalogServices.client : .empty))
    model = AppModel(client: windows.client, catalog: catalog)
    windows.coordinator = self
  }

  public var hasOpenDocuments: Bool { !model.documents.isEmpty }
  public var documentControllers: [SlopDocumentWindowController] { Array(windows.controllers.values) }
  /// The key or main document window's controller: which commands it can run
  /// (`isAvailable`) and its state.
  public var activeController: SlopDocumentWindowController? { activeID.flatMap { windows.controllers[$0] } }
  private var activeID: UUID? {
    // Resolve from AppKit at invocation; modal panels cannot retarget an existing operation.
    let candidate = NSApp.keyWindow ?? NSApp.mainWindow
    return windows.controllers.first { $0.value.owns(candidate) }?.key
  }

  public func showCatalog() {
    guard presentsWindows, model.quitPhase == .running else { return }
    if catalogWindow == nil {
      let host = NSHostingController(rootView: CatalogView(model: model.catalog))
      let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720),
        styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered,
        defer: false)
      window.title = ""
      window.titleVisibility = .hidden
      window.titlebarAppearsTransparent = true
      window.isReleasedWhenClosed = false
      window.minSize = NSSize(width: 900, height: 600)
      window.contentViewController = host
      window.center()
      catalogWindow = NSWindowController(window: window)
    }
    catalogWindow?.showWindow(nil)
    if catalogWindow?.window?.isMiniaturized == true { catalogWindow?.window?.deminiaturize(nil) }
    catalogWindow?.window?.makeKeyAndOrderFront(nil)
    NSApp.activate()
  }
  /// A document's page is ready; the catalog that opened it steps aside.
  func hideCatalog() { catalogWindow?.window?.orderOut(nil) }

  public func openDocument(_ url: URL) {
    guard model.quitPhase == .running else { return }
    let canonical: URL
    do { canonical = try SlopFile.resolvedRoot(url) } catch {
      present(.failure(error.localizedDescription), for: nil)
      return
    }
    // A template opens by creating a document from it, wherever it lives. Its header
    // decides, so a document is checked once, by its owner's open. Anything else,
    // including a file that fails its checks, goes to the document path, which reports.
    Task {
      let template = try? await SlopPreparation.run { () throws -> CatalogEntry? in
        guard try SlopFile.kind(of: canonical) == .template else { return nil }
        return CatalogScanner.entry(template: try SlopSummary(url: canonical))
      }
      guard model.quitPhase == .running else { return }
      guard let template else { return model.open(canonical) }
      showCatalog()
      model.catalog.primaryAction(template)
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
  public func sendToActiveDocument(_ command: SlopDocumentCommand) {
    guard let id = activeID else { return }
    model.send(command, to: id)
  }
  public func clearRecentDocuments() {
    NSDocumentController.shared.clearRecentDocuments(nil)
    model.catalog.refreshRecents()
  }
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

/// The open documents' windows, and the native side of `AppModel`'s client.
@MainActor private final class DocumentWindows {
  private let presentsWindows: Bool
  private let telemetry: SlopTelemetry
  init(presentsWindows: Bool, telemetry: SlopTelemetry) {
    self.presentsWindows = presentsWindows
    self.telemetry = telemetry
  }
  weak var coordinator: SlopApplicationCoordinator?
  var controllers: [UUID: SlopDocumentWindowController] = [:]
  /// Each document still opening's progress panel, which focus brings forward.
  private var openings: [UUID: SlopOpeningProgress] = [:]
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
        do { return try await perform(id, command: command) } catch { throw SlopDocumentFailure(command: error) }
      },
      prepareToQuit: { [self] id in
        do { try await controller(id).prepareToClose(operation: .quit) } catch {
          throw SlopDocumentFailure(command: error)
        }
      },
      finishQuit: { [self] id in
        do { try await finishQuit(id) } catch { throw SlopDocumentFailure(command: error) }
      },
      cancelQuit: { [self] id in await controllers[id]?.cancelPreparedClose() },
      replyToQuit: { allowed in NSApp.reply(toApplicationShouldTerminate: allowed) },
      alert: { [self] alert, id in coordinator?.present(alert, for: id) },
      commandsEnabled: { [self] id, enabled in controllers[id]?.setCommandsEnabled(enabled) },
      showCatalog: { [self] in
        coordinator?.showCatalog()
        if presentsWindows { SlopCloseTrace.browserHandoff() }
      }
    )
  }
  func controller(_ id: UUID) throws -> SlopDocumentWindowController {
    guard let controller = controllers[id] else { throw SlopFailure("The document is no longer open.") }
    return controller
  }
  private func open(_ id: UUID, url: URL) async throws {
    try Task.checkCancellation()
    let routing = SlopDocumentRouting(
      command: { [weak self] command in self?.coordinator?.model.send(command, to: id) },
      pageReady: { [weak self] in self?.coordinator?.hideCatalog() },
      closeHidden: { [weak self] in self?.coordinator?.model.documentHiddenForClose(id) })
    let progress = presentsWindows ? SlopOpeningProgress() : nil
    openings[id] = progress
    defer { openings[id] = nil }
    let controller = try await SlopDocumentWindowController.open(
      url: url, routing: routing, progress: progress, telemetry: telemetry)
    controllers[id] = controller
    if presentsWindows {
      NSDocumentController.shared.noteNewRecentDocumentURL(url)
      NSApp.activate()
    }
    telemetry.send(.breadcrumb(.open, .completed))
    telemetry.send(.opened)
  }
  private func finishQuit(_ id: UUID) async throws {
    try await controller(id).finishClose(operation: .quit)
    controllers.removeValue(forKey: id)
  }
  func focus(_ id: UUID) {
    if presentsWindows {
      if let controller = controllers[id] { controller.revealFromDock() } else { openings[id]?.focus() }
      NSApp.activate()
    }
  }
  private func perform(_ id: UUID, command: SlopDocumentCommand) async throws -> URL? {
    let controller = try controller(id)
    let result = try await controller.perform(command)
    if command == .duplicate, result != nil { telemetry.send(.duplicated) }
    if command == .close { controllers.removeValue(forKey: id) }
    return result
  }
}

import AppKit
import ComposableArchitecture
import HitSlopCore
import HitSlopFeatures
import HitSlopHost
import HitSlopDocument
import SwiftUI

/// The macOS composition root. Stores own decisions; services own native resources.
@MainActor public final class SlopApplicationCoordinator {
    let store: StoreOf<AppFeature>
    private let presentsWindows: Bool
    private let alerts = NativeAlertPresenter()
    private let native: NativeDocumentServices
    private let catalogServices: CatalogServices
    private var catalogWindow: NSWindowController?
    private var observation: ObserveToken?
    private var documentObservations: [UUID: ObserveToken] = [:]

    private var previousDocumentCount = 0

    /// The app's coordinator, reporting through the app's `telemetry`.
    public convenience init(templatesURL: URL = SlopTemplateLocation.templatesRoot, telemetry: SlopTelemetry) {
        self.init(templatesURL: templatesURL, presentsWindows: true, telemetry: telemetry)
    }

    /// Native integration tests use hidden windows and avoid modifying the user's recents;
    /// they may supply the catalog's client.
    init(templatesURL: URL, presentsWindows: Bool, telemetry: SlopTelemetry = .disabled, catalogClient: CatalogClient? = nil) {
        self.presentsWindows = presentsWindows
        let native = NativeDocumentServices(presentsWindows: presentsWindows, telemetry: telemetry)
        let catalogServices = CatalogServices(templatesURL: templatesURL, telemetry: telemetry)
        self.native = native; self.catalogServices = catalogServices
        store = Store(initialState: AppFeature.State()) { AppFeature() } withDependencies: {
            $0.catalogClient = catalogClient ?? (presentsWindows ? catalogServices.client : .empty)
            $0.documentClient = native.client
            $0.uuid = .init { UUID() }
        }
        native.routing = { [weak self] id in
            SlopDocumentRouting(
                command: { command in self?.send(.command(command), to: id) },
                pageReady: { self?.catalogWindow?.window?.orderOut(nil) },
                closed: { self?.documentObservations.removeValue(forKey: id) })
        }
        native.onOpened = { [weak self] id, controller in self?.connect(id, controller: controller) }
        observation = observe { [weak self] in
            guard let self else { return }
            let count = self.store.documents.count
            if count == 0 && self.previousDocumentCount > 0 && self.store.quitPhase == .running { self.showCatalog() }
            self.previousDocumentCount = count
            if self.presentsWindows, let alert = self.store.alert {
                self.alerts.enqueue(alert, window: NSApp.keyWindow ?? self.catalogWindow?.window, isCurrent: { [weak self] in
                    self?.store.alert?.id == alert.id
                }, dismiss: { [weak self] action in self?.dismissAlert(action) { self?.store.send(.alert(.dismiss)) } })
            }
        }
    }

    public var hasOpenDocuments: Bool { !store.documents.isEmpty }
    public var documentControllers: [SlopDocumentWindowController] { Array(native.controllers.values) }
    /// The key or main document window's controller: which commands it can run
    /// (`isAvailable`) and its state.
    public var activeController: SlopDocumentWindowController? { activeID.flatMap { native.controllers[$0] } }
    private var activeID: UUID? {
        // Resolve from AppKit at invocation; modal panels cannot retarget an existing operation.
        let candidate = NSApp.keyWindow ?? NSApp.mainWindow
        return native.controllers.first { $0.value.owns(candidate) }?.key
    }

    public func showCatalog() {
        guard presentsWindows, store.quitPhase == .running else { return }
        if catalogWindow == nil {
            let host = NSHostingController(rootView: CatalogView(store: store.scope(state: \.catalog, action: \.catalog)))
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720), styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
            window.title = ""; window.titleVisibility = .hidden; window.titlebarAppearsTransparent = true; window.isReleasedWhenClosed = false
            window.minSize = NSSize(width: 900, height: 600); window.contentViewController = host; window.center()
            catalogWindow = NSWindowController(window: window)
        }
        catalogWindow?.showWindow(nil); catalogWindow?.window?.deminiaturize(nil)
        catalogWindow?.window?.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
    }

    public func openDocument(_ url: URL) {
        guard store.quitPhase == .running else { return }
        let canonical: URL
        do { canonical = try SlopFile.resolvedRoot(url) } catch {
            store.send(.externalFailure(error.localizedDescription))
            return
        }
        // A template opens by creating a document from it, wherever it lives. Its header
        // decides, so a document is checked once, by its owner's open. Anything else,
        // including a file that fails its checks, goes to the document path, which reports.
        Task {
            let template = try? await SlopPreparation.run { () throws -> CatalogEntry? in
                guard try SlopFile.kind(of: canonical) == .template else { return nil }
                return CatalogScanner.entry(template: try SlopFile(url: canonical))
            }
            guard store.quitPhase == .running else { return }
            guard let template else { store.send(.openDocument(canonical)); return }
            showCatalog()
            store.send(.catalog(.primaryAction(template)))
        }
    }
    public func revealDocuments() {
        for controller in native.controllers.values { controller.revealFromDock() }
        NSApp.activate(ignoringOtherApps: true)
    }
    public func sendToActiveDocument(_ command: SlopDocumentCommand) {
        guard let id = activeID else { return }
        store.send(.documents(.element(id: id, action: .command(command))))
    }
    public func clearRecentDocuments() {
        NSDocumentController.shared.clearRecentDocuments(nil)
        store.send(.catalog(.refreshRecents))
    }
    public func requestQuit() { store.send(.quitRequested) }

    /// Runs the app's update check. The app installs it; Sparkle lives in the app target.
    public var checkForUpdates: @MainActor () -> Void = {}
    /// Store state stays authoritative: the chosen button's work runs here, then the alert
    /// is dismissed like any other.
    private func dismissAlert(_ action: ErrorAlertAction?, _ dismiss: () -> Void) {
        switch action {
        case .checkForUpdates: checkForUpdates()
        case nil: break
        }
        dismiss()
    }
    private func send(_ action: DocumentFeature.Action, to id: UUID) {
        guard store.documents[id: id] != nil else { return }
        store.send(.documents(.element(id: id, action: action)))
    }
    private func connect(_ id: UUID, controller: SlopDocumentWindowController) {
        if let document = store.scope(state: \.documents[id: id], action: \.documents[id: id]) {
            documentObservations[id] = observe { [weak self, weak controller] in
                controller?.setCommandsEnabled(document.acceptsCommands)
                if self?.presentsWindows == true, let alert = document.alert {
                    self?.alerts.enqueue(alert, window: controller?.window, isCurrent: { [weak self] in
                        self?.store.documents[id: id]?.alert?.id == alert.id
                    }, dismiss: { [weak self] action in self?.dismissAlert(action) { self?.send(.alert(.dismiss), to: id) } })
                }
            }
        }
    }

}

@MainActor private final class NativeDocumentServices {
    private let presentsWindows: Bool
    private let telemetry: SlopTelemetry
    init(presentsWindows: Bool, telemetry: SlopTelemetry) { self.presentsWindows = presentsWindows; self.telemetry = telemetry }
    var controllers: [UUID: SlopDocumentWindowController] = [:]
    /// Each document still opening's progress panel, which focus brings forward.
    private var openings: [UUID: SlopOpeningProgress] = [:]
    /// Sends a document's window commands to the coordinator, which runs them in order.
    var routing: ((UUID) -> SlopDocumentRouting)?
    var onOpened: ((UUID, SlopDocumentWindowController) -> Void)?
    var client: DocumentClient {
        DocumentClient(
            open: { [self] id, url in
                await telemetry.send(.breadcrumb(.open, .started))
                do { try await open(id, url: url) }
                catch { await telemetry.failure(.open, error: error); throw error }
            },
            focus: { [self] id in await focus(id) },
            perform: { [self] id, command in
                do { return try await perform(id, command: command) } catch { throw SlopDocumentFailure(command: error) }
            },
            prepareToQuit: { [self] id in
                do { try await prepareToQuit(id) } catch { throw SlopDocumentFailure(command: error) }
            },
            finishQuit: { [self] id in
                do { try await finishQuit(id) } catch { throw SlopDocumentFailure(command: error) }
            },
            cancelQuit: { [self] id in await cancelQuit(id) },
            replyToQuit: { allowed in await MainActor.run { NSApp.reply(toApplicationShouldTerminate: allowed) } }
        )
    }
    func controller(_ id: UUID) throws -> SlopDocumentWindowController {
        guard let controller = controllers[id] else { throw SlopFailure("The document is no longer open.") }
        return controller
    }
    private func open(_ id: UUID, url: URL) async throws {
        try Task.checkCancellation()
        guard let routing = routing?(id) else { throw SlopFailure("Document windows need a coordinator.") }
        let progress = presentsWindows ? SlopOpeningProgress() : nil
        openings[id] = progress
        defer { openings[id] = nil }
        let controller = try await SlopDocumentWindowController.open(url: url, routing: routing, progress: progress, telemetry: telemetry)
        controllers[id] = controller
        onOpened?(id, controller)
        if presentsWindows {
            NSDocumentController.shared.noteNewRecentDocumentURL(url); NSApp.activate(ignoringOtherApps: true)
        }
        telemetry.send(.breadcrumb(.open, .completed))
        telemetry.send(.opened)
    }
    private func finishQuit(_ id: UUID) async throws {
        try await controller(id).finishClose(operation: .quit)
        controllers.removeValue(forKey: id)
    }
    private func cancelQuit(_ id: UUID) async { await controllers[id]?.cancelPreparedClose() }
    private func prepareToQuit(_ id: UUID) async throws { try await controller(id).prepareToClose(operation: .quit) }
    private func focus(_ id: UUID) {
        if presentsWindows {
            if let controller = controllers[id] { controller.revealFromDock() } else { openings[id]?.focus() }
            NSApp.activate(ignoringOtherApps: true)
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

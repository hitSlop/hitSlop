import AppKit
import ComposableArchitecture
import HitSlopCore
import HitSlopFeatures
import HitSlopFirebase
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

    public convenience init(templatesURL: URL = SlopTemplateLocation.defaultTemplatesRoot) {
        self.init(templatesURL: templatesURL, presentsWindows: true)
    }

    /// Native integration tests use hidden windows and avoid modifying the user's recents.
    init(templatesURL: URL, presentsWindows: Bool) {
        self.presentsWindows = presentsWindows
        let native = NativeDocumentServices(templatesURL: templatesURL, presentsWindows: presentsWindows, telemetry: presentsWindows ? HitSlopFirebase.telemetry : .disabled)
        let catalogServices = CatalogServices(templatesURL: templatesURL, telemetry: presentsWindows ? HitSlopFirebase.telemetry : .disabled)
        self.native = native; self.catalogServices = catalogServices
        store = Store(initialState: AppFeature.State()) { AppFeature() } withDependencies: {
            $0.catalogClient = presentsWindows ? catalogServices.client : .empty
            $0.documentClient = native.client
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
                }, dismiss: { [weak self] in self?.store.send(.alert(.dismiss)) })
            }
        }
    }

    public var hasOpenDocuments: Bool { !store.documents.isEmpty }
    public var documentControllers: [SlopDocumentWindowController] { Array(native.controllers.values) }
    public var canPerformDocumentCommands: Bool {
        guard let id = activeID else { return false }
        return store.documents[id: id]?.acceptsCommands == true && native.controllers[id]?.isContentReady == true
    }
    public var isActiveDocumentPinned: Bool { activeID.flatMap { store.documents[id: $0]?.isPinned } ?? false }
    public var isActiveDocumentThemeShown: Bool { activeID.flatMap { native.controllers[$0]?.isThemeShown } ?? false }
    public var canEditActiveDocumentTheme: Bool { activeID.flatMap { native.controllers[$0]?.session.canEditTheme } ?? false }
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
        guard (try? url.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true else {
            store.send(.externalFailure("Document packages cannot be symlinks."))
            return
        }
        let canonical = url.standardizedFileURL.resolvingSymlinksInPath()
        if SlopTemplateLocation.isManagedTemplatePackage(canonical, templatesRoot: catalogServices.templatesURL) {
            showCatalog()
            Task { do {
                let package = try await SlopPreparation.run {
                    let package = try SlopPackage(rootURL: canonical)
                    try package.validateAsTemplate()
                    return package
                }
                guard store.quitPhase == .running else { return }
                var entry = CatalogEntry(id: "local:\(package.manifest.slug)", source: .local(canonical), title: package.manifest.title)
                CatalogServices.apply(package.manifest, to: &entry)
                store.send(.catalog(.primaryAction(entry)))
            } catch { store.send(.externalFailure(error.localizedDescription)) } }
        } else { store.send(.openDocument(canonical)) }
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

    private func send(_ action: DocumentFeature.Action, to id: UUID) {
        guard store.documents[id: id] != nil else { return }
        store.send(.documents(.element(id: id, action: action)))
    }
    private func connect(_ id: UUID, controller: SlopDocumentWindowController) {
        controller.onCommand = { [weak self] command in self?.send(.command(command), to: id) }
        controller.onPageReady = { [weak self] in
            self?.catalogWindow?.window?.orderOut(nil)
            self?.send(.pageReady, to: id)
        }
        controller.onPageFailure = { [weak self] message in self?.send(.pageFailed(message), to: id) }
        controller.onClose = { [weak self] in self?.documentObservations.removeValue(forKey: id) }
        if let document = store.scope(state: \.documents[id: id], action: \.documents[id: id]) {
            documentObservations[id] = observe { [weak self, weak controller] in
                controller?.updatePresentation(pinned: document.isPinned, commandsEnabled: document.acceptsCommands, pageError: document.pageError)
                if self?.presentsWindows == true, let alert = document.alert {
                    self?.alerts.enqueue(alert, window: controller?.window, isCurrent: { [weak self] in
                        self?.store.documents[id: id]?.alert?.id == alert.id
                    }, dismiss: { [weak self] in self?.send(.alert(.dismiss), to: id) })
                }
            }
        }
    }

}

@MainActor private final class NativeDocumentServices {
    private let templatesURL: URL
    private let presentsWindows: Bool
    private let telemetry: SlopTelemetry
    init(templatesURL: URL, presentsWindows: Bool, telemetry: SlopTelemetry) { self.templatesURL = templatesURL; self.presentsWindows = presentsWindows; self.telemetry = telemetry }
    var controllers: [UUID: SlopDocumentWindowController] = [:]
    private var preparingURLs: [UUID: URL] = [:]
    var onOpened: ((UUID, SlopDocumentWindowController) -> Void)?
    var client: DocumentClient {
        DocumentClient(
            open: { [self] id, url in
                await telemetry.send(.breadcrumb(.open, .started))
                do { return try await open(id, url: url) }
                catch { await telemetry.failure(.open, error: error); throw error }
            },
            focus: { [self] id in await focus(id) },
            perform: { [self] id, command in try await perform(id, command: command) },
            prepareToQuit: { [self] id in try await prepareToQuit(id) },
            finishQuit: { [self] id in try await finishQuit(id) },
            cancelQuit: { [self] id in await cancelQuit(id) },
            finishAssetRefreshes: { await SlopDocumentWindowController.finishAssetRefreshesForTermination() },
            replyToQuit: { allowed in await MainActor.run { NSApp.reply(toApplicationShouldTerminate: allowed) } }
        )
    }
    func controller(_ id: UUID) throws -> SlopDocumentWindowController {
        guard let controller = controllers[id] else { throw SlopPackageError.invalid("The document is no longer open.") }
        return controller
    }
    private func open(_ id: UUID, url: URL) async throws -> String {
        guard !SlopTemplateLocation.isManagedTemplatePackage(url, templatesRoot: templatesURL) else {
            throw SlopPackageError.invalid("Create a document from this template before opening it.")
        }
        try Task.checkCancellation()
        preparingURLs[id] = url
        defer { preparingURLs[id] = nil }
        let controller = try await SlopDocumentWindowController.open(packageURL: url, presentsWindow: presentsWindows, telemetry: telemetry)
        controllers[id] = controller
        onOpened?(id, controller)
        if presentsWindows {
            NSDocumentController.shared.noteNewRecentDocumentURL(url); NSApp.activate(ignoringOtherApps: true)
        }
        telemetry.send(.breadcrumb(.open, .completed))
        telemetry.send(.opened)
        return controller.documentTitle
    }
    private func finishQuit(_ id: UUID) async throws {
        try await controller(id).finishClose(operation: .quit)
        controllers.removeValue(forKey: id)
    }
    private func cancelQuit(_ id: UUID) async { await controllers[id]?.cancelPreparedClose() }
    private func prepareToQuit(_ id: UUID) async throws { try await controller(id).prepareToClose(operation: .quit) }
    private func focus(_ id: UUID) {
        if presentsWindows {
            if let controller = controllers[id] { controller.revealFromDock() }
            else if let url = preparingURLs[id] { SlopDocumentWindowController.focusOpeningDocument(at: url) }
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

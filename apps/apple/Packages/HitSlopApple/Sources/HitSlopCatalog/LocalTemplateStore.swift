import Combine
import Darwin
import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopDocument

public struct LocalTemplate: Identifiable, Sendable {
    public let url: URL
    public let icon: CatalogArtwork
    public let preview: CatalogArtwork
    public let manifest: SlopManifest
    public let fileBytes: Int64
    public let createdAt: Date?
    public let updatedAt: Date?
    public var id: String { "local:\(manifest.slug)" }
}

public struct LocalTemplateSnapshot: Sendable {
    public var templates: [LocalTemplate] = []
    public var issues: [String] = []
    public var diagnostics: [SlopFailureContext] = []
}

@MainActor public final class LocalTemplateStore: ObservableObject {
    @Published public private(set) var snapshot = LocalTemplateSnapshot()
    public var templates: [LocalTemplate] { snapshot.templates }
    public var issues: [String] { snapshot.issues }
    public let templatesURL: URL
    private let scan: @Sendable (URL) async throws -> LocalTemplateSnapshot
    private var watcher: DispatchSourceFileSystemObject?
    /// Folder events arrive in bursts while a template installs; one scan follows them.
    private var pendingScan: Task<Void, Never>?
    private var scanTask: Task<Void, Never>?
    private var generation = 0
    private var stopped = false

    public convenience init(templatesURL: URL = SlopTemplateLocation.templatesRoot) {
        let scanner = CatalogScanner()
        self.init(templatesURL: templatesURL, scan: { try await scanner.local(at: $0) })
    }

    init(templatesURL: URL, scan: @escaping @Sendable (URL) async throws -> LocalTemplateSnapshot) {
        self.templatesURL = templatesURL
        self.scan = scan
        startWatching()
        scheduleScan()
    }

    deinit { watcher?.cancel(); scanTask?.cancel(); pendingScan?.cancel() }

    /// Joins an existing scan instead of restarting the watcher or duplicating disk work.
    /// Unless `force`, a live folder watcher already covers changes and nothing is scanned.
    public func refresh(force: Bool = true) async {
        guard !stopped, force || watcher == nil || pendingScan != nil else { return }
        pendingScan?.cancel()
        pendingScan = nil
        if scanTask == nil { scheduleScan() }
        while let task = scanTask, !stopped {
            let requestedGeneration = generation
            await task.value
            if generation == requestedGeneration { return }
            // A filesystem event superseded the scan we joined; await its replacement.
        }
    }

    func stop() {
        stopped = true
        generation += 1
        pendingScan?.cancel()
        pendingScan = nil
        scanTask?.cancel()
        scanTask = nil
        watcher?.cancel()
        watcher = nil
    }

    private func scheduleScan() {
        guard !stopped else { return }
        generation += 1
        let requestedGeneration = generation
        scanTask?.cancel()
        scanTask = Task { [weak self, scan, templatesURL] in
            let result: LocalTemplateSnapshot
            do { result = try await scan(templatesURL) }
            catch is CancellationError { return }
            catch { result = LocalTemplateSnapshot(issues: [error.localizedDescription], diagnostics: [.classify(error)]) }
            guard !Task.isCancelled, let self, self.generation == requestedGeneration else { return }
            self.snapshot = result
            self.scanTask = nil
            // The first scan may have created a previously missing templates directory.
            self.startWatching()
        }
    }

    private func scheduleScanSoon() {
        pendingScan?.cancel()
        pendingScan = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled, let self else { return }
            self.pendingScan = nil
            self.scheduleScan()
        }
    }

    private func startWatching() {
        guard watcher == nil, !stopped else { return }
        let descriptor = open(templatesURL.path, O_EVTONLY)
        guard descriptor >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: descriptor, eventMask: [.write, .rename, .delete, .extend], queue: .main
        )
        source.setEventHandler { [weak self, weak source] in
            guard let self else { return }
            // A moved or deleted folder is no longer the templates folder. The scan
            // recreates it and watches the new one.
            if let source, !source.data.isDisjoint(with: [.delete, .rename]) {
                source.cancel()
                self.watcher = nil
            }
            self.scheduleScanSoon()
        }
        source.setCancelHandler { if descriptor >= 0 { close(descriptor) } }
        watcher = source
        source.resume()
    }
}

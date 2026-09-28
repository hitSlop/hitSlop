import Combine
import Darwin
import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopRuntime

public struct LocalTemplate: Identifiable, Sendable {
    public let packageURL: URL
    public let icon: CatalogArtwork
    public let preview: CatalogArtwork
    public let manifest: SlopManifest
    public let packageBytes: Int64
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
    private var scanTask: Task<Void, Never>?
    private var generation = 0
    private var stopped = false

    public convenience init(templatesURL: URL = DocumentFactory.defaultTemplatesRoot) {
        let scanner = CatalogScanner()
        self.init(templatesURL: templatesURL, scan: { try await scanner.local(at: $0) })
    }

    init(templatesURL: URL, scan: @escaping @Sendable (URL) async throws -> LocalTemplateSnapshot) {
        self.templatesURL = templatesURL
        self.scan = scan
        startWatching()
        scheduleScan()
    }

    deinit { watcher?.cancel(); scanTask?.cancel() }

    /// Joins an existing scan instead of restarting the watcher or duplicating disk work.
    public func refresh() async {
        guard !stopped else { return }
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

    private func startWatching() {
        guard watcher == nil, !stopped else { return }
        let descriptor = open(templatesURL.path, O_EVTONLY)
        guard descriptor >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: descriptor, eventMask: [.write, .rename, .delete, .extend], queue: .main
        )
        source.setEventHandler { [weak self] in self?.scheduleScan() }
        source.setCancelHandler { if descriptor >= 0 { close(descriptor) } }
        watcher = source
        source.resume()
    }
}

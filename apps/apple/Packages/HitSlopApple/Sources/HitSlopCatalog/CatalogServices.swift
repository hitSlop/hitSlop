import AppKit
import Combine
import ComposableArchitecture
import HitSlopCore
import HitSlopFeatures
import HitSlopHost
import HitSlopDocument

@MainActor final class CatalogServices {
    let templatesURL: URL
    private let telemetry: SlopTelemetry
    private let bundledRoot: URL?
    private let chooseDestination: (String) async -> URL?
    private let recordRecent: (URL) -> Void
    private let scanner = CatalogScanner()
    private weak var localStore: LocalTemplateStore?
    init(
        templatesURL: URL,
        bundledRoot: URL? = Bundle.main.resourceURL?.appendingPathComponent("StarterTemplates"),
        telemetry: SlopTelemetry = .disabled,
        chooseDestination: @escaping (String) async -> URL? = CatalogServices.chooseDestination,
        recordRecent: @escaping (URL) -> Void = { NSDocumentController.shared.noteNewRecentDocumentURL($0) }
    ) {
        self.templatesURL = templatesURL; self.bundledRoot = bundledRoot
        self.telemetry = telemetry; self.chooseDestination = chooseDestination; self.recordRecent = recordRecent
    }

    var client: CatalogClient {
        CatalogClient(
            local: { [self] in await local() },
            refreshLocal: { [self] force in await localStore?.refresh(force: force) },
            recents: { [self] in await recents() },
            recent: { [self] url in try? await scanner.recent(url) },
            chooseDestination: { [self] entry in
                do { return try await destination(for: entry) }
                catch { await telemetry.failure(.create, error: error); throw error }
            },
            create: { [self] entry, url in
                await telemetry.send(.breadcrumb(.create, .started))
                do { return try await create(entry, at: url) }
                catch { await telemetry.failure(.create, error: error); throw error }
            }
        )
    }

    private func local() async -> AsyncStream<CatalogSnapshot> {
        let bundled: LocalTemplateSnapshot
        if let bundledRoot, FileManager.default.fileExists(atPath: bundledRoot.path) {
            do { bundled = try await scanner.local(at: bundledRoot, makeImmutable: false) }
            catch { bundled = LocalTemplateSnapshot(issues: ["Could not load built-in templates: \(error.localizedDescription)"], diagnostics: [.classify(error)]) }
        } else { bundled = LocalTemplateSnapshot() }
        let store = LocalTemplateStore(templatesURL: templatesURL)
        localStore = store
        return AsyncStream(bufferingPolicy: .bufferingNewest(1)) { continuation in
            var reported = Set<Int>()
            let token = store.$snapshot.sink { [telemetry] snapshot in
                for diagnostic in bundled.diagnostics + snapshot.diagnostics {
                    if reported.insert(diagnostic.code(for: .catalog)).inserted { telemetry.send(.failed(.catalog, diagnostic)) }
                }
                let starters = bundled.templates.map { template in
                    var entry = Self.localEntry(template)
                    entry.isBundled = true
                    return entry
                }
                continuation.yield(CatalogSnapshot(entries: starters + snapshot.templates.map(Self.localEntry), issues: bundled.issues + snapshot.issues))
            }
            let lifetime = LocalSubscription(store: store, token: token)
            continuation.onTermination = { _ in Task { @MainActor in lifetime.cancel() } }
        }
    }

    func recents() async -> [CatalogEntry] {
        let urls = NSDocumentController.shared.recentDocumentURLs
        do { return try await scanner.recents(urls, templatesRoot: templatesURL) }
        catch { telemetry.failure(.catalog, error: error); return [] }
    }

    private func destination(for entry: CatalogEntry) async throws -> URL? {
        guard case .local = entry.source else { return nil }
        let destination = await chooseDestination(entry.slug)
        if destination == nil { telemetry.send(.breadcrumb(.create, .cancelled)) }
        return destination
    }

    private func create(_ entry: CatalogEntry, at url: URL) async throws -> URL {
        guard case .local(let source) = entry.source else { throw CocoaError(.fileNoSuchFile) }
        let package = try await SlopPreparation.run { try SlopDuplicator.duplicate(from: source, to: url, fromTemplate: true) }
        SlopPreviewWriter.installAuthoredIcon(for: package)
        telemetry.send(.breadcrumb(.create, .completed))
        telemetry.send(.created(entry.isBundled ? .bundled : .installed))
        recordRecent(url)
        return package.rootURL
    }

    private static func chooseDestination(_ slug: String) async -> URL? {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.slop]; panel.canCreateDirectories = true
        panel.startOnDesktop(); panel.nameFieldStringValue = "\(slug).slop"
        let response = await withCheckedContinuation { continuation in
            panel.begin { continuation.resume(returning: $0) }
        }
        return response == .OK ? panel.url : nil
    }

    static func localEntry(_ template: LocalTemplate) -> CatalogEntry {
        var entry = CatalogEntry(id: "local:\(template.packageURL.path)", source: .local(template.packageURL), title: template.manifest.title)
        apply(template.manifest, to: &entry)
        entry.icons = [template.icon, template.preview]
        entry.previews = [template.preview, template.icon]
        entry.packageBytes = template.packageBytes
        entry.createdAt = template.createdAt
        entry.updatedAt = template.updatedAt
        return entry
    }

    nonisolated static func apply(_ manifest: SlopManifest, to entry: inout CatalogEntry) {
        entry.slug = manifest.slug
        entry.description = manifest.description
        entry.categories = manifest.categories
        entry.authorName = manifest.author.name
        entry.authorURL = manifest.author.url.flatMap(URL.init(string:))
        entry.initialSize = "\(manifest.presentation.width) × \(manifest.presentation.height)"
    }
}

@MainActor private final class LocalSubscription {
    private var store: LocalTemplateStore?
    private var token: AnyCancellable?
    init(store: LocalTemplateStore, token: AnyCancellable) { self.store = store; self.token = token }
    func cancel() { token?.cancel(); token = nil; store?.stop(); store = nil }
}

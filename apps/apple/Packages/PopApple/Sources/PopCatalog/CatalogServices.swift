import AppKit
import PopCore
import PopDocument
import PopFeatures
import PopHost

@MainActor final class CatalogServices {
  /// The installed templates folder; nil lists only the bundled starters.
  let templatesURL: URL?
  private let telemetry: PopTelemetry
  private let bundledRoot: URL?
  private let chooseDestination: (String) async throws -> URL?
  private let systemRecents: @MainActor () -> [URL]
  private let history: RecentDocumentHistory
  private var importCandidates: [URL]?
  private(set) var recentsIssue: String?
  private var lastHistoryFailure: PopFailureContext?
  private let scanner = CatalogScanner()
  private weak var localStore: LocalTemplateStore?
  init(
    templatesURL: URL?,
    bundledRoot: URL? = PopTemplateLocation.bundledRoot,
    telemetry: PopTelemetry = .disabled,
    historyURL: URL? = RecentDocumentHistory.defaultURL,
    systemRecents: @escaping @MainActor () -> [URL] = CatalogServices.systemRecentDocuments,
    chooseDestination: @escaping (String) async throws -> URL? = CatalogServices.chooseDestination
  ) {
    self.templatesURL = templatesURL
    self.bundledRoot = bundledRoot
    self.telemetry = telemetry
    self.chooseDestination = chooseDestination
    self.systemRecents = systemRecents
    history = RecentDocumentHistory(fileURL: historyURL)
  }

  var client: CatalogClient {
    var client = CatalogClient(
      local: { [self] in await local() },
      refreshLocal: { [self] force in await localStore?.refresh(force: force) },
      recents: { [self] in await recents() },
      recent: { [self] url in
        let snapshot = await history.snapshot(importing: seedURLs())
        receive(snapshot)
        guard let document = snapshot.documents.first(where: { $0.url == url }) else { return nil }
        return await entry(document)
      },
      chooseDestination: { [self] entry in
        do { return try await destination(for: entry) } catch {
          telemetry.failure(.create, error: error)
          throw error
        }
      },
      create: { [self] entry, url in
        telemetry.send(.breadcrumb(.create, .started))
        do { return try await create(entry, at: url) } catch {
          telemetry.failure(.create, error: error)
          throw error
        }
      }
    )
    client.recentsIssue = { [self] in recentsIssue }
    client.resolveRecent = { [self] entry in
      guard case .recent(let url) = entry.source else { throw CocoaError(.fileNoSuchFile) }
      guard let id = entry.recentHistoryID else { return url }
      return try await history.resolve(id)
    }
    return client
  }

  private func local() async -> AsyncStream<CatalogSnapshot> {
    let bundled: LocalTemplateSnapshot
    if let bundledRoot, FileManager.default.fileExists(atPath: bundledRoot.path) {
      do { bundled = try await scanner.local(at: bundledRoot) } catch {
        bundled = LocalTemplateSnapshot(
          issues: ["Could not load built-in templates: \(error.localizedDescription)"], diagnostics: [.classify(error)])
      }
    } else {
      bundled = LocalTemplateSnapshot()
    }
    let store = templatesURL.map { LocalTemplateStore(templatesURL: $0) }
    localStore = store
    let installed = store?.snapshots ?? AsyncStream { $0.yield(LocalTemplateSnapshot()) }
    let starters = bundled.templates.map { template in
      var entry = template
      entry.isBundled = true
      return entry
    }
    return AsyncStream(bufferingPolicy: .bufferingNewest(1)) { continuation in
      let forwarding = Task { @MainActor [telemetry] in
        var reported = Set<Int>()
        for await snapshot in installed {
          for diagnostic in bundled.diagnostics + snapshot.diagnostics {
            if reported.insert(diagnostic.code(for: .catalog)).inserted {
              telemetry.send(.failed(.catalog, diagnostic))
            }
          }
          continuation.yield(
            CatalogSnapshot(entries: starters + snapshot.templates, issues: bundled.issues + snapshot.issues))
        }
      }
      continuation.onTermination = { _ in
        forwarding.cancel()
        Task { @MainActor in store?.stop() }
      }
    }
  }

  func recents() async -> [CatalogEntry] {
    let snapshot = await history.snapshot(importing: seedURLs())
    receive(snapshot)
    var entries: [CatalogEntry] = []
    for document in snapshot.documents {
      entries.append(await entry(document))
    }
    return entries
  }

  private func entry(_ document: RecentDocumentHistory.Document) async -> CatalogEntry {
    let scanned = document.isUnavailable ? nil : try? await scanner.recent(document.url)
    var entry =
      scanned
      ?? CatalogEntry(
        id: "recent:\(document.id)", source: .recent(document.url),
        title: document.url.deletingPathExtension().lastPathComponent)
    entry.id = "recent:\(document.id)"
    entry.recentHistoryID = document.id
    entry.isUnavailable = document.isUnavailable || scanned == nil
    return entry
  }

  private func seedURLs() -> [URL] {
    if let importCandidates { return importCandidates }
    let urls = systemRecents()
    importCandidates = urls
    return urls
  }

  func recordOpened(_ url: URL) async {
    receive(await history.recordOpened(url, importing: seedURLs()))
  }

  func clearRecents() async { receive(await history.clear()) }

  private func receive(_ snapshot: RecentDocumentHistory.Snapshot) {
    recentsIssue =
      snapshot.failure == nil
      ? nil
      : "Could not save or load recent documents. Your previous saved history has been kept."
    if let failure = snapshot.failure, failure != lastHistoryFailure {
      telemetry.send(.failed(.catalog, failure))
    }
    lastHistoryFailure = snapshot.failure
  }

  static func systemRecentDocuments() -> [URL] {
    guard RecentDocumentHistory.usesSystemRecents else { return [] }
    #if DEBUG
      // Debug has its own history and must not seed it with the release app's list.
      return []
    #else
      return NSDocumentController.shared.recentDocumentURLs
    #endif
  }

  private func destination(for entry: CatalogEntry) async throws -> URL? {
    let destination = try await chooseDestination(entry.slug)
    if destination == nil { telemetry.send(.breadcrumb(.create, .cancelled)) }
    return destination
  }

  private func create(_ entry: CatalogEntry, at url: URL) async throws -> URL {
    guard case .local(let source) = entry.source else { throw CocoaError(.fileNoSuchFile) }
    let created = try await PopPreparation.run { try PopFile.create(from: source, to: url) }
    PopSaveLocation.remember(created)
    telemetry.send(.breadcrumb(.create, .completed))
    telemetry.send(.created(entry.isBundled ? .bundled : .installed))
    return created
  }

  private static func chooseDestination(_ slug: String) async throws -> URL? {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.pop]
    panel.canCreateDirectories = true
    try panel.startInLocalDocuments()
    panel.nameFieldStringValue = "\(slug).pop"
    return await PopPanels.select(panel)
  }

}

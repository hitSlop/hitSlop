import AppKit
import HitSlopCore
import HitSlopDocument
import HitSlopFeatures
import HitSlopHost

@MainActor final class CatalogServices {
  /// The installed templates folder; nil lists only the bundled starters.
  let templatesURL: URL?
  private let telemetry: SlopTelemetry
  private let bundledRoot: URL?
  private let chooseDestination: (String) async -> URL?
  private let scanner = CatalogScanner()
  private weak var localStore: LocalTemplateStore?
  init(
    templatesURL: URL?,
    bundledRoot: URL? = SlopTemplateLocation.bundledRoot,
    telemetry: SlopTelemetry = .disabled,
    chooseDestination: @escaping (String) async -> URL? = CatalogServices.chooseDestination
  ) {
    self.templatesURL = templatesURL
    self.bundledRoot = bundledRoot
    self.telemetry = telemetry
    self.chooseDestination = chooseDestination
  }

  var client: CatalogClient {
    CatalogClient(
      local: { [self] in await local() },
      refreshLocal: { [self] force in await localStore?.refresh(force: force) },
      recents: { [self] in await recents() },
      recent: { [self] url in try? await scanner.recent(url) },
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
    let urls = NSDocumentController.shared.recentDocumentURLs
    do { return try await scanner.recents(urls) } catch {
      telemetry.failure(.catalog, error: error)
      return []
    }
  }

  private func destination(for entry: CatalogEntry) async throws -> URL? {
    let destination = await chooseDestination(entry.slug)
    if destination == nil { telemetry.send(.breadcrumb(.create, .cancelled)) }
    return destination
  }

  private func create(_ entry: CatalogEntry, at url: URL) async throws -> URL {
    guard case .local(let source) = entry.source else { throw CocoaError(.fileNoSuchFile) }
    let created = try await SlopPreparation.run { try SlopFile.create(from: source, to: url) }
    telemetry.send(.breadcrumb(.create, .completed))
    telemetry.send(.created(entry.isBundled ? .bundled : .installed))
    return created
  }

  private static func chooseDestination(_ slug: String) async -> URL? {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.slop]
    panel.canCreateDirectories = true
    panel.startOnDesktop()
    panel.nameFieldStringValue = "\(slug).slop"
    return await panel.begin() == .OK ? panel.url : nil
  }

}

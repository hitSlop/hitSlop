import Foundation
import HitSlopCore
import HitSlopDocument
import HitSlopFeatures

/// Catalog file work runs on its own queue, off the main actor and away from document
/// opening. Installed templates are validated once per version; recent documents are
/// listed from their checked apps.
actor CatalogScanner {
  /// A template file as last validated. Installs replace the whole file, so its identity,
  /// date or size changes with every new version.
  private struct Version: Equatable, Sendable {
    let identifier: UInt64?
    let modified: Date?
    let size: Int?
    init(_ url: URL) {
      let values = try? url.resourceValues(forKeys: [.fileIdentifierKey, .contentModificationDateKey, .fileSizeKey])
      identifier = values?.fileIdentifier
      modified = values?.contentModificationDate
      size = values?.fileSize
    }
  }
  private enum Outcome: Sendable {
    case template(CatalogEntry)
    case issue(String, SlopFailureContext)
  }
  /// Per templates folder, each template file's last validation.
  private var validated: [URL: [URL: (version: Version, outcome: Outcome)]] = [:]

  func local(at root: URL) async throws -> LocalTemplateSnapshot {
    try Task.checkCancellation()
    let children = try await SlopPreparation.run(on: SlopPreparation.catalog) {
      // The installed templates folder may not exist yet.
      try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
      return try FileManager.default.contentsOfDirectory(
        at: root, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]
      ).filter { $0.pathExtension == "slop" }
        .sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
    }
    var result = LocalTemplateSnapshot()
    var seen: [URL: (version: Version, outcome: Outcome)] = [:]
    for child in children {
      try Task.checkCancellation()
      let version = Version(child)
      let outcome: Outcome
      if let cached = validated[root]?[child], cached.version == version {
        outcome = cached.outcome
      } else {
        outcome = try await validate(child)
      }
      seen[child] = (version, outcome)
      switch outcome {
      case .template(let template): result.templates.append(template)
      case .issue(let issue, let diagnostic):
        result.issues.append(issue)
        if !result.diagnostics.contains(diagnostic) { result.diagnostics.append(diagnostic) }
      }
    }
    // Removed templates leave the cache.
    validated[root] = seen
    return result
  }

  private func validate(_ child: URL) async throws -> Outcome {
    do {
      let template = try await SlopPreparation.run(on: SlopPreparation.catalog) {
        Self.entry(template: try SlopFile(template: child))
      }
      return .template(template)
    } catch is CancellationError {
      throw CancellationError()
    } catch {
      return .issue("\(child.lastPathComponent): \(error.localizedDescription)", .classify(error))
    }
  }

  func recents(_ urls: [URL]) async throws -> [CatalogEntry] {
    try Task.checkCancellation()
    var seen = Set<URL>()
    var entries: [CatalogEntry] = []
    for original in urls {
      try Task.checkCancellation()
      guard let url = try? SlopFile.resolvedRoot(original), seen.insert(url).inserted,
        let entry = try await recent(url)
      else { continue }
      entries.append(entry)
    }
    return entries
  }

  /// A recent document's entry, or nil once it no longer exists.
  func recent(_ url: URL) async throws -> CatalogEntry? {
    try await SlopPreparation.run(on: SlopPreparation.catalog) {
      guard FileManager.default.fileExists(atPath: url.path) else { return nil }
      guard let file = try? SlopFile(url: url) else {
        var entry = CatalogEntry(
          id: "recent:\(url.path)", source: .recent(url), title: url.deletingPathExtension().lastPathComponent)
        Self.dates(url, &entry)
        return entry
      }
      return Self.entry(file, source: .recent(url), id: "recent:\(url.path)")
    }
  }

  /// A checked template's catalog entry, keyed by its path.
  static func entry(template file: SlopFile) -> CatalogEntry {
    entry(file, source: .local(file.url), id: "local:\(file.url.path)")
  }
  private static func entry(_ file: SlopFile, source: CatalogEntry.Source, id: String) -> CatalogEntry {
    let manifest = file.manifest
    var entry = CatalogEntry(id: id, source: source, title: manifest.title)
    entry.slug = manifest.slug
    entry.description = manifest.description
    entry.categories = manifest.categories
    entry.authorName = manifest.author.name
    entry.authorURL = manifest.author.url.flatMap(URL.init(string:))
    entry.initialSize = "\(manifest.presentation.width) × \(manifest.presentation.height)"
    let icon = artwork(file.url, .icon)
    let preview = artwork(file.url, .preview)
    entry.icons = [icon, preview]
    entry.previews = [preview, icon]
    entry.fileBytes = file.byteCount
    dates(file.url, &entry)
    return entry
  }
  private static func dates(_ url: URL, _ entry: inout CatalogEntry) {
    let values = try? url.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
    entry.createdAt = values?.creationDate
    entry.updatedAt = values?.contentModificationDate
  }

  private static func artwork(_ file: URL, _ name: SlopArtwork.Name) -> CatalogArtwork {
    let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
    return CatalogArtwork(
      file: file, name: name, modifiedAt: values?.contentModificationDate, byteCount: values?.fileSize)
  }
}

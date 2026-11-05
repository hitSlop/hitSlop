import Foundation
import PopCore
import PopCoreBinding
import PopDocument
import PopFeatures

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
    case issue(String, PopFailureContext)
  }
  /// Per templates folder, each template file's last validation.
  private var validated: [URL: [URL: (version: Version, outcome: Outcome)]] = [:]

  func local(at root: URL) async throws -> LocalTemplateSnapshot {
    try Task.checkCancellation()
    let children = try await PopPreparation.run(on: PopPreparation.catalog) {
      // The installed templates folder may not exist yet.
      try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
      return try FileManager.default.contentsOfDirectory(
        at: root, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]
      ).filter { $0.pathExtension == "pop" }
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
      let template = try await PopPreparation.run(on: PopPreparation.catalog) {
        Self.entry(template: try PopSummary(url: child, template: true))
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
      guard let url = try? PopFile.resolvedRoot(original), seen.insert(url).inserted,
        let entry = try await recent(url)
      else { continue }
      entries.append(entry)
    }
    return entries
  }

  /// A recent document's entry, or nil once it no longer exists.
  func recent(_ url: URL) async throws -> CatalogEntry? {
    try await PopPreparation.run(on: PopPreparation.catalog) {
      guard FileManager.default.fileExists(atPath: url.path) else { return nil }
      guard let file = try? PopSummary(url: url) else {
        var entry = CatalogEntry(
          id: "recent:\(url.path)", source: .recent(url), title: url.deletingPathExtension().lastPathComponent)
        Self.dates(url, &entry)
        return entry
      }
      var entry = Self.entry(file, source: .recent(url), id: "recent:\(url.path)")
      entry.savedDocumentID = try? sharingLocalDocument(path: url.path)
      return entry
    }
  }

  /// A checked template's catalog entry, keyed by its path.
  static func entry(template file: PopSummary) -> CatalogEntry {
    entry(file, source: .local(file.url), id: "local:\(file.url.path)")
  }
  private static func entry(_ file: PopSummary, source: CatalogEntry.Source, id: String) -> CatalogEntry {
    let metadata = file.metadata
    var entry = CatalogEntry(id: id, source: source, title: metadata.title)
    entry.slug = metadata.slug
    entry.description = metadata.description
    entry.categories = metadata.categories
    entry.authorName = metadata.author.name
    entry.authorURL = metadata.author.url.flatMap(URL.init(string:))
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

  private static func artwork(_ file: URL, _ name: PopArtwork.Name) -> CatalogArtwork {
    let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
    return CatalogArtwork(
      file: file, name: name, modifiedAt: values?.contentModificationDate, byteCount: values?.fileSize)
  }
}

import Foundation
import HitSlopCore
import Observation

extension SlopCategory {
  public var label: String { self == .developerTools ? "Developer Tools" : rawValue.capitalized }
  public var emoji: String {
    switch self {
    case .popular: "🔥"
    case .productivity: "⚡️"
    case .utilities: "🪄"
    case .finance: "🤑"
    case .media: "🎬"
    case .games: "🎮"
    case .developerTools: "👾"
    case .education: "🎓"
    case .business: "📊"
    case .personal: "💖"
    case .health: "🫀"
    case .creative: "🎨"
    case .music: "🎵"
    case .other: "🎲"
    }
  }
}

public enum CatalogFilter: Hashable, Sendable {
  case all, recents, synced
  case category(SlopCategory)
  public var title: String {
    switch self {
    case .all: "Templates"
    case .recents: "Recents"
    case .synced: "Synced"
    case .category(let category): category.label
    }
  }
}

/// A slop's preview or icon artwork: the file holding it and which one. The file's
/// modification date and size participate in view reloads and decoded-image caching.
public struct CatalogArtwork: Hashable, Sendable {
  public let file: URL
  public let name: SlopArtwork.Name
  public let modifiedAt: Date?
  public let byteCount: Int?
  public let revision: String?
  public init(file: URL, name: SlopArtwork.Name, modifiedAt: Date?, byteCount: Int?, revision: String? = nil) {
    self.file = file
    self.name = name
    self.modifiedAt = modifiedAt
    self.byteCount = byteCount
    self.revision = revision
  }
}

/// Display and creation metadata only; no file reads occur when rendering a view.
public struct CatalogEntry: Equatable, Identifiable, Sendable {
  public enum Source: Equatable, Sendable {
    case local(URL)
    case recent(URL)
  }
  public var id: String
  public var source: Source
  public var title: String
  public var slug = ""
  public var isBundled = false
  public var isSynced = false
  /// A synced entry kept in iCloud, not one kept only on this device.
  public var inCloud = false
  public var description = ""
  public var categories: [SlopCategory] = []
  public var authorName: String?
  public var authorURL: URL?
  public var icons: [CatalogArtwork] = []
  public var previews: [CatalogArtwork] = []
  public var fileBytes: Int64 = 0
  public var createdAt: Date?
  public var updatedAt: Date?
  public var initialSize: String?
  public init(id: String, source: Source, title: String) {
    self.id = id
    self.source = source
    self.title = title
  }
  public var isRecent: Bool { if case .recent = source { true } else { false } }
  public var documentIdentity: SlopDocumentIdentity? {
    guard case .recent(let url) = source else { return nil }
    return SlopDocumentIdentity(url: url)
  }
  public var displayTitle: String { isSynced ? title : (documentIdentity?.filename ?? title) }
  public var searchableText: String {
    ([title, description, authorName ?? "", documentIdentity?.path ?? "", documentIdentity?.folderPath ?? ""]
      + categories.flatMap { [$0.rawValue, $0.label] })
      .joined(separator: " ").localizedLowercase
  }
}

public struct CatalogSnapshot: Equatable, Sendable {
  public var entries: [CatalogEntry]
  public var issues: [String]
  public init(entries: [CatalogEntry] = [], issues: [String] = []) {
    self.entries = entries
    self.issues = issues
  }
}

/// How the catalog reaches templates, recent documents and the file system.
public struct CatalogClient {
  /// The installed and bundled templates: the current listing, then each change.
  public var local: @MainActor () async -> AsyncStream<CatalogSnapshot>
  /// Rescans installed templates; unless `force`, only when no folder watcher sees changes.
  public var refreshLocal: @MainActor (_ force: Bool) async -> Void
  public var recents: @MainActor () async -> [CatalogEntry]
  /// The recent document at a URL, read again after its artwork changed.
  public var recent: @MainActor (URL) async -> CatalogEntry?
  /// Returns nil when the destination picker is cancelled.
  public var chooseDestination: @MainActor (CatalogEntry) async throws -> URL?
  /// Copies the template to the chosen destination and returns the new document.
  public var create: @MainActor (CatalogEntry, URL) async throws -> URL

  public init(
    local: @escaping @MainActor () async -> AsyncStream<CatalogSnapshot>,
    refreshLocal: @escaping @MainActor (_ force: Bool) async -> Void,
    recents: @escaping @MainActor () async -> [CatalogEntry],
    recent: @escaping @MainActor (URL) async -> CatalogEntry?,
    chooseDestination: @escaping @MainActor (CatalogEntry) async throws -> URL?,
    create: @escaping @MainActor (CatalogEntry, URL) async throws -> URL
  ) {
    self.local = local
    self.refreshLocal = refreshLocal
    self.recents = recents
    self.recent = recent
    self.chooseDestination = chooseDestination
    self.create = create
  }

  /// No templates, no recents and no destinations: for windows that show no catalog.
  public static var empty: Self {
    Self(
      local: { AsyncStream { $0.finish() } }, refreshLocal: { _ in }, recents: { [] }, recent: { _ in nil },
      chooseDestination: { _ in nil }, create: { _, url in url })
  }
}

/// The catalog's storage question can create a library copy or continue to the local picker.
public enum CatalogStorageChoice: Sendable {
  case local, cancelled
  case created(URL)
}

/// The catalog window's state: templates and recent documents, the search, filter and
/// selection over them, and creating a document from a template.
@MainActor @Observable public final class CatalogModel {
  public var query = "" { didSet { synchronizeSelection() } }
  public private(set) var filter: CatalogFilter = .category(.popular)
  public var selectedID: String?
  public private(set) var local: [CatalogEntry] = []
  private var recentFiles: [CatalogEntry] = []
  /// The managed library's folder. Files inside it appear in Recents only as their synced
  /// entries; anything else there (another account's or an earlier preview's library) is left out.
  public var managedFolder: String?
  /// Recent documents, newest first; a library document shows as its synced entry.
  public var recents: [CatalogEntry] {
    let synced = Dictionary(
      self.synced.compactMap { entry -> (String, CatalogEntry)? in
        guard case .recent(let url) = entry.source else { return nil }
        return (url.standardizedFileURL.path, entry)
      }, uniquingKeysWith: { first, _ in first })
    return recentFiles.compactMap { recent in
      guard case .recent(let url) = recent.source else { return recent }
      let path = url.standardizedFileURL.path
      if var entry = synced[path] {
        entry.createdAt = recent.createdAt
        entry.updatedAt = recent.updatedAt
        return entry
      }
      if let managedFolder {
        let folder = URL(fileURLWithPath: managedFolder).standardizedFileURL.path
        // Earlier previews left sibling Library-development-… folders. Other siblings are local files.
        if path.hasPrefix(folder + "/") || path.hasPrefix(folder + "-development-") { return nil }
      }
      return recent
    }
  }
  public private(set) var localIssues: [String] = []
  /// The template being created, from choosing its destination until its copy finishes.
  public private(set) var creating: CatalogEntry?
  /// True only while the template is copied, never while the save panel is open.
  public private(set) var isCopying = false
  /// Quit began: nothing is created or opened. The app sets it.
  public internal(set) var isQuitting = false
  /// Why the last creation failed, until the person acknowledges it.
  public var creationError: String?
  @ObservationIgnored public var chooseStorage: @MainActor (CatalogEntry) async throws -> CatalogStorageChoice = { _ in
    .local
  }

  @ObservationIgnored private let client: CatalogClient
  @ObservationIgnored private var hasSelectedFilter = false
  @ObservationIgnored private var localTask: Task<Void, Never>?
  @ObservationIgnored private var recentsTask: Task<Void, Never>?
  @ObservationIgnored private var refreshTask: Task<Void, Never>?
  @ObservationIgnored let work = TaskSet()
  /// A recent document was chosen.
  @ObservationIgnored var onOpen: @MainActor (URL) -> Void = { _ in }
  /// A creation ended: the new document, or nil when it was cancelled or failed.
  @ObservationIgnored var onCreationEnded: @MainActor (URL?) -> Void = { _ in }

  public init(client: CatalogClient) { self.client = client }

  public var synced: [CatalogEntry] = [] { didSet { synchronizeSelection() } }

  public var categories: [SlopCategory] {
    let present = Set(local.flatMap(\.categories))
    return SlopCategory.allCases.filter { present.contains($0) }
  }
  public var visibleEntries: [CatalogEntry] {
    let items: [CatalogEntry]
    switch filter {
    case .all: items = local
    case .category(let category): items = local.filter { $0.categories.contains(category) }
    case .recents: items = recents
    case .synced: items = synced
    }
    let search = query.trimmingCharacters(in: .whitespacesAndNewlines).localizedLowercase
    return search.isEmpty ? items : items.filter { $0.searchableText.contains(search) }
  }
  public var selectedEntry: CatalogEntry? { visibleEntries.first { $0.id == selectedID } }

  /// Starts listing templates and recent documents; later calls do nothing.
  public func start() {
    guard localTask == nil else { return }
    localTask = Task { [weak self, client] in
      for await snapshot in await client.local() { self?.receive(snapshot) }
    }
    refreshRecents()
  }

  /// Rescans everything.
  public func refreshSources() { refresh(force: true) }
  /// The app became active: rescans what no folder watcher covers.
  public func activated() { refresh(force: false) }

  /// Lists recent documents again; a listing still in flight is dropped.
  public func refreshRecents() {
    recentsTask?.cancel()
    recentsTask = work.run {
      let entries = await self.client.recents()
      guard !Task.isCancelled else { return }
      self.recentFiles = entries
      self.synchronizeSelection()
    }
  }

  /// A document's artwork changed: only its entry is read again.
  public func artworkChanged(_ url: URL) {
    guard localTask != nil else { return }
    guard recentFiles.contains(where: { $0.source == .recent(url) }) else { return refreshRecents() }
    work.run {
      let entry = await self.client.recent(url)
      guard let index = self.recentFiles.firstIndex(where: { $0.source == .recent(url) }) else { return }
      if let entry { self.recentFiles[index] = entry } else { self.recentFiles.remove(at: index) }
      self.synchronizeSelection()
    }
  }

  public func select(_ filter: CatalogFilter) {
    hasSelectedFilter = true
    guard self.filter != filter else { return }
    self.filter = filter
    selectedID = nil
    synchronizeSelection()
  }

  /// Opens a recent document, or creates a document from a template where the person
  /// chooses. One creation at a time; nothing while quitting.
  public func primaryAction(_ entry: CatalogEntry) {
    guard !isQuitting, creating == nil else { return }
    if case .recent(let url) = entry.source { return onOpen(url) }
    creating = entry
    work.run { await self.create(entry) }
  }

  private func create(_ entry: CatalogEntry) async {
    var created: URL?
    do {
      switch try await chooseStorage(entry) {
      case .created(let url): created = url
      case .cancelled: break
      case .local:
        if let destination = try await client.chooseDestination(entry) {
          isCopying = true
          created = try await client.create(entry, destination)
        }
      }
      refreshRecents()
    } catch {
      creationError = error.localizedDescription
    }
    creating = nil
    isCopying = false
    onCreationEnded(created)
  }

  private func refresh(force: Bool) {
    guard localTask != nil else { return }
    refreshRecents()
    refreshTask?.cancel()
    refreshTask = work.run { await self.client.refreshLocal(force) }
  }

  private func receive(_ snapshot: CatalogSnapshot) {
    local = snapshot.entries
    localIssues = snapshot.issues
    if !hasSelectedFilter {
      filter = categories.contains(.popular) ? .category(.popular) : .all
    } else if case .category(let category) = filter, !categories.contains(category) {
      filter = .all
    }
    synchronizeSelection()
  }

  private func synchronizeSelection() {
    if let selectedID, visibleEntries.contains(where: { $0.id == selectedID }) { return }
    selectedID = visibleEntries.first?.id
  }
}

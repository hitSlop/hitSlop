import Foundation
import PopCore

/// Mac-local navigation metadata, independent of document storage and the widget cache.
/// All filesystem work and mutations share one serial queue, including reads and retries.
struct RecentDocumentHistory: Sendable {
  struct Document: Equatable, Identifiable, Sendable {
    let id: UUID
    let url: URL
    let openedAt: Date
    let isUnavailable: Bool
  }

  struct Snapshot: Sendable {
    var documents: [Document]
    var failure: PopFailureContext?
  }

  static func location(in support: URL, development: Bool) -> URL {
    support.appendingPathComponent("pop/Library/Recents", isDirectory: true)
      .appendingPathComponent(development ? "Debug.json" : "Release.json")
  }

  static var defaultURL: URL {
    if let path = ProcessInfo.processInfo.environment["POP_TEST_RECENTS_FILE"] {
      return URL(fileURLWithPath: path)
    }
    #if DEBUG
      let development = true
    #else
      let development = false
    #endif
    return location(
      in: FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0],
      development: development)
  }

  static var usesSystemRecents: Bool { ProcessInfo.processInfo.environment["POP_TEST_RECENTS_FILE"] == nil }

  private let queue = DispatchQueue(label: "click.dotpop.recents", qos: .utility)
  private let storage: Storage

  /// A nil location is an isolated in-memory history, used by hidden native test hosts.
  init(fileURL: URL?) { storage = Storage(fileURL: fileURL) }

  func snapshot(importing urls: [URL] = []) async -> Snapshot {
    await run { $0.update(importing: urls) }
  }

  func recordOpened(_ url: URL, at date: Date = .now, importing urls: [URL] = []) async -> Snapshot {
    await run { $0.update(importing: urls, opened: (url, date)) }
  }

  func clear() async -> Snapshot { await run { $0.update(clear: true) } }

  /// Forgets one document; its file is untouched.
  func remove(_ id: UUID) async -> Snapshot { await run { $0.update(removing: id) } }

  func resolve(_ id: UUID) async throws -> URL {
    let storage = storage
    return try await withCheckedThrowingContinuation { continuation in
      queue.async { continuation.resume(with: Result { try storage.resolve(id) }) }
    }
  }

  private func run(_ action: @escaping @Sendable (Storage) -> Snapshot) async -> Snapshot {
    let storage = storage
    // An accepted history change completes even if its requesting catalog refresh was
    // cancelled. The model separately fences publication of superseded snapshots.
    return await withCheckedContinuation { continuation in
      queue.async { continuation.resume(returning: action(storage)) }
    }
  }

  /// Accessed exclusively on this history's queue.
  private final class Storage: @unchecked Sendable {
    private struct Record: Codable {
      var id: UUID
      var bookmark: Data
      var url: URL
      var openedAt: Date
    }
    private struct Saved: Codable {
      var version = 1
      var documents: [Record]
    }
    private let fileURL: URL?
    private var records: [Record] = []
    private var loaded = false
    private var dirty = false

    init(fileURL: URL?) {
      self.fileURL = fileURL
      loaded = fileURL == nil
    }

    func update(
      importing urls: [URL] = [], opened: (URL, Date)? = nil, removing: UUID? = nil, clear: Bool = false
    ) -> Snapshot {
      var failure: PopFailureContext?
      if clear {
        // Explicit clearing may replace an unreadable history. An ordinary refresh or
        // opening never does, and clearing persists initialization even when empty.
        records = []
        loaded = true
        dirty = true
      } else {
        do { try load(importing: urls) } catch { failure = .classify(error) }
      }
      if let removing, let index = records.firstIndex(where: { $0.id == removing }) {
        records.remove(at: index)
        dirty = true
      }
      if let (url, date) = opened {
        do { try remember(url, at: date) } catch { failure = .classify(error) }
      }
      let documents = records.indices.map { index -> Document in
        do {
          let (url, stale) = try resolution(records[index])
          if url != records[index].url {
            records[index].url = url
            dirty = true
          }
          // Recreate stale bookmarks, including after a rename, without changing recency.
          if stale {
            do {
              records[index].bookmark = try bookmark(url)
              dirty = true
            } catch { failure = .classify(error) }
          }
          return Document(id: records[index].id, url: url, openedAt: records[index].openedAt, isUnavailable: false)
        } catch {
          return Document(
            id: records[index].id, url: records[index].url, openedAt: records[index].openedAt, isUnavailable: true)
        }
      }
      // If loading failed, keep any newly opened documents in memory, but never replace
      // the unreadable saved history. A retry loads and merges them before writing.
      if loaded, dirty {
        do { try save() } catch { failure = .classify(error) }
      }
      return Snapshot(documents: documents, failure: failure)
    }

    func resolve(_ id: UUID) throws -> URL {
      guard let record = records.first(where: { $0.id == id }) else { throw CocoaError(.fileNoSuchFile) }
      return try resolved(record)
    }

    private func load(importing urls: [URL]) throws {
      guard !loaded, let fileURL else { return }
      let saved: Saved
      do {
        let data = try Data(contentsOf: fileURL)
        saved = try JSONDecoder().decode(Saved.self, from: data)
        guard saved.version == 1,
          Set(saved.documents.map(\.id)).count == saved.documents.count,
          saved.documents.allSatisfy({ $0.url.isFileURL && $0.url.pathExtension == "pop" })
        else { throw CocoaError(.fileReadCorruptFile) }
      } catch let error as CocoaError where error.code == .fileReadNoSuchFile {
        // Only the absence of our file permits importing the system list.
        for (index, original) in urls.enumerated().reversed() {
          guard let url = try? PopFile.resolvedRoot(original),
            (try? PopFile.kind(of: url)) == .document
          else { continue }
          try? remember(url, at: Date().addingTimeInterval(-Double(index)))
        }
        loaded = true
        dirty = true
        return
      }
      // Pending openings after a read failure take precedence over saved references.
      var seen = Set<UUID>()
      var paths = Set<String>()
      records = (records + saved.documents).filter {
        seen.insert($0.id).inserted && paths.insert(((try? resolved($0)) ?? $0.url).path).inserted
      }.sorted { $0.openedAt > $1.openedAt }
      loaded = true
    }

    private func remember(_ original: URL, at date: Date) throws {
      let url = try PopFile.resolvedRoot(original)
      let data = try bookmark(url)
      let index = records.firstIndex { ((try? resolved($0)) ?? $0.url) == url }
      let id = index.map { records[$0].id } ?? UUID()
      if let index { records.remove(at: index) }
      records.insert(Record(id: id, bookmark: data, url: url, openedAt: date), at: 0)
      dirty = true
    }

    private func resolved(_ record: Record) throws -> URL {
      try resolution(record).url
    }

    private func resolution(_ record: Record) throws -> (url: URL, stale: Bool) {
      var stale = false
      let url = try URL(
        resolvingBookmarkData: record.bookmark, options: [.withoutUI, .withoutMounting],
        bookmarkDataIsStale: &stale)
      return (try PopFile.resolvedRoot(url), stale)
    }

    private func bookmark(_ url: URL) throws -> Data {
      try url.bookmarkData(options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil)
    }

    private func save() throws {
      if let fileURL {
        let data = try JSONEncoder().encode(Saved(documents: records))
        try FileManager.default.createDirectory(
          at: fileURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: fileURL, options: .atomic)
      }
      dirty = false
    }
  }
}

import Foundation
import ImageIO

/// Image output only. The extension never opens a document or participates in its owner.
public struct WidgetCache: Sendable {
  public static let kind = "HitSlopDocumentWidget"
  #if os(macOS)
    public static let group = "78UAXU8QG8.com.hitslop.widgets"
  #else
    public static let group = "group.com.hitslop"
  #endif
  public let root: URL

  public init(root: URL) { self.root = root }
  public static func shared() throws -> WidgetCache {
    guard let group = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group) else {
      throw CocoaError(.fileNoSuchFile)
    }
    return WidgetCache(root: group.appendingPathComponent("widgets-v1", isDirectory: true))
  }
  public func records() -> [WidgetRecord] {
    let directories = (try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)) ?? []
    return directories.compactMap { record(id: $0.lastPathComponent) }
      .sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
  }
  public func record(id: String) -> WidgetRecord? {
    guard UUID(uuidString: id) != nil else { return nil }
    for _ in 0..<2 {
      guard let data = try? Data(contentsOf: folder(id).appendingPathComponent("current.json")), data.count <= 16384,
        let value = try? JSONDecoder().decode(WidgetRecord.self, from: data), value.isValid, value.id == id
      else { return nil }
      if WidgetImageFamily.allCases.allSatisfy({ FileManager.default.fileExists(atPath: imageURL(value, $0).path) }) {
        return value
      }
      // A concurrent publication may have retired the pointer we just read.
    }
    return nil
  }

  public func imageURL(_ record: WidgetRecord, _ family: WidgetImageFamily) -> URL {
    folder(record.id).appendingPathComponent(record.generation).appendingPathComponent("\(family.filename).png")
  }
  public static func openURL(id: String) -> URL {
    URL(string: "tinys://widget/\(id)")!
  }
  public static func selectionID(from url: URL) -> String? {
    guard url.scheme == "tinys", url.host == "widget", url.query == nil, url.fragment == nil,
      url.pathComponents.count == 2, let id = url.pathComponents.last, UUID(uuidString: id) != nil
    else { return nil }
    return id
  }
  private func folder(_ id: String) -> URL { root.appendingPathComponent(id, isDirectory: true) }

  #if os(macOS)
    /// The local file identity survives a rename, but a copied/replaced file is a new choice.
    public static func identity(_ url: URL) throws -> String {
      let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
      guard attrs[.type] as? FileAttributeType == .typeRegular,
        let device = attrs[.systemNumber] as? NSNumber, let inode = attrs[.systemFileNumber] as? NSNumber
      else { throw CocoaError(.fileReadInvalidFileName) }
      return "\(device):\(inode)"
    }
    public func resolve(_ record: WidgetRecord) throws -> URL {
      let data = try Data(contentsOf: folder(record.id).appendingPathComponent("document.bookmark"))
      var stale = false
      let url = try URL(
        resolvingBookmarkData: data, options: [.withoutUI, .withoutMounting], bookmarkDataIsStale: &stale)
      guard try Self.identity(url) == record.fileIdentity else { throw CocoaError(.fileNoSuchFile) }
      return url
    }
    public func record(for url: URL) -> WidgetRecord? {
      guard let identity = try? Self.identity(url) else { return nil }
      // Resolving the bookmark also prevents an inode reused by a different file from matching.
      let canonical = url.resolvingSymlinksInPath().standardizedFileURL
      return records().first {
        $0.fileIdentity == identity && (try? resolve($0).resolvingSymlinksInPath().standardizedFileURL) == canonical
      }
    }

    /// App-only metadata maintenance after a bookmark follows a rename or move. Preserve
    /// the image generation and its capture time. This and `publish` run on the main actor
    /// without awaiting between reading and writing the record.
    @MainActor @discardableResult public func refreshLocation(id: String, url: URL) throws -> Bool {
      guard let current = record(id: id) else { return false }
      let canonical = url.resolvingSymlinksInPath().standardizedFileURL
      guard try Self.identity(url) == current.fileIdentity,
        try resolve(current).resolvingSymlinksInPath().standardizedFileURL == canonical
      else { throw CocoaError(.fileNoSuchFile) }
      let title = url.deletingPathExtension().lastPathComponent
      let volumePath = try url.resourceValues(forKeys: [.volumeURLKey]).volume?.path
      // A bookmark may normalize linked parent folders without the document moving.
      let previousLocation = current.sourcePath.map {
        URL(fileURLWithPath: $0).resolvingSymlinksInPath().standardizedFileURL
      }
      guard current.title != title || previousLocation != canonical || current.volumePath != volumePath else {
        return false
      }
      let updated = WidgetRecord(
        id: current.id, title: title, fileIdentity: current.fileIdentity, generation: current.generation,
        updatedAt: current.updatedAt, refreshMinutes: current.refreshMinutes,
        sourcePath: url.path, volumePath: volumePath, previousGeneration: current.previousGeneration,
        sourceVersion: current.sourceVersion)
      guard updated.isValid else { throw CocoaError(.fileWriteInvalidFileName) }
      let json = try JSONEncoder().encode(updated)
      guard json.count <= 16384 else { throw CocoaError(.fileWriteInvalidFileName) }
      let bookmark = try url.bookmarkData(
        options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil)
      try bookmark.write(to: folder(id).appendingPathComponent("document.bookmark"), options: .atomic)
      try json.write(to: folder(id).appendingPathComponent("current.json"), options: .atomic)
      return true
    }

  #endif

  /// Own the bytes before returning; cleanup may remove the file after a pointer read.
  public func imageData(_ record: WidgetRecord, _ family: WidgetImageFamily) -> Data? {
    if let data = try? Data(contentsOf: imageURL(record, family)) { return data }
    guard let current = self.record(id: record.id) else { return nil }
    return try? Data(contentsOf: imageURL(current, family))
  }

  private func pruneGenerations(id: String, keeping: Set<String>) {
    let children = (try? FileManager.default.contentsOfDirectory(at: folder(id), includingPropertiesForKeys: nil)) ?? []
    for child in children
    where UUID(uuidString: child.lastPathComponent) != nil && !keeping.contains(child.lastPathComponent) {
      try? FileManager.default.removeItem(at: child)
    }
  }

  #if os(macOS)
    /// App-only maintenance. A disconnected volume or denied access is not a deletion.
    @discardableResult public func pruneUnavailable() -> Bool {
      var removed = false
      let directories = (try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)) ?? []
      for directory in directories where UUID(uuidString: directory.lastPathComponent) != nil {
        if !FileManager.default.fileExists(atPath: directory.appendingPathComponent("current.json").path) {
          try? FileManager.default.removeItem(at: directory)
        }
      }
      for record in records() {
        pruneGenerations(id: record.id, keeping: Set([record.generation, record.previousGeneration].compactMap { $0 }))
        do {
          _ = try resolve(record)
          continue
        } catch let error as CocoaError {
          guard error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile else { continue }
        } catch { continue }
        guard let path = record.sourcePath, let volumePath = record.volumePath else { continue }
        let volume = URL(fileURLWithPath: volumePath)
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: volume.path),
          let device = attrs[.systemNumber] as? NSNumber,
          record.fileIdentity?.hasPrefix("\(device):") == true
        else { continue }
        let source = URL(fileURLWithPath: path)
        // Require an accessible parent on the original mounted volume before treating
        // absence as permanent. Other I/O failures retain the last useful image.
        guard (try? source.deletingLastPathComponent().checkResourceIsReachable()) == true else { continue }
        let obsolete: Bool
        do { obsolete = try Self.identity(source) != record.fileIdentity } catch let error as CocoaError {
          obsolete = error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile
        } catch { obsolete = false }
        if obsolete {
          do {
            try FileManager.default.removeItem(at: folder(record.id))
            removed = true
          } catch {}
        }
      }
      return removed
    }

    /// Write every image and the bookmark before replacing the one visible pointer. A
    /// failure leaves the previous generation readable. Main-actor calls do not await
    /// between reading and writing the record, including during `refreshLocation`.
    @MainActor @discardableResult public func publish(
      url: URL, title: String, images: [WidgetImage], capturedAt: Date, refreshMinutes: UInt32?,
      sourceVersion: String? = nil
    ) throws -> WidgetRecord {
      let old = record(for: url)
      if let old, old.updatedAt > capturedAt.timeIntervalSince1970 { return old }
      let identity = try Self.identity(url)
      let id = old?.id ?? UUID().uuidString
      let generation = UUID().uuidString
      let record = WidgetRecord(
        id: id, title: title, fileIdentity: identity, generation: generation,
        updatedAt: capturedAt.timeIntervalSince1970, refreshMinutes: refreshMinutes,
        sourcePath: url.path, volumePath: try url.resourceValues(forKeys: [.volumeURLKey]).volume?.path,
        previousGeneration: old?.generation, sourceVersion: sourceVersion)
      let bookmark = try url.bookmarkData(
        options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil)
      return try publish(record, old: old, images: images, bookmark: bookmark)
    }
  #endif

  @MainActor @discardableResult public func publish(
    libraryID: String, title: String, images: [WidgetImage], capturedAt: Date, sourceVersion: String
  ) throws -> WidgetRecord {
    let old = record(id: libraryID)
    if let old, old.updatedAt > capturedAt.timeIntervalSince1970 { return old }
    let record = WidgetRecord(
      id: libraryID, title: title, fileIdentity: nil, generation: UUID().uuidString,
      updatedAt: capturedAt.timeIntervalSince1970, refreshMinutes: nil,
      sourcePath: nil, volumePath: nil, previousGeneration: old?.generation, sourceVersion: sourceVersion)
    return try publish(record, old: old, images: images, bookmark: nil)
  }

  @MainActor private func publish(
    _ record: WidgetRecord, old: WidgetRecord?, images: [WidgetImage], bookmark: Data?
  ) throws -> WidgetRecord {
    guard record.isValid else { throw CocoaError(.fileWriteInvalidFileName) }
    let json = try JSONEncoder().encode(record)
    guard json.count <= 16384 else { throw CocoaError(.fileWriteInvalidFileName) }
    let id = record.id
    let generation = record.generation
    guard images.count == 3,
      WidgetImageFamily.allCases.allSatisfy({ family in
        images.filter { $0.family == family }.count == 1
      })
    else { throw CocoaError(.fileWriteUnknown) }
    // Refuse corrupt/oversized input before publishing a selectable document.
    for image in images {
      guard image.png.count <= 4 * 1024 * 1024,
        let source = CGImageSourceCreateWithData(image.png as CFData, nil),
        let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
        let width = props[kCGImagePropertyPixelWidth] as? Int,
        let height = props[kCGImagePropertyPixelHeight] as? Int,
        width > 0, height > 0, width <= 1024, height <= 1024,
        CGImageSourceCreateImageAtIndex(source, 0, nil) != nil
      else { throw CocoaError(.fileReadCorruptFile) }
    }
    let directory = folder(id).appendingPathComponent(generation, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    var committed = false
    defer { if !committed { try? FileManager.default.removeItem(at: directory) } }
    for image in images { try image.png.write(to: imageURL(record, image.family), options: .atomic) }
    try bookmark?.write(to: folder(id).appendingPathComponent("document.bookmark"), options: .atomic)
    try json.write(to: folder(id).appendingPathComponent("current.json"), options: .atomic)
    committed = true
    pruneGenerations(id: id, keeping: Set([generation, old?.generation].compactMap { $0 }))
    return record
  }
  private struct LibraryIndex: Codable {
    let generation: String
    let previousGeneration: String?
    let entries: [WidgetLibraryEntry]
  }
  public func libraryIndex() -> [WidgetLibraryEntry] {
    for _ in 0..<2 {
      guard let data = try? Data(contentsOf: root.appendingPathComponent("index.json")),
        let index = try? JSONDecoder().decode(LibraryIndex.self, from: data),
        UUID(uuidString: index.generation) != nil, index.entries.allSatisfy({ $0.isValid })
      else { return [] }
      let directory = root.appendingPathComponent("icons").appendingPathComponent(index.generation)
      guard FileManager.default.fileExists(atPath: directory.path) else { continue }
      let entries = index.entries.map {
        WidgetLibraryEntry(
          id: $0.id, title: $0.title,
          icon: try? Data(contentsOf: directory.appendingPathComponent("\($0.id).png")))
      }
      if FileManager.default.fileExists(atPath: directory.path) { return entries }
    }
    return []
  }

  @MainActor public func writeLibraryIndex(_ entries: [WidgetLibraryEntry]) throws {
    guard entries.allSatisfy({ $0.isValid }), Set(entries.map(\.id)).count == entries.count else {
      throw CocoaError(.fileWriteInvalidFileName)
    }
    let previous = (try? Data(contentsOf: root.appendingPathComponent("index.json")))
      .flatMap { try? JSONDecoder().decode(LibraryIndex.self, from: $0) }?.generation
    let generation = UUID().uuidString
    let icons = root.appendingPathComponent("icons", isDirectory: true)
    let directory = icons.appendingPathComponent(generation, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    var committed = false
    defer { if !committed { try? FileManager.default.removeItem(at: directory) } }
    for entry in entries {
      try entry.icon?.write(to: directory.appendingPathComponent("\(entry.id).png"), options: .atomic)
    }
    let index = LibraryIndex(
      generation: generation, previousGeneration: previous,
      entries: entries.map { WidgetLibraryEntry(id: $0.id, title: $0.title, icon: nil) })
    try JSONEncoder().encode(index).write(to: root.appendingPathComponent("index.json"), options: .atomic)
    committed = true
    let children = try FileManager.default.contentsOfDirectory(at: icons, includingPropertiesForKeys: nil)
    for child in children where child.lastPathComponent != generation && child.lastPathComponent != previous {
      try? FileManager.default.removeItem(at: child)
    }
  }
  @MainActor public func remove(libraryID: String) throws {
    guard UUID(uuidString: libraryID) != nil else { throw CocoaError(.fileWriteInvalidFileName) }
    if FileManager.default.fileExists(atPath: folder(libraryID).path) {
      try FileManager.default.removeItem(at: folder(libraryID))
    }
  }
  @MainActor public func clear() throws {
    if FileManager.default.fileExists(atPath: root.path) { try FileManager.default.removeItem(at: root) }
  }
}

public struct WidgetImage: Sendable {
  public let family: WidgetImageFamily
  public let png: Data
  public init(family: WidgetImageFamily, png: Data) {
    self.family = family
    self.png = png
  }
}
/// Private presentation vocabulary: the host maps Rust's capture families to these filenames.
public enum WidgetImageFamily: String, CaseIterable, Sendable {
  case small, medium, large
  public var filename: String { rawValue }
}

public struct WidgetRecord: Codable, Sendable {
  public let id: String
  public let title: String
  public let fileIdentity: String?
  public let generation: String
  public let updatedAt: Double
  public let refreshMinutes: UInt32?
  let sourcePath: String?
  let volumePath: String?
  let previousGeneration: String?
  /// Opaque core token for saved state, renderer/fonts and canvas settings.
  /// Missing/older tokens stay readable and are refreshed by the app.
  public let sourceVersion: String?

  var isValid: Bool {
    UUID(uuidString: id) != nil && UUID(uuidString: generation) != nil
      && !title.isEmpty && title.utf8.count <= 4096
      && (fileIdentity == nil || (!fileIdentity!.isEmpty && fileIdentity!.utf8.count <= 256))
      && updatedAt.isFinite && updatedAt >= 0
      && (previousGeneration == nil || UUID(uuidString: previousGeneration!) != nil)
  }
}

public struct WidgetLibraryEntry: Codable, Sendable {
  public let id: String
  public let title: String
  public let icon: Data?
  public init(id: String, title: String, icon: Data?) {
    self.id = id
    self.title = title
    self.icon = icon
  }
  var isValid: Bool {
    UUID(uuidString: id) != nil && !title.isEmpty && title.utf8.count <= 4096
      && (icon?.count ?? 0) <= 256 * 1024
  }
}

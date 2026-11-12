import Foundation
import ImageIO

/// Image output only. The extension never opens a document or participates in its owner.
public struct WidgetCache: Sendable {
  public static let kind = "click.dotpop.DocumentWidget"
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
    return WidgetCache(root: group.appendingPathComponent("library-widgets", isDirectory: true))
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
    URL(string: "dotpop://widget/\(id)")!
  }
  public static func selectionID(from url: URL) -> String? {
    guard url.scheme == "dotpop", url.host == "widget", url.query == nil, url.fragment == nil,
      url.pathComponents.count == 2, let id = url.pathComponents.last, UUID(uuidString: id) != nil
    else { return nil }
    return id
  }
  private func folder(_ id: String) -> URL { root.appendingPathComponent(id, isDirectory: true) }

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

  @MainActor @discardableResult public func publish(
    libraryID: String, title: String, images: [WidgetImage], capturedAt: Date, refreshMinutes: UInt32? = nil,
    sourceVersion: String?
  ) throws -> WidgetRecord {
    let old = record(id: libraryID)
    if let old, old.updatedAt > capturedAt.timeIntervalSince1970 { return old }
    let record = WidgetRecord(
      id: libraryID, title: title, generation: UUID().uuidString,
      updatedAt: capturedAt.timeIntervalSince1970, refreshMinutes: refreshMinutes,
      previousGeneration: old?.generation, sourceVersion: sourceVersion)
    return try publish(record, old: old, images: images)
  }

  @MainActor private func publish(
    _ record: WidgetRecord, old: WidgetRecord?, images: [WidgetImage]
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
          icon: try? Data(contentsOf: directory.appendingPathComponent("\($0.id).png")), accountBound: $0.accountBound)
      }
      if FileManager.default.fileExists(atPath: directory.path) { return entries }
    }
    return []
  }

  @MainActor public func writeLibraryIndex(_ entries: [WidgetLibraryEntry], retainingPrevious: Bool = true) throws {
    guard entries.allSatisfy({ $0.isValid }), Set(entries.map(\.id)).count == entries.count else {
      throw CocoaError(.fileWriteInvalidFileName)
    }
    let previous =
      retainingPrevious
      ? (try? Data(contentsOf: root.appendingPathComponent("index.json")))
        .flatMap { try? JSONDecoder().decode(LibraryIndex.self, from: $0) }?.generation
      : nil
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
      entries: entries.map { WidgetLibraryEntry(id: $0.id, title: $0.title, icon: nil, accountBound: $0.accountBound) })
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
  public let generation: String
  public let updatedAt: Double
  public let refreshMinutes: UInt32?
  let previousGeneration: String?
  /// Opaque core token for saved state, renderer/fonts and canvas settings.
  /// Missing/older tokens stay readable and are refreshed by the app.
  public let sourceVersion: String?

  var isValid: Bool {
    UUID(uuidString: id) != nil && UUID(uuidString: generation) != nil
      && !title.isEmpty && title.utf8.count <= 4096
      && updatedAt.isFinite && updatedAt >= 0
      && (previousGeneration == nil || UUID(uuidString: previousGeneration!) != nil)
  }
}

public struct WidgetLibraryEntry: Codable, Sendable {
  public let id: String
  public let title: String
  public let icon: Data?
  public let accountBound: Bool
  public init(id: String, title: String, icon: Data?, accountBound: Bool = false) {
    self.id = id
    self.title = title
    self.icon = icon
    self.accountBound = accountBound
  }
  var isValid: Bool {
    UUID(uuidString: id) != nil && !title.isEmpty && title.utf8.count <= 4096
      && (icon?.count ?? 0) <= 256 * 1024
  }
}

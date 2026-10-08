import Darwin
import Foundation
import SQLite3

public struct DiskState: Codable {
  public let checkpoint: String?
  public let updates: [String]
  public let generation: String
  public let schemaKey: String?
  public let docId: String
  public let readerRevision: Int
}
public struct Metadata: Codable {
  public let generation: String
  public let schemaKey: String?
  public let docId: String
  public let readerRevision: Int
  public let checkpointBytes: Int
  public let updateBytes: Int
  public let updateRows: Int
}
/// One serial executor owns this object. Uses the production format and sync policy,
/// but only opens fresh harness directories, never user packages.
public final class Store {
  private var db: OpaquePointer?
  private var lock: Int32 = -1
  public var beforeCommit: (() throws -> Void)?
  public var afterCommit: (() throws -> Void)?
  public private(set) var writtenBytes = 0
  public private(set) var commits = 0
  public let maximumBytes: Int
  public init(root: URL, maximumBytes: Int = 32 * 1024 * 1024) throws {
    self.maximumBytes = maximumBytes
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    lock = Darwin.open(
      root.appendingPathComponent("writer.lock").path, O_CREAT | O_RDWR | O_NOFOLLOW, 0o600)
    guard lock >= 0, flock(lock, LOCK_EX | LOCK_NB) == 0 else {
      if lock >= 0 {
        Darwin.close(lock)
        lock = -1
      }
      throw SpikeError("writer_busy")
    }
    do {
      guard let resolved = Darwin.realpath(root.path, nil) else {
        throw SpikeError("Cannot resolve storage root")
      }
      let databasePath = String(cString: resolved) + "/document.sqlite"
      free(resolved)
      guard
        sqlite3_open_v2(
          databasePath, &db,
          SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_NOFOLLOW,
          nil) == SQLITE_OK
      else { throw error() }
      try exec(
        "PRAGMA trusted_schema=OFF; PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON;"
      )
      try exec(
        "CREATE TABLE IF NOT EXISTS document(id INTEGER PRIMARY KEY CHECK(id=1), checkpoint BLOB, schema_key TEXT, generation INTEGER NOT NULL, doc_id TEXT NOT NULL, reader_revision INTEGER NOT NULL); INSERT OR IGNORE INTO document VALUES(1,NULL,NULL,0,lower(hex(randomblob(16))),1); CREATE TABLE IF NOT EXISTS updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL); PRAGMA user_version=2;"
      )
    } catch {
      close()
      throw error
    }
  }
  deinit { close() }
  public func close() {
    if let db {
      sqlite3_close(db)
      self.db = nil
    }
    if lock >= 0 {
      flock(lock, LOCK_UN)
      Darwin.close(lock)
      lock = -1
    }
  }
  private func error() -> SpikeError {
    SpikeError(db.map { String(cString: sqlite3_errmsg($0)) } ?? "storage_closed")
  }
  private func exec(_ sql: String) throws {
    guard db != nil, sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else { throw error() }
  }
  private func statement(_ sql: String) throws -> OpaquePointer {
    var p: OpaquePointer?
    guard db != nil, sqlite3_prepare_v2(db, sql, -1, &p, nil) == SQLITE_OK, let p else {
      throw error()
    }
    return p
  }
  private func blob(_ p: OpaquePointer, _ column: Int32) -> Data? {
    guard sqlite3_column_type(p, column) != SQLITE_NULL else { return nil }
    let n = Int(sqlite3_column_bytes(p, column))
    return sqlite3_column_blob(p, column).map { Data(bytes: $0, count: n) } ?? Data()
  }
  public func load() throws -> DiskState {
    let p = try statement("SELECT checkpoint,schema_key,generation,doc_id FROM document WHERE id=1")
    defer { sqlite3_finalize(p) }
    guard sqlite3_step(p) == SQLITE_ROW else { throw error() }
    let checkpoint = blob(p, 0)?.base64EncodedString()
    let key = sqlite3_column_text(p, 1).map { String(cString: $0) }
    let gen = String(sqlite3_column_int64(p, 2))
    let id = String(cString: sqlite3_column_text(p, 3))
    let u = try statement("SELECT bytes FROM updates ORDER BY seq")
    defer { sqlite3_finalize(u) }
    var updates: [String] = []
    while sqlite3_step(u) == SQLITE_ROW { updates.append(blob(u, 0)!.base64EncodedString()) }
    return DiskState(
      checkpoint: checkpoint, updates: updates, generation: gen, schemaKey: key, docId: id,
      readerRevision: 1)
  }
  public func metadata() throws -> Metadata {
    let p = try statement(
      "SELECT generation,schema_key,doc_id,coalesce(length(checkpoint),0),(SELECT coalesce(sum(length(bytes)),0) FROM updates),(SELECT count(*) FROM updates) FROM document WHERE id=1"
    )
    defer { sqlite3_finalize(p) }
    guard sqlite3_step(p) == SQLITE_ROW else { throw error() }
    return Metadata(
      generation: String(sqlite3_column_int64(p, 0)),
      schemaKey: sqlite3_column_text(p, 1).map { String(cString: $0) },
      docId: String(cString: sqlite3_column_text(p, 2)), readerRevision: 1,
      checkpointBytes: Int(sqlite3_column_int64(p, 3)),
      updateBytes: Int(sqlite3_column_int64(p, 4)), updateRows: Int(sqlite3_column_int64(p, 5)))
  }
  private let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)
  public func write(
    generation: String, checkpoint: Data? = nil, updates: [Data] = [], schemaKey: String? = nil
  ) throws -> String {
    try exec("BEGIN IMMEDIATE")
    do {
      let meta = try metadata()
      guard meta.generation == generation else { throw SpikeError("revision_conflict") }
      let bytes =
        checkpoint?.count
        ?? (meta.checkpointBytes + meta.updateBytes + updates.reduce(0) { $0 + $1.count })
      guard bytes <= maximumBytes, checkpoint != nil || meta.updateRows + updates.count <= 4096
      else { throw SpikeError("capacity") }
      if let checkpoint {
        let p = try statement("UPDATE document SET checkpoint=?,schema_key=? WHERE id=1")
        defer { sqlite3_finalize(p) }
        _ = checkpoint.withUnsafeBytes {
          sqlite3_bind_blob(p, 1, $0.baseAddress, Int32($0.count), transient)
        }
        sqlite3_bind_text(p, 2, schemaKey ?? meta.schemaKey, -1, transient)
        guard sqlite3_step(p) == SQLITE_DONE else { throw error() }
        try exec("DELETE FROM updates")
      } else {
        for bytes in updates {
          let p = try statement("INSERT INTO updates(bytes) VALUES(?)")
          defer { sqlite3_finalize(p) }
          _ = bytes.withUnsafeBytes {
            sqlite3_bind_blob(p, 1, $0.baseAddress, Int32($0.count), transient)
          }
          guard sqlite3_step(p) == SQLITE_DONE else { throw error() }
        }
      }
      try exec("UPDATE document SET generation=generation+1 WHERE id=1")
      try beforeCommit?()
      try exec("COMMIT")
      commits += 1
      writtenBytes += checkpoint?.count ?? updates.reduce(0) { $0 + $1.count }
      try afterCommit?()
      return String((Int(generation) ?? 0) + 1)
    } catch {
      try? exec("ROLLBACK")
      throw error
    }
  }
}

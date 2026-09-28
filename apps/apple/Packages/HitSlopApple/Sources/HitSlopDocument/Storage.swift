import Darwin
import Foundation
import HitSlopCore
import SQLite3

func failure(_ message: String) -> NSError {
  NSError(domain: "hitSlop", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
}

/// `document` owns the package and persists writes. `snapshot` copies the saved
/// document and theme into memory without ownership; renderer writes are discarded.
public enum StorageMode: Sendable { case document, snapshot }

/// Every database call runs on `queue` (the persistence queue); the owner's core calls
/// never do. Swift never interprets Loro bytes.
final class Storage: @unchecked Sendable {
  static let maximumBytes: Int64 = 32 * 1024 * 1024
  static let maximumRows: Int64 = 4096
  static let fullMessage = "Document is full (32 MiB limit); saved state is intact. Retry saving or explicitly discard unsaved edits."
  let queue = DispatchQueue(label: "hitslop.persistence")
  private var db: OpaquePointer?
  #if DEBUG
  /// Fault injection at the real I/O boundary. Set from tests on any thread; fires on `queue`.
  private let phaseLock = NSLock()
  private var phase: ((String) throws -> Void)?
  var testingPhase: ((String) throws -> Void)? {
    get { phaseLock.withLock { phase } }
    set { phaseLock.withLock { phase = newValue } }
  }
  #endif
  private var ownership: DocumentWriterLock?
  private let root: URL
  private let inode: UInt64
  let mode: StorageMode
  private var snapshotTheme: [String: String] = [:]
  /// `doc_id` names the logical document. It is minted with the database and renewed by
  /// Duplicate; a plain filesystem copy keeps it, so it never authorizes synchronization.
  private static let schema =
    "CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id=1), checkpoint BLOB, schema_key TEXT, generation INTEGER NOT NULL, doc_id TEXT NOT NULL, last_attempt TEXT); INSERT INTO document VALUES(1,NULL,NULL,0,lower(hex(randomblob(16))),NULL); CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);"
  private func checkLocation() throws {
    let current = try FileManager.default.attributesOfItem(atPath: root.path)
    guard (current[.systemFileNumber] as? NSNumber)?.uint64Value == inode else {
      throw failure("Document moved or replaced; close before moving a document")
    }
  }
  init(root: URL, mode: StorageMode = .document) throws {
    self.root = root
    self.mode = mode
    let attributes = try FileManager.default.attributesOfItem(atPath: root.path)
    guard let inode = (attributes[.systemFileNumber] as? NSNumber)?.uint64Value else {
      throw failure("Cannot identify document directory")
    }
    self.inode = inode
    if mode == .document { ownership = try DocumentWriterLock(root: root) }
    let state = root.appendingPathComponent("state")
    do {
      let source = state.appendingPathComponent("document.sqlite")
      if mode == .snapshot {
        try openSnapshot(state: state, source: source)
        snapshotTheme = try readTheme()
      } else {
        try safeFile(source, optional: true)
        guard let resolved = Darwin.realpath(state.path, nil) else {
          throw failure("Cannot resolve storage directory")
        }
        let databasePath = String(cString: resolved) + "/document.sqlite"
        free(resolved)
        guard
          sqlite3_open_v2(
            databasePath, &db,
            SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_NOFOLLOW,
            nil) == SQLITE_OK
        else { throw error("open") }
      }
      try exec("PRAGMA trusted_schema=OFF")
      sqlite3_limit(db, SQLITE_LIMIT_LENGTH, 32 * 1024 * 1024)
      let isNew = try scalar("SELECT count(*) FROM sqlite_master WHERE type='table'") == 0
      try exec("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON;")
      if isNew {
        try exec("BEGIN IMMEDIATE; \(Self.schema) COMMIT;")
      }
    } catch {
      close()
      throw error
    }
  }
  /// Copies the saved rows into memory, so a render never creates, locks, or
  /// modifies files inside the package. Limits are checked before any blob is
  /// read, and free pages in the source file are never loaded.
  private func openSnapshot(state: URL, source: URL) throws {
    var saved: (checkpoint: Data?, schemaKey: String?, generation: Int64, docID: String, updates: [(Int64, Data)])?
    if FileManager.default.fileExists(atPath: state.path) {
      let info = try state.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
      guard info.isDirectory == true, info.isSymbolicLink != true else {
        throw failure("Unsafe directory: state")
      }
      try safeFile(source, optional: true)
    }
    if FileManager.default.fileExists(atPath: source.path),
      let resolved = Darwin.realpath(state.path, nil)
    {
      let databasePath = String(cString: resolved) + "/document.sqlite"
      free(resolved)
      guard
        sqlite3_open_v2(
          databasePath, &db, SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_NOFOLLOW, nil)
          == SQLITE_OK
      else { throw error("open snapshot") }
      sqlite3_busy_timeout(db, 5000)
      try exec("PRAGMA trusted_schema=OFF")
      sqlite3_limit(db, SQLITE_LIMIT_LENGTH, 32 * 1024 * 1024)
      // One read transaction: validation and the copied rows see the same saved state.
      try exec("BEGIN")
      if try scalar("SELECT count(*) FROM sqlite_master WHERE type='table'") > 0 {
        try checkBounds()
        let row = try statement("SELECT checkpoint,schema_key,generation,doc_id FROM document WHERE id=1")
        defer { sqlite3_finalize(row) }
        guard sqlite3_step(row) == SQLITE_ROW, let docID = sqlite3_column_text(row, 3).map({ String(cString: $0) })
        else { throw error("read") }
        let updates = try statement("SELECT seq,bytes FROM updates ORDER BY seq")
        defer { sqlite3_finalize(updates) }
        var records: [(Int64, Data)] = []
        var status = sqlite3_step(updates)
        while status == SQLITE_ROW {
          guard let bytes = try data(updates, 1) else { throw failure("Missing update bytes") }
          records.append((sqlite3_column_int64(updates, 0), bytes))
          status = sqlite3_step(updates)
        }
        guard status == SQLITE_DONE else { throw error("read updates") }
        saved = (
          try data(row, 0), sqlite3_column_text(row, 1).map { String(cString: $0) },
          sqlite3_column_int64(row, 2), docID, records
        )
      }
      try exec("COMMIT")
      guard sqlite3_close_v2(db) == SQLITE_OK else { throw error("close snapshot") }
      db = nil
    }
    guard sqlite3_open_v2(":memory:", &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX, nil) == SQLITE_OK
    else { throw error("open snapshot storage") }
    guard let saved else { return }
    try exec(Self.schema)
    let row = try statement("UPDATE document SET checkpoint=?,schema_key=?,generation=?,doc_id=? WHERE id=1")
    defer { sqlite3_finalize(row) }
    if let checkpoint = saved.checkpoint { try bind(checkpoint, row, 1) }
    if let schemaKey = saved.schemaKey {
      guard
        sqlite3_bind_text(row, 2, schemaKey, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self))
          == SQLITE_OK
      else { throw error("bind schema key") }
    }
    sqlite3_bind_int64(row, 3, saved.generation)
    guard sqlite3_bind_text(row, 4, saved.docID, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self)) == SQLITE_OK
    else { throw error("bind document ID") }
    try done(row)
    for (seq, bytes) in saved.updates {
      let update = try statement("INSERT INTO updates(seq,bytes) VALUES(?,?)")
      defer { sqlite3_finalize(update) }
      sqlite3_bind_int64(update, 1, seq)
      try bind(bytes, update, 2)
      try done(update)
    }
  }
  private func readTheme() throws -> [String: String] {
    let url = root.appendingPathComponent("state/theme.json")
    try safeFile(url, optional: true)
    if !FileManager.default.fileExists(atPath: url.path) { return [:] }
    let bytes = try SlopFile.read(url, within: root, maximumBytes: 65536)
    guard let values = try JSONSerialization.jsonObject(with: bytes) as? [String: String] else {
      throw failure("Invalid theme overrides")
    }
    return values
  }
  func close() {
    if let db {
      // All calls and teardown run on queue; no outstanding statement may outlive ownership.
      while let statement = sqlite3_next_stmt(db, nil) {
        NSLog("hitSlop: finalizing outstanding SQLite statement at close")
        sqlite3_finalize(statement)
      }
      let result = sqlite3_close_v2(db)
      guard result == SQLITE_OK else {
        NSLog("hitSlop: SQLite close failed (%d); retaining document ownership", result)
        return
      }
      self.db = nil
    }
    ownership?.close()
    ownership = nil
  }
  deinit { close() }
  private func error(_ action: String) -> NSError {
    failure("\(action): \(db.map { String(cString: sqlite3_errmsg($0)) } ?? "database closed")")
  }
  private func exec(_ sql: String) throws {
    guard sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else { throw error(sql) }
  }
  private func statement(_ sql: String) throws -> OpaquePointer {
    var s: OpaquePointer?
    guard sqlite3_prepare_v2(db, sql, -1, &s, nil) == SQLITE_OK, let s else { throw error(sql) }
    return s
  }
  private func scalar(_ sql: String) throws -> Int64 {
    let s = try statement(sql)
    defer { sqlite3_finalize(s) }
    guard sqlite3_step(s) == SQLITE_ROW else { throw error(sql) }
    return sqlite3_column_int64(s, 0)
  }
  private func data(_ s: OpaquePointer, _ column: Int32) throws -> Data? {
    guard sqlite3_column_type(s, column) != SQLITE_NULL else { return nil }
    let size = Int(sqlite3_column_bytes(s, column))
    if size == 0 { return Data() }
    guard let pointer = sqlite3_column_blob(s, column) else { throw error("read blob") }
    return Data(bytes: pointer, count: size)
  }
  private func bind(_ data: Data, _ s: OpaquePointer, _ index: Int32) throws {
    let result = data.withUnsafeBytes {
      sqlite3_bind_blob(
        s, index, $0.baseAddress, Int32(data.count),
        unsafeBitCast(-1, to: sqlite3_destructor_type.self))
    }
    guard result == SQLITE_OK else { throw error("bind bytes") }
  }
  private func done(_ s: OpaquePointer) throws {
    guard sqlite3_step(s) == SQLITE_DONE else { throw error("write") }
  }
  private func checkBounds(additionalRows: Int64 = 0, additionalBytes: Int64 = 0) throws {
    let rows = try scalar("SELECT count(*) FROM updates")
    let bytes =
      try scalar("SELECT COALESCE(sum(length(bytes)),0) FROM updates")
      + scalar("SELECT COALESCE(length(checkpoint),0) FROM document WHERE id=1")
    let schemaBytes = try scalar(
      "SELECT COALESCE(length(CAST(schema_key AS BLOB)),0) FROM document WHERE id=1")
    guard schemaBytes <= 1_048_576, rows + additionalRows <= Self.maximumRows,
      bytes + additionalBytes <= Self.maximumBytes
    else {
      // A write that would cross the limits leaves saved state intact; the window offers retry or discard.
      if additionalRows > 0 || additionalBytes > 0 { throw failure(Self.fullMessage) }
      throw failure(
        "Document exceeds storage limits (32 MiB or 4096 updates); preserve the package for recovery"
      )
    }
  }
  func call(_ args: [String: Any]) throws -> [String: Any] {
    try checkLocation()
    let method = args["method"] as? String ?? ""
    if method == "attachments.list" { return ["files": try SlopAttachments.list(in: root)] }
    if method == "attachments.read" {
      guard let id = args["attachmentID"] as? String else { throw failure("Missing attachment ID") }
      return ["bytes": try SlopAttachments.read(id, in: root).base64EncodedString()]
    }
    if method == "attachments.put" {
      guard ownership != nil, let encoded = args["bytes"] as? String,
        encoded.utf8.count <= 13981016, let data = Data(base64Encoded: encoded) else {
        throw failure("Invalid attachment bytes")
      }
      return try SlopAttachments.put(data, in: root)
    }
    if method == "theme.load" { return ["values": mode == .snapshot ? snapshotTheme : try readTheme()] }
    if method == "theme.save" {
      guard mode == .snapshot || ownership != nil else { throw failure("Document owner is closed") }
      let url = root.appendingPathComponent("state/theme.json")
      try safeFile(url, optional: true)
      guard let values = args["values"] as? [String: String] else {
        throw failure("Invalid theme values")
      }
      let bytes = try JSONSerialization.data(withJSONObject: values, options: [.sortedKeys])
      guard bytes.count <= 65536 else { throw failure("Theme exceeds 64 KiB") }
      // Snapshot renders keep their theme writes in memory with the rest of the document.
      if mode == .snapshot { snapshotTheme = values } else { try bytes.write(to: url, options: .atomic) }
      return [:]
    }
    if method == "metadata" {
      #if DEBUG
      try testingPhase?("metadata")
      #endif
      try checkBounds()
      let row = try statement("SELECT generation,schema_key,doc_id,COALESCE(length(checkpoint),0),(SELECT COALESCE(sum(length(bytes)),0) FROM updates),(SELECT count(*) FROM updates),last_attempt FROM document WHERE id=1")
      defer { sqlite3_finalize(row) }
      guard sqlite3_step(row) == SQLITE_ROW, let id = sqlite3_column_text(row, 2) else { throw error("read metadata") }
      return [
        "generation": String(sqlite3_column_int64(row, 0)),
        "schemaKey": sqlite3_column_text(row, 1).map { String(cString: $0) } ?? NSNull() as Any,
        "docId": String(cString: id), "checkpointBytes": sqlite3_column_int64(row, 3),
        "updateBytes": sqlite3_column_int64(row, 4), "updateRows": sqlite3_column_int64(row, 5),
        "lastAttempt": sqlite3_column_text(row, 6).map { String(cString: $0) } ?? NSNull() as Any,
      ]
    }
    if method == "load" {
      // Aggregate lengths are checked before allocating or base64 encoding any blobs.
      try checkBounds()
      let s = try statement("SELECT checkpoint,schema_key,generation,doc_id FROM document WHERE id=1")
      defer { sqlite3_finalize(s) }
      guard sqlite3_step(s) == SQLITE_ROW, let docID = sqlite3_column_text(s, 3).map({ String(cString: $0) })
      else { throw error("read") }
      let schema: Any = sqlite3_column_text(s, 1).map { String(cString: $0) } ?? NSNull() as Any
      let updates = try statement("SELECT bytes FROM updates ORDER BY seq")
      defer { sqlite3_finalize(updates) }
      var records: [String] = []
      var status = sqlite3_step(updates)
      while status == SQLITE_ROW {
        guard let bytes = try data(updates, 0) else { throw failure("Missing update bytes") }
        records.append(bytes.base64EncodedString())
        status = sqlite3_step(updates)
      }
      guard status == SQLITE_DONE else { throw error("read updates") }
      return [
        "checkpoint": try data(s, 0)?.base64EncodedString() ?? NSNull() as Any, "schemaKey": schema,
        "generation": String(sqlite3_column_int64(s, 2)), "updates": records, "docId": docID,
      ]
    }
    try exec("BEGIN IMMEDIATE")
    do {
      guard
        args["generation"] as? String
          == String(try scalar("SELECT generation FROM document WHERE id=1"))
      else { throw failure("revision_conflict") }
      switch method {
      case "append":
        guard let updates = args["updates"] as? [String] else { throw failure("Missing updates") }
        guard !updates.isEmpty, updates.count <= Self.maximumRows else {
          throw failure("Invalid update count")
        }
        let incoming = try updates.map { encoded -> Data in
          guard let bytes = Data(base64Encoded: encoded), !bytes.isEmpty else {
            throw failure("Invalid update bytes")
          }
          return bytes
        }
        try checkBounds(
          additionalRows: Int64(incoming.count),
          additionalBytes: incoming.reduce(0) { $0 + Int64($1.count) })
        let s = try statement("INSERT INTO updates(bytes) VALUES(?)")
        defer { sqlite3_finalize(s) }
        for bytes in incoming {
          sqlite3_reset(s)
          sqlite3_clear_bindings(s)
          try bind(bytes, s, 1)
          try done(s)
        }
        try exec("UPDATE document SET generation=generation+1 WHERE id=1")
      case "checkpoint":
        guard let encoded = args["bytes"] as? String, let bytes = Data(base64Encoded: encoded),
          !bytes.isEmpty,
          let key = args["schemaKey"] as? String
        else { throw failure("Invalid checkpoint") }
        guard bytes.count <= Self.maximumBytes, key.utf8.count <= 1_048_576 else {
          throw failure(Self.fullMessage)
        }
        let s = try statement(
          "UPDATE document SET checkpoint=?,schema_key=?,generation=generation+1 WHERE id=1")
        defer { sqlite3_finalize(s) }
        try bind(bytes, s, 1)
        guard
          sqlite3_bind_text(s, 2, key, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self))
            == SQLITE_OK
        else { throw error("bind schema key") }
        try done(s)
        try exec("DELETE FROM updates")
      default: throw failure("Unknown storage method")
      }
      // The attempt token lets a writer whose reply was lost learn whether this write committed.
      guard let attempt = args["attempt"] as? String, !attempt.isEmpty else { throw failure("Missing write attempt") }
      let marker = try statement("UPDATE document SET last_attempt=? WHERE id=1")
      defer { sqlite3_finalize(marker) }
      guard sqlite3_bind_text(marker, 1, attempt, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self)) == SQLITE_OK
      else { throw error("bind attempt") }
      try done(marker)
      #if DEBUG
      try testingPhase?(method + ":uncommitted")
      #endif
      try exec("COMMIT")
      #if DEBUG
      try testingPhase?(method + ":committed")
      #endif
      return ["generation": String(try scalar("SELECT generation FROM document WHERE id=1"))]
    } catch {
      try? exec("ROLLBACK")
      throw error
    }
  }
}

import Foundation
import SQLite3

/// Benchmark scratch database, not a production package writer. The runner supplies fresh scratch paths.
/// Same durability pragmas as the placement harness; JSON snapshots replace Loro bytes.
final class Store {
  private var db: OpaquePointer?
  private(set) var commits = 0
  private(set) var bytes = 0
  let url: URL
  init(_ url: URL) throws {
    self.url = url
    guard sqlite3_open_v2(url.path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX, nil) == SQLITE_OK else { throw Failure("SQLite open") }
    try exec("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA fullfsync=ON; CREATE TABLE IF NOT EXISTS state(id INTEGER PRIMARY KEY, value TEXT NOT NULL);")
  }
  private func exec(_ sql: String) throws {
    guard sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else { throw Failure(String(cString: sqlite3_errmsg(db))) }
  }
  func save(_ value: String) throws {
    var statement: OpaquePointer?
    guard sqlite3_prepare_v2(db, "INSERT OR REPLACE INTO state VALUES(1,?)", -1, &statement, nil) == SQLITE_OK else { throw Failure("SQLite prepare") }
    defer { sqlite3_finalize(statement) }
    sqlite3_bind_text(statement, 1, value, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self))
    guard sqlite3_step(statement) == SQLITE_DONE else { throw Failure(String(cString: sqlite3_errmsg(db))) }
    commits += 1; bytes += value.utf8.count
  }
  func read() throws -> String {
    var s: OpaquePointer?
    guard sqlite3_prepare_v2(db, "SELECT value FROM state WHERE id=1", -1, &s, nil) == SQLITE_OK else { throw Failure("SQLite read") }
    defer { sqlite3_finalize(s) }
    guard sqlite3_step(s) == SQLITE_ROW, let p = sqlite3_column_text(s, 0) else { throw Failure("Missing snapshot") }
    return String(cString: p)
  }
  func close() { if let db { sqlite3_close(db); self.db = nil } }
  deinit { close() }
}

"""Consolidate the two SQLite disk opens within Storage."""
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[3]/"generated/boundary-simplification"/sys.argv[1]
p=root/"apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Storage.swift"
s=p.read_text()
start=s.index("        guard let resolved = Darwin.realpath")
end=s.index("\n      }\n      try exec", start)
s=s[:start]+'''        try openDisk(state: state, flags: SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
          operation: "open", busyMS: 2000)'''+s[end:]
start=s.index("    if FileManager.default.fileExists(atPath: source.path),")
end=s.index('      try exec("PRAGMA trusted_schema=OFF")', start)
s=s[:start]+'''    if FileManager.default.fileExists(atPath: source.path) {
      try openDisk(state: state, flags: SQLITE_OPEN_READONLY, operation: "open snapshot", busyMS: 5000)
'''+s[end:]
marker="  /// Copies the saved rows into memory"
helper='''  private func openDisk(state: URL, flags: Int32, operation: String, busyMS: Int32) throws {
    guard let resolved = Darwin.realpath(state.path, nil) else {
      throw failure("Cannot resolve storage directory")
    }
    defer { free(resolved) }
    let path = String(cString: resolved) + "/document.sqlite"
    guard sqlite3_open_v2(path, &db, flags | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_NOFOLLOW, nil) == SQLITE_OK
    else { throw error(operation) }
    sqlite3_busy_timeout(db, busyMS)
  }
'''
s=s.replace(marker, helper+marker)
p.write_text(s)

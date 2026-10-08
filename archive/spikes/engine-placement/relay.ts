import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
/** Local durable opaque-log fixture. No auth/deployment claims; loopback callers only. */
export class RelayLog {
  private db: Database;
  constructor(path: string) {
    this.db = new Database(path, { create: true });
    this.db.exec(
      "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS entries(seq INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE, hash TEXT NOT NULL, bytes BLOB NOT NULL)",
    );
  }
  append(id: string, bytes: Uint8Array) {
    if (!id || id.length > 100 || bytes.length > 2 * 1024 * 1024)
      throw new Error("Invalid relay envelope");
    const hash = createHash("sha256").update(bytes).digest("hex");
    return this.db.transaction(() => {
      const existing = this.db.query("SELECT seq,hash FROM entries WHERE id=?").get(id) as {
        seq: number;
        hash: string;
      } | null;
      if (existing) {
        if (existing.hash !== hash) throw new Error("Retry ID reused for different bytes");
        return existing.seq;
      }
      this.db.query("INSERT INTO entries(id,hash,bytes) VALUES(?,?,?)").run(id, hash, bytes);
      return Number(
        this.db.query<{ seq: number }, []>("SELECT last_insert_rowid() AS seq").get()!.seq,
      );
    })();
  }
  after(cursor: number) {
    return this.db
      .query("SELECT seq,id,bytes FROM entries WHERE seq>? ORDER BY seq LIMIT 128")
      .all(cursor) as { seq: number; id: string; bytes: Uint8Array }[];
  }
  close() {
    this.db.close();
  }
}

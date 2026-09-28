import { Database } from "bun:sqlite";
import { join } from "node:path";
import { realpath, stat, type FileHandle } from "node:fs/promises";
import { acquireWriter } from "./writer-lock.ts";
import type { ByteStore, Stored } from "../src/storage.ts";
/** Format-1 storage interoperability/crash test fixture. Never used by the document CLI. */
export class SQLiteStore implements ByteStore {
  private constructor(
    private db: Database,
    private lease: FileHandle,
    private phase?: (phase: string) => void,
    private root?: string,
    private inode?: number,
  ) {}
  static async open(root: string, phase?: (phase: string) => void) {
    root = await realpath(root);
    const lease = await acquireWriter(root);
    let db: Database | undefined;
    try {
      db = new Database(join(root, "state/document.sqlite"), { create: true });
      const version = (db.query("PRAGMA user_version").get() as { user_version: number })
        .user_version;
      if (version !== 0 && version !== 1) throw new Error("Unsupported database format");
      if (version === 0 && db.query("SELECT name FROM sqlite_master WHERE type='table'").get())
        throw new Error("Legacy database; migration is not implemented");
      db.exec("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;");
      if (version === 0)
        db.exec(`BEGIN IMMEDIATE;
        CREATE TABLE IF NOT EXISTS document(id INTEGER PRIMARY KEY CHECK(id=1), checkpoint BLOB, schema_key TEXT, generation INTEGER NOT NULL, doc_id TEXT NOT NULL);
        INSERT OR IGNORE INTO document VALUES(1,NULL,NULL,0,lower(hex(randomblob(16))));
        CREATE TABLE IF NOT EXISTS updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);
        PRAGMA user_version=1; COMMIT;`);
      return new SQLiteStore(db, lease, phase, root, (await stat(root)).ino);
    } catch (error) {
      db?.close();
      await lease.close();
      throw error;
    }
  }
  private async checkLocation() {
    if (this.root && (await stat(this.root)).ino !== this.inode)
      throw new Error("Document moved or replaced; close before moving");
  }
  async load(): Promise<Stored> {
    await this.checkLocation();
    const row = this.db
      .query("SELECT checkpoint,schema_key,generation,doc_id FROM document WHERE id=1")
      .get() as { checkpoint: Uint8Array | null; schema_key: string | null; generation: number; doc_id: string };
    const updates = this.db.query("SELECT bytes FROM updates ORDER BY seq").all() as {
      bytes: Uint8Array;
    }[];
    return {
      checkpoint: row.checkpoint,
      schemaKey: row.schema_key,
      generation: String(row.generation),
      updates: updates.map((r) => r.bytes),
      docId: row.doc_id,
    };
  }
  private check(generation: string) {
    const row = this.db.query("SELECT generation FROM document WHERE id=1").get() as {
      generation: number;
    };
    if (String(row.generation) !== generation) throw new Error("revision_conflict");
  }
  async append(generation: string, updates: Uint8Array[]) {
    await this.checkLocation();
    if (updates.some((b) => b.length > 32 * 1024 * 1024)) throw new Error("Update too large");
    this.db
      .transaction(() => {
        this.check(generation);
        for (const bytes of updates)
          this.db.query("INSERT INTO updates(bytes) VALUES(?)").run(bytes);
        this.db.exec("UPDATE document SET generation=generation+1 WHERE id=1");
        this.phase?.("append:uncommitted");
      })
      .immediate();
    this.phase?.("append:committed");
    return String(
      (this.db.query("SELECT generation FROM document WHERE id=1").get() as { generation: number })
        .generation,
    );
  }
  async checkpoint(generation: string, bytes: Uint8Array, key: string) {
    await this.checkLocation();
    if (bytes.length > 32 * 1024 * 1024) throw new Error("Checkpoint too large");
    this.db
      .transaction(() => {
        this.check(generation);
        this.db
          .query("UPDATE document SET checkpoint=?,schema_key=?,generation=generation+1 WHERE id=1")
          .run(bytes, key);
        this.db.exec("DELETE FROM updates");
        this.phase?.("checkpoint:uncommitted");
      })
      .immediate();
    this.phase?.("checkpoint:committed");
    return String(
      (this.db.query("SELECT generation FROM document WHERE id=1").get() as { generation: number })
        .generation,
    );
  }
  async close() {
    this.db.close();
    await this.lease.close();
  }
}

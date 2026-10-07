/** Execute the proposed DDL in memory. This is plan evidence, not the live layout. */
import { Database } from "bun:sqlite";
import assert from "node:assert/strict";

const plan = await Bun.file(new URL("../../plans/app-definition-and-packaging.md", import.meta.url)).text();
const section = plan.split("## 9. SQLite layout")[1]!.split("## 10.")[0]!;
const ddl = [...section.matchAll(/```sql\n([\s\S]*?)```/g)]
  .map(match => match[1]!).find(sql => sql.includes("CREATE TABLE app("));
assert(ddl, "The plan must contain the proposed schema");
const db = new Database(":memory:");
try {
  db.exec("PRAGMA recursive_triggers=OFF");
  db.exec(ddl);
  db.exec("INSERT INTO assets VALUES('ui.js','text/javascript','identity',1,x'61')");
  const appRow = "VALUES(1,1,1,'ab','A','Description','Author',NULL,'other',NULL,'{}')";
  db.exec(`INSERT INTO app ${appRow}`);
  const snapshot = () => JSON.stringify([
    db.query("SELECT * FROM app").all(), db.query("SELECT rowid,* FROM assets").all(),
  ]);
  const before = snapshot();
  const cases: [string, string][] = [
    ["app REPLACE", `INSERT OR REPLACE INTO app ${appRow}`],
    ["app IGNORE", `INSERT OR IGNORE INTO app ${appRow}`],
    ["app UPSERT", `INSERT INTO app ${appRow} ON CONFLICT(id) DO UPDATE SET title='Changed'`],
    ["app UPDATE", "UPDATE app SET title='Changed'"],
    ["app DELETE", "DELETE FROM app"],
    ["asset REPLACE key", "INSERT OR REPLACE INTO assets VALUES('ui.js','text/javascript','identity',1,x'62')"],
    ["asset REPLACE rowid", "INSERT OR REPLACE INTO assets(rowid,key,media_type,encoding,size,bytes) VALUES(1,'new','text/javascript','identity',1,x'62')"],
    ["asset REPLACE negative rowid", "INSERT OR REPLACE INTO assets(rowid,key,media_type,encoding,size,bytes) VALUES(-1,'new','text/javascript','identity',1,x'62')"],
    ["asset IGNORE", "INSERT OR IGNORE INTO assets VALUES('ui.js','text/javascript','identity',1,x'62')"],
    ["asset UPSERT", "INSERT INTO assets VALUES('ui.js','text/javascript','identity',1,x'62') ON CONFLICT(key) DO UPDATE SET bytes=x'62'"],
    ["program inserted after seal", "INSERT INTO assets VALUES('commands.js','text/javascript','identity',1,x'62')"],
    ["asset UPDATE", "UPDATE assets SET bytes=x'62'"],
    ["asset DELETE", "DELETE FROM assets"],
  ];
  for (const [name, sql] of cases) {
    assert.throws(() => db.exec(sql), name);
    assert.equal(snapshot(), before, `${name} changed sealed rows`);
  }
  // These exercise SQL lifecycle permissions, not image/hash/Loro acceptance.
  db.query("INSERT INTO attachments VALUES(?,?,?)").run("a".repeat(64), "image/png", new Uint8Array([97]));
  db.exec("DELETE FROM attachments");
  db.exec("INSERT INTO artwork VALUES('preview',x'61')");
  db.exec("INSERT INTO artwork VALUES('preview',x'62') ON CONFLICT(name) DO UPDATE SET png=excluded.png");
  assert.equal((db.query("SELECT hex(png) AS value FROM artwork").get() as {value:string}).value, "62");
  db.exec("DELETE FROM artwork; INSERT INTO artwork VALUES('preview',x'63')");
  db.exec("INSERT INTO document VALUES(1); INSERT INTO checkpoint VALUES(1,x'61'); INSERT INTO updates VALUES(1,x'62')");
  assert.equal(snapshot(), before);
  console.log(JSON.stringify({
    sqlite: (db.query("SELECT sqlite_version() AS version").get() as {version:string}).version,
    bypassesRefused: cases.map(([name]) => name),
    appAndAssetsUnchanged: true,
    allowed: ["attachment insert/delete", "artwork upsert", "artwork delete/insert", "document/checkpoint/update insertion"],
  }, null, 2));
} finally { db.close(); }

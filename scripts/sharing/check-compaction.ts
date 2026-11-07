/** Qualification against the real Rust relay. The wrapper exists only in Miniflare:
 * no test route, clock override or storage bypass is compiled into the Worker. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import type { Miniflare } from "miniflare";
import { SharingHost } from "../../packages/pop/src/wire/constants.generated";

export const documentName = (id: string) => id;
export const objectKey = (id: string, suffix: string) => `${id}/${suffix}`;

export function relayHarness(source: string) {
  return `import relay, {CloudDocument as Room, DocumentAccount as Account} from ${JSON.stringify(source)};
export default relay;
export class DocumentAccount {
  constructor(state,env){this.state=state;this.inner=new Account(state,env);}
  async fetch(req){
    if (new URL(req.url).pathname === '/__test/sql') {
      const {sql,args=[]} = await req.json();
      return Response.json(this.state.storage.sql.exec(sql,...args).toArray());
    }
    try{return await this.inner.fetch(req);}catch(error){console.error('Test account failure:',String(error));throw error;}
  }
}
export class CloudDocument {
  constructor(state, env) {
    this.state = state;
    this.failDeletes = false;
    const bucket = new Proxy(env.SEEDS, {get: (target, key) => {
      if (key === 'delete') return (...args) => {
        if (this.failDeletes) throw new Error('Injected R2 delete failure');
        return target.delete(...args);
      };
      const value = Reflect.get(target, key, target);
      return typeof value === 'function' && key !== 'constructor' ? value.bind(target) : value;
    }});
    this.inner = new Room(state, {...env, SEEDS: bucket});
  }
  async fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/__test/body-reads') return Response.json(this.bodyReads ?? 0);
    if (req.headers.get('X-Test-Observe-Body') === '1' && req.body) {
      const reader = req.body.getReader();
      const body = new ReadableStream({
        pull: async controller => {
          this.bodyReads = (this.bodyReads ?? 0) + 1;
          const next = await reader.read();
          if (next.done) controller.close(); else controller.enqueue(next.value);
        },
        cancel: reason => reader.cancel(reason),
      }, {highWaterMark:0});
      req = new Request(req.url, {method:req.method, headers:req.headers, body});
    }
    if (path === '/__test/delete-failure') {
      this.failDeletes = (await req.json()).enabled;
      return new Response('ready');
    }
    if (path === '/__test/sql') {
      const {sql,args=[]} = await req.json();
      return Response.json(this.state.storage.sql.exec(sql,...args).toArray());
    }
    if (path === '/__test/fixture') {
      const {count,size=1} = await req.json();
      this.state.storage.transactionSync(() => {
        this.state.storage.sql.exec('DELETE FROM records WHERE seq>0; DELETE FROM chunks WHERE seq>0');
        for (let n=1;n<=count;n++) {
          this.state.storage.sql.exec('INSERT INTO records(seq,spans,size) VALUES(?,?,?)', n, JSON.stringify([['0000000000000001',n-1,n]]),size);
          for(let part=0;part*1800000<size;part++) this.state.storage.sql.exec('INSERT INTO chunks VALUES(?,?,?)',n,part,new Uint8Array(Math.min(1800000,size-part*1800000)).fill(n%256));
        }
        this.state.storage.sql.exec("UPDATE meta SET value=? WHERE key='heads'",JSON.stringify([['0000000000000001',Math.max(count,JSON.parse(this.state.storage.sql.exec("SELECT value FROM meta WHERE key='base_version'").toArray()[0]?.value ?? '[]').find(([p])=>p==='0000000000000001')?.[1] ?? 0)],['0000000000000002',5]]));
      });
      return new Response('ready');
    }
    if (path === '/__test/freeze-windows') {
      const change = await req.json();
      for(const socket of this.state.getWebSockets()) {
        const state = socket.deserializeAttachment();
        socket.serializeAttachment({...state,...change,window_start:change.window_start ?? Date.now()+3600000});
      }
      return new Response('ready');
    }
    if (path === '/__test/alarm') return await this.inner.alarm();
    try { return await this.inner.fetch(req); } catch(error) { console.error('Test document failure:',String(error)); throw error; }
  }
  alarm() { return this.inner.alarm(); }
  webSocketMessage(...args) { return this.inner.webSocketMessage(...args); }
  webSocketClose(...args) { return this.inner.webSocketClose(...args); }
  webSocketError(...args) { return this.inner.webSocketError(...args); }
}`;
}
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const uint = (n: number | bigint): number[] => {
  let value = BigInt(n),
    bytes = [];
  while (value >= 128n) {
    bytes.push(Number(value & 127n) | 128);
    value >>= 7n;
  }
  return [...bytes, Number(value)];
};
const u64 = (n: number | bigint): number[] => {
  const value = BigInt(n);
  return Array.from({ length: 8 }, (_, i) => Number((value >> BigInt((7 - i) * 8)) & 255n));
};
export const helloFrame = (version: [number, number][] = [], protocol = 1) =>
  new Uint8Array([1, protocol, ...Array(32).fill(0xaa), ...uint(version.length),
    ...version.flatMap(([peer, counter]) => [...u64(peer), ...uint(counter)])]);
const hello = (n = 0, protocol = 1) => helloFrame(n ? [[1, n], [2, 5]] : [[2, 5]], protocol);
export const push = (id: number, start: number, end: number, bytes = new Uint8Array([7])) =>
  new Uint8Array([5, ...u64(id), 1, 1, ...u64(1), ...uint(start), ...uint(end), ...uint(bytes.length), ...bytes]);
export function decode(bytes: ArrayBuffer) {
  const b = new Uint8Array(bytes);
  const view = new DataView(bytes);
  let pos = 0;
  const byte = () => { assert(pos < b.length, "Truncated relay reply"); return b[pos++]!; };
  const read = () => {
    let n = 0, scale = 1;
    for (;;) {
      const v = byte(); n += (v & 127) * scale;
      if (!(v & 128)) return n;
      scale *= 128;
      assert(scale <= 2 ** 63, "Invalid relay integer");
    }
  };
  const peer = () => { const n = Number(view.getBigUint64(pos)); pos += 8; return n; };
  const version = () => {
    const v = [];
    for (let count = read(); count; count--) v.push([peer(), read()]);
    return v;
  };
  const tag = byte();
  if (tag === 2) return { tag, write: byte(), version: version() };
  if (tag === 6) return { tag, id: peer(), status: byte() };
  if (tag === 7) return { tag, code: byte() };
  if (tag === 3) {
    const records = [];
    for (let count = read(); count; count--) {
      const spans = [];
      for (let count = read(); count; count--) spans.push([peer(), read(), read()]);
      const size = read(), bytes = b.slice(pos, pos + size);
      pos += size; records.push({ spans, bytes });
    }
    return { tag, records };
  }
  assert.equal(tag, 4);
  return { tag };
}
export async function checkCompaction(
  worker: Miniflare,
  call: (
    path: string,
    token: string | undefined,
    method?: string,
    body?: unknown,
    headers?: Record<string, string>,
  ) => Promise<any>,
  owner: string,
  identity: (name: string, provider?: string) => string,
) {
  assert.equal(
    (
      await call("documents", identity("anonymous-owner", "anonymous"), "POST", {
        document: "00000000-0000-4000-a000-000000000002",
      })
    ).status,
    401,
  );
  const namespace = await worker.getDurableObjectNamespace("DOCUMENTS", "relay");
  const bucket = await worker.getR2Bucket("SEEDS", "relay");
  const room: { room: string; capability?: string; access?: string } = {
    room: "00000000-0000-4000-a000-000000000001",
  };
  const created = await call("documents", owner, "POST", { document: room.room });
  assert.equal(created.status, 204, "Document creation: " + (await created.clone().text()));
  room.capability = (await (await call("documents/"+room.room+"/sharing",owner,"POST")).json()).capability;
  const base = "documents/" + room.room;
  const stub = namespace.get(namespace.idFromName(documentName(room.room)));
  async function test(path: string, body?: unknown) {
    const response = await stub.fetch("https://test/__test/" + path, {
      method: "POST",
      body: JSON.stringify(body ?? {}),
    });
    assert.equal(response.status, 200);
    return new Response(await response.arrayBuffer(), { status: response.status });
  }
  const sql = async (sql: string, args: unknown[] = []) =>
    (await test("sql", { sql, args })).json() as Promise<any[]>;
  const fixture = async (count: number, size = 1) => test("fixture", { count, size });
  const seed = new Uint8Array(512).fill(42);
  const invalidManifestBytes = new TextEncoder().encode("{");
  const malformedManifest = await call(base + "/resources", owner, "POST", invalidManifestBytes);
  assert.equal(malformedManifest.status, 400, "Malformed app inventory is a request refusal");
  assert.equal(
    (
      await call(base + "/seed", owner, "PUT", seed, {
        "X-Sharing-SHA256": hash(seed),
        "X-Sharing-App": "aa".repeat(32),
        "X-Sharing-Version": '[["0000000000000002",5]]',
      })
    ).status,
    200,
  );
  assert.equal(
    (await call(base + "/resources", owner, "POST", { app: "aa".repeat(32), resources: [] }))
      .status,
    200,
  );
  assert.equal((await call(base + "/open", owner, "POST")).status, 200);
  room.access = identity("room-guest", "anonymous");
  assert.equal((await call(base+"/members",room.access,"POST",{capability:room.capability})).status,200);
  const unlistedBlob = new Uint8Array([23, 24, 25]);
  const unlistedHash = hash(unlistedBlob);
  await bucket.put(objectKey(room.room, "blobs/" + unlistedHash), unlistedBlob);
  assert.equal((await call(base + "/blob/" + unlistedHash, room.access)).status, 404,
    "An R2 object outside the accepted inventory is not readable");
  // An upload interrupted after its R2 write leaves only a registered cleanup key.
  await sql("INSERT INTO garbage VALUES(?)", [objectKey(room.room, "blobs/" + unlistedHash)]);
  await test("delete-failure", {enabled: true});
  await test("alarm");
  assert.equal((await sql("SELECT count(*) AS n FROM garbage"))[0].n, 1,
    "Failed blob cleanup remains registered for retry");
  await test("delete-failure", {enabled: false});
  await test("alarm");
  assert.equal((await sql("SELECT count(*) AS n FROM garbage"))[0].n, 0);
  assert.equal(await bucket.get(objectKey(room.room, "blobs/" + unlistedHash)), null);
  for (const suffix of ["/seed/extra", "/sync/extra", "/metadata/extra", "/sharing/extra", "/members/other"]) {
    assert.equal((await call(base + suffix, owner)).status, 404, "Extra route segments are refused: " + suffix);
  }
  let request = 0;
  const body = new Uint8Array(2_000_000).fill(9);
  const compact = (end: number, data = body, extra: Record<string, string> = {}) =>
    call(base + "/compact", room.access, "POST", data, {
      "X-Sharing-SHA256": hash(data),
      "X-Sharing-Version": JSON.stringify([
        ["0000000000000001", end],
        ["0000000000000002", 5],
      ]),
      "CF-Connecting-IP": "198.51.100." + ++request,
      ...extra,
    });
  await fixture(1);
  assert.equal((await compact(1)).status, 409, "Valid frontier but not due");
  assert.equal((await (await call(base, room.access)).json()).compact, false, "Every reply states whether compaction is due");
  await fixture(4, 1 << 20);
  assert.equal((await compact(5)).status, 409, "F cannot exceed heads");
  assert.equal(
    (await compact(4, body, { "X-Sharing-Version": '[["0000000000000001",4]]' })).status,
    409,
    "F cannot precede root",
  );
  // Fixture heads include the seed's peer, as accepted Push preserves it.
  await sql("UPDATE meta SET value=? WHERE key='heads'", [
    '[["0000000000000001",4],["0000000000000002",5]]',
  ]);
  assert.equal((await compact(4, body, { "X-Sharing-SHA256": "f".repeat(64) })).status, 400);
  // Fold only: a base over BASE_BYTES is the typed refusal the client pauses as outgrown.
  const oversized = await compact(4, new Uint8Array((16 << 20) + 1));
  assert.equal(oversized.status, 413, "A base over its byte limit is refused");
  assert.equal((await oversized.json()).code, "invalid_request", "The refusal the client classifies as outgrown");
  assert.equal((await sql("SELECT count(*) AS n FROM records WHERE seq=0"))[0].n, 0, "A refused base leaves no record");
  let stage = "initial backfill";
  const sockets: any[] = [];
  async function socket(ip = "203.0.113.1", endpoint = base, token = room.access) {
    const response = await call(endpoint + "/sync", token, "GET", undefined, {
      Upgrade: "websocket",
      "Sec-WebSocket-Protocol": SharingHost.socketProtocol,
      "CF-Connecting-IP": ip,
    });
    assert.equal(response.status, 101);
    const ws = response.webSocket!;
    ws.accept();
    sockets.push(ws);
    const messages: any[] = [];
    const waiters: {
      resolve: (frame: any) => void;
      reject: (error: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }[] = [];
    let closeCode: number | undefined;
    const closed = new Promise<number>((resolve) =>
      ws.addEventListener("close", (e: any) => {
        closeCode = e.code;
        resolve(e.code);
        for (const waiter of waiters.splice(0)) {
          clearTimeout(waiter.timer);
          waiter.reject(new Error("Socket closed during " + stage + ": " + e.code));
        }
      }),
    );
    ws.addEventListener("message", (e: any) => {
      const frame = decode(e.data);
      const waiter = waiters.shift();
      if (waiter) {
        clearTimeout(waiter.timer);
        waiter.resolve(frame);
      } else messages.push(frame);
    });
    const next = async () => {
      if (messages.length) return messages.shift();
      if (closeCode !== undefined) throw new Error("Socket already closed: " + closeCode);
      return new Promise<any>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("Socket message timeout: " + stage)), 5000);
        waiters.push({ resolve, reject, timer });
      });
    };
    const joined = async (n = 0) => {
      ws.send(hello(n));
      const frames = [];
      for (;;) {
        const frame = await next();
        frames.push(frame);
        if (frame.tag === 4) return frames;
      }
    };
    return { ws, next, joined, closed, messages };
  }
  assert.equal((await (await call(base, room.access)).json()).compact, true);
  stage = "connected";
  const connected = await socket();
  await connected.joined(4);
  // The seed upload stored the document's root with its heads. A live document without it
  // is a fault in the document, never something a member supplies: nothing is installed.
  const [storedRoot] = await sql("SELECT value FROM meta WHERE key='root'");
  assert.equal(storedRoot.value, '[["0000000000000002",5]]', "The seed upload stores the root");
  await sql("DELETE FROM meta WHERE key='root'");
  const rootless = await compact(3);
  assert.equal(rootless.status, 500, "Compaction without a stored root is an internal failure");
  assert.equal((await rootless.json()).code, "unavailable");
  assert.equal((await sql("SELECT count(*) AS n FROM records WHERE seq=0"))[0].n, 0, "A rootless document installs no base");
  assert.deepEqual(await sql("SELECT key FROM meta WHERE key IN ('root','base_version')"), [], "A refused base records no versions");
  await sql("INSERT INTO meta VALUES('root',?)", [storedRoot.value]);
  assert.equal((await compact(3)).status, 204, "Install partial base");
  assert.equal(connected.messages.length, 0, "Base is never broadcast");
  const rows = await sql("SELECT seq,spans FROM records ORDER BY seq");
  assert.deepEqual(
    rows.map((r) => r.seq),
    [0, 4],
  );
  assert.deepEqual(
    JSON.parse(rows[0].spans),
    [["0000000000000001", 0, 3]],
    "Seed-only peers produce no empty spans",
  );
  assert.equal((await bucket.list({prefix:objectKey(room.room, "compacted/")})).objects.length,0,"compaction retains no history archive");
  await test("alarm");
  assert.equal((await call(base,owner)).status,200,"offline retention is persistent");
  stage = "behind base";
  const behind = await socket("203.0.113.2");
  const frames = await behind.joined();
  assert.deepEqual(
    frames.filter((f) => f.tag === 3).map((f) => f.records.map((r: any) => r.bytes.length)),
    [[body.length], [1 << 20]],
    "Large base first and alone",
  );
  stage = "current backfill";
  const current = await socket("203.0.113.3");
  assert.equal((await current.joined(4)).filter((f) => f.tag === 3).length, 0);
  await fixture(4, 1 << 20); // Keep the previous base version but refill updates.
  await sql("INSERT INTO records(seq,spans,size) VALUES(5,?,0)", [
    JSON.stringify([
      ["0000000000000001", 3, 4],
      ["0000000000000002", 5, 6],
    ]),
  ]);
  await sql("UPDATE meta SET value=? WHERE key='heads'", [
    '[["0000000000000001",4],["0000000000000002",6]]',
  ]);
  assert.equal((await compact(2)).status, 409, "Nonmonotonic base");
  assert.equal((await compact(3)).status, 409, "Base must strictly advance");
  assert.equal((await compact(4)).status, 204, "A later base advances from the previous one");
  assert.deepEqual(
    (await sql("SELECT seq FROM records ORDER BY seq")).map((row) => row.seq),
    [0, 5],
    "A record with any uncovered span is retained whole",
  );
  assert.equal(
    (await sql("SELECT value FROM meta WHERE key='heads'"))[0].value,
    '[["0000000000000001",4],["0000000000000002",6]]',
    "Compaction leaves heads unchanged",
  );
  for (const ws of sockets) {
    try {
      ws.close();
    } catch {
      /* Already closed by a refusal. */
    }
  }
  stage = "quota socket";
  const limited = await socket("203.0.113.4");
  await limited.joined(4);
  await fixture(16, 1 << 20);
  stage = "quota refusal";
  limited.ws.send(push(10, 16, 17));
  assert.equal((await limited.next()).status, 4, "Bytes quota");
  await sql("UPDATE meta SET value=? WHERE key='heads'", [
    '[["0000000000000001",16],["0000000000000002",5]]',
  ]);
  assert.equal((await compact(16)).status, 204);
  stage = "quota freed";
  limited.ws.send(push(11, 16, 17));
  assert.equal((await limited.next()).status, 0, "Quota freed");
  stage = "quota echo";
  await limited.next(); // harmless echo
  const sequences = await sql("SELECT seq FROM records ORDER BY seq");
  assert.equal(sequences.length, 2);
  assert.equal(sequences[0].seq, 0, "Push preserves the base");
  assert(sequences[1].seq > 0, "Normal seq allocation remains positive");
  await fixture(32768, 0);
  assert.equal(
    (await (await call(base, room.access)).json()).compact,
    true,
    "Row threshold triggers compaction with zero update bytes",
  );
  stage = "row quota";
  limited.ws.send(push(120, 32767, 32768));
  assert.equal((await limited.next()).status, 0, "A duplicate at row quota is already stored");
  limited.ws.send(push(12, 32768, 32769));
  assert.equal((await limited.next()).status, 4, "Row quota");
  await fixture(0);
  // Budget includes duplicate pushes, so retries cannot evade it.
  stage = "budget join";
  const budget = await socket("203.0.113.5");
  await budget.joined();
  await test("freeze-windows");
  await sql("UPDATE meta SET value=? WHERE key='heads'", [
    '[["0000000000000001",16],["0000000000000002",5]]',
  ]);
  const largePush = new Uint8Array((512 << 10) - 1000);
  const large = (id: number, start = 0, end = 1) => {
    const header = new Uint8Array([
      5,
      ...u64(id),
      1,
      1,
      ...u64(1),
      ...uint(start),
      ...uint(end),
      ...uint(largePush.length),
    ]);
    const frame = new Uint8Array(header.length + largePush.length);
    frame.set(header);
    frame.set(largePush, header.length);
    return frame;
  };
  stage = "byte budget";
  for (let n = 0; n < 16; n++) {
    budget.ws.send(large(20 + n));
    assert.equal((await budget.next()).status, 0);
  }
  budget.ws.send(large(36, 16, 17));
  assert.equal(await budget.closed, 1013);
  assert.equal(
    (await sql("SELECT count(*) AS n FROM records WHERE seq>0"))[0].n,
    0,
    "Over-budget new updates are not stored",
  );
  assert(!budget.messages.some((m) => m.tag === 7));
  const countBudget = await socket("203.0.113.21");
  await countBudget.joined(1);
  await test("freeze-windows", { pushes: 1799 });
  stage = "push count budget";
  countBudget.ws.send(push(1800, 0, 1));
  assert.equal((await countBudget.next()).status, 0);
  countBudget.ws.send(push(1801, 0, 1));
  assert.equal(await countBudget.closed, 1013);
  assert(!countBudget.messages.some((m) => m.tag === 7));
  const resetBudget = await socket("203.0.113.24");
  await resetBudget.joined(1);
  await test("freeze-windows", { window_start: 0, bytes: 8 << 20, pushes: 1800 });
  resetBudget.ws.send(push(1802, 0, 1));
  assert.equal((await resetBudget.next()).status, 0, "Old minute budgets reset after hibernation");
  const perIP = [];
  for (let n = 0; n < 8; n++) perIP.push(await socket("203.0.113.20"));
  assert.equal(
    (
      await call(base + "/sync", room.access, "GET", undefined, {
        Upgrade: "websocket",
        "Sec-WebSocket-Protocol": SharingHost.socketProtocol,
        "CF-Connecting-IP": "203.0.113.20",
      })
    ).status,
    429,
  );
  for (const s of perIP) s.ws.close();
  const malformed = await socket("203.0.113.6");
  malformed.ws.send(new Uint8Array([255]));
  assert.equal((await malformed.next()).code, 5);
  assert.equal(await malformed.closed, 1008);
  const protocol = await socket("203.0.113.7");
  protocol.ws.send(hello(0, 2));
  assert.equal((await protocol.next()).code, 1);
  const transient = await socket("203.0.113.8");
  await sql("UPDATE meta SET value='broken' WHERE key='heads'");
  transient.ws.send(hello());
  assert.equal(await transient.closed, 1011);
  assert.equal(transient.messages.length, 0);
  await sql("UPDATE meta SET value='[]' WHERE key='heads'");
  for (const ws of sockets) {
    try {
      ws.close();
    } catch {
      /* Already closed by a refusal. */
    }
  }
  const unjoinedEnd = await socket("203.0.113.23");
  const ended = await socket("203.0.113.22");
  await ended.joined(32769);
  assert.equal((await call(base, owner, "DELETE")).status, 200);
  assert.equal((await ended.next()).code, 3);
  assert.equal(await ended.closed, 1008);
  assert.equal((await unjoinedEnd.next()).code, 3);
  assert.equal(await unjoinedEnd.closed, 1008);
  assert.equal((await bucket.list({ prefix: objectKey(room.room, "compacted/") })).objects.length, 0);
  assert.equal((await sql("SELECT count(*) AS n FROM records"))[0].n, 0);
  assert.equal((await sql("SELECT count(*) AS n FROM chunks"))[0].n, 0);
  assert.equal((await call(base,owner)).status,410);
  await test("alarm");
  assert.equal((await call(base,owner)).status,410,"deletion tombstone preserves UUID ownership");
  const idleRoom: { room: string; capability?: string; access?: string } = {
    room: "00000000-0000-4000-a000-000000000003",
  };
  assert.equal((await call("documents", owner, "POST", { document: idleRoom.room })).status, 204);
  const idleBase = "documents/" + idleRoom.room;
  idleRoom.capability = (await (await call(idleBase+"/sharing",owner,"POST")).json()).capability;
  assert.equal(
    (
      await call(idleBase + "/seed", owner, "PUT", seed, {
        "X-Sharing-SHA256": hash(seed),
        "X-Sharing-App": "aa".repeat(32),
        "X-Sharing-Version": "[]",
      })
    ).status,
    200,
  );
  assert.equal(
    (await call(idleBase + "/resources", owner, "POST", { app: "aa".repeat(32), resources: [] }))
      .status,
    200,
  );
  assert.equal((await call(idleBase + "/open", owner, "POST")).status, 200);
  idleRoom.access = identity("idleRoom-guest", "anonymous");
  assert.equal((await call(idleBase+"/members",idleRoom.access,"POST",{capability:idleRoom.capability})).status,200);
  const idleStub = namespace.get(namespace.idFromName(documentName(idleRoom.room)));
  const idleTest = (path: string, body = {}) =>
    idleStub.fetch("https://test/__test/" + path, { method: "POST", body: JSON.stringify(body) });
  await idleTest("alarm");
  assert.equal((await call(idleBase, idleRoom.access)).status, 200, "Recent room remains Live");
  await idleTest("sql", { sql: "UPDATE meta SET value='0' WHERE key='last_active'" });
  await idleTest("alarm");
  assert.equal((await call(idleBase, owner)).status,200,"persistent document never idle expires");
  console.log("Passed: opaque fold-only compaction, the base limit, blob cleanup, socket quotas/refusals and persistent retention.");
}

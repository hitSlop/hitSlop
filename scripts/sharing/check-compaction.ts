/** Qualification against the real Rust relay. The wrapper exists only in Miniflare:
 * no test route, clock override or storage bypass is compiled into the Worker. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import type { Miniflare } from "miniflare";

export function relayHarness(source: string) {
  return `import relay, {SlopRoom as Room, SharingAccount} from ${JSON.stringify(source)};
export { SharingAccount }; export default relay;
export class SlopRoom {
  constructor(state, env) { this.state = state; this.inner = new Room(state, env); }
  async fetch(req) {
    const path = new URL(req.url).pathname;
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
    return await this.inner.fetch(req);
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
const vv = (n: number) => (n ? [2, 1, ...uint(n), 2, 5] : [1, 2, 5]);
const hello = (n = 0, protocol = 1) =>
  new Uint8Array([0, protocol, ...Array.from({ length: 32 }, () => 0xaa), ...vv(n)]);
const push = (id: number, start: number, end: number, bytes = new Uint8Array([7])) =>
  new Uint8Array([
    4,
    ...uint(id),
    1,
    1,
    ...uint(start),
    ...uint(end),
    ...uint(bytes.length),
    ...bytes,
  ]);
function decode(bytes: ArrayBuffer) {
  const b = new Uint8Array(bytes);
  let pos = 0;
  const read = () => {
    let n = 0,
      scale = 1;
    for (;;) {
      const v = b[pos++]!;
      n += (v & 127) * scale;
      if (!(v & 128)) return n;
      scale *= 128;
    }
  };
  const version = () => {
    const v = [];
    for (let count = read(); count; count--) v.push([read(), read()]);
    return v;
  };
  const tag = read();
  if (tag === 1) return { tag, write: read(), version: version() };
  if (tag === 5) return { tag, id: read(), status: read() };
  if (tag === 6) return { tag, code: read() };
  if (tag === 2) {
    const records = [];
    for (let count = read(); count; count--) {
      const spans = [];
      for (let count = read(); count; count--) spans.push([read(), read(), read()]);
      const size = read();
      const bytes = b.slice(pos, pos + size);
      pos += size;
      records.push({ spans, bytes });
    }
    return { tag, records };
  }
  assert.equal(tag, 3);
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
      await call("rooms", identity("anonymous-owner", "anonymous"), "POST", {
        document: "00000000-0000-4000-a000-000000000002",
      })
    ).status,
    401,
  );
  const namespace = await worker.getDurableObjectNamespace("ROOMS", "relay");
  const bucket = await worker.getR2Bucket("SEEDS", "relay");
  const created = await call("rooms", owner, "POST", {
    document: "00000000-0000-4000-a000-000000000001",
  });
  assert.equal(created.status, 200);
  const room = await created.json();
  const base = "rooms/" + room.room;
  const stub = namespace.get(namespace.idFromName(room.room));
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
  let request = 0;
  const body = new Uint8Array(2_000_000).fill(9);
  const compact = (end: number, data = body, extra: Record<string, string> = {}) =>
    call(base + "/compact", room.capability, "POST", data, {
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
  assert.equal((await (await call(base, room.capability)).json()).compact, undefined);
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
  assert.equal((await compact(4, new Uint8Array((16 << 20) + 1))).status, 413);
  let stage = "initial backfill";
  const sockets: any[] = [];
  async function socket(ip = "203.0.113.1") {
    const response = await call(base + "/sync", room.capability, "GET", undefined, {
      Upgrade: "websocket",
      "Sec-WebSocket-Protocol": "hitslop.sync.v1",
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
        if (frame.tag === 3) return frames;
      }
    };
    return { ws, next, joined, closed, messages };
  }
  assert.equal((await (await call(base, room.capability)).json()).compact, true);
  stage = "connected";
  const connected = await socket();
  await connected.joined(4);
  await sql("DELETE FROM meta WHERE key='root'");
  assert.equal((await compact(3)).status, 400, "Legacy room needs a root header");
  assert.equal(
    (await compact(3, body, { "X-Sharing-Root": '[["0000000000000002",5]]' })).status,
    204,
    "Install partial base",
  );
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
  const archive = (await bucket.list({ prefix: `rooms/${room.room}/compacted/` })).objects;
  assert.equal(archive.length, 1);
  const archived = await bucket.get(archive[0]!.key);
  assert(archived && archived.size > 3 << 20);
  assert(Number(archived.customMetadata?.expires) > Date.now());
  await bucket.put(`rooms/${room.room}/compacted/expired`, new Uint8Array([0]), {
    customMetadata: { expires: "0" },
  });
  await sql("UPDATE meta SET value='0' WHERE key='last_active'");
  await test("alarm");
  assert.equal(
    await bucket.head(`rooms/${room.room}/compacted/expired`),
    null,
    "Alarm sweeps expired archives",
  );
  assert.equal((await call(base, room.capability)).status, 200, "Open sockets prevent idle expiry");
  await sql("UPDATE meta SET value=? WHERE key='last_active'", [String(Date.now())]);
  stage = "behind base";
  const behind = await socket("203.0.113.2");
  const frames = await behind.joined();
  assert.deepEqual(
    frames.filter((f) => f.tag === 2).map((f) => f.records.map((r: any) => r.bytes.length)),
    [[body.length], [1 << 20]],
    "Large base first and alone",
  );
  stage = "current backfill";
  const current = await socket("203.0.113.3");
  assert.equal((await current.joined(4)).filter((f) => f.tag === 2).length, 0);
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
  assert.equal((await compact(4, body, { "X-Sharing-Root": "invalid-but-ignored" })).status, 204);
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
  const secondArchive = await bucket.get(`rooms/${room.room}/compacted/2`);
  assert(secondArchive);
  const secondBytes = new Uint8Array(await secondArchive.arrayBuffer());
  const firstLength = new DataView(secondBytes.buffer).getUint32(0);
  assert(firstLength > body.length, "Previous base is archived first");
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
    (await (await call(base, room.capability)).json()).compact,
    true,
    "Row threshold triggers compaction with zero update bytes",
  );
  stage = "row quota";
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
      4,
      ...uint(id),
      1,
      1,
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
  assert(!budget.messages.some((m) => m.tag === 6));
  const countBudget = await socket("203.0.113.21");
  await countBudget.joined(1);
  await test("freeze-windows", { pushes: 1799 });
  stage = "push count budget";
  countBudget.ws.send(push(1800, 0, 1));
  assert.equal((await countBudget.next()).status, 0);
  countBudget.ws.send(push(1801, 0, 1));
  assert.equal(await countBudget.closed, 1013);
  assert(!countBudget.messages.some((m) => m.tag === 6));
  const resetBudget = await socket("203.0.113.24");
  await resetBudget.joined(1);
  await test("freeze-windows", { window_start: 0, bytes: 8 << 20, pushes: 1800 });
  resetBudget.ws.send(push(1802, 0, 1));
  assert.equal((await resetBudget.next()).status, 0, "Old minute budgets reset after hibernation");
  const perIP = [];
  for (let n = 0; n < 8; n++) perIP.push(await socket("203.0.113.20"));
  assert.equal(
    (
      await call(base + "/sync", room.capability, "GET", undefined, {
        Upgrade: "websocket",
        "Sec-WebSocket-Protocol": "hitslop.sync.v1",
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
  assert.equal((await bucket.list({ prefix: `rooms/${room.room}/compacted/` })).objects.length, 0);
  assert.equal((await sql("SELECT count(*) AS n FROM records"))[0].n, 0);
  assert.equal((await sql("SELECT count(*) AS n FROM chunks"))[0].n, 0);
  assert.deepEqual(
    (await sql("SELECT key FROM meta ORDER BY key")).map((r) => r.key),
    ["ended_at", "phase"],
  );
  assert.equal((await call(base, room.capability)).status, 410);
  await sql("UPDATE meta SET value='0' WHERE key='ended_at'");
  await test("alarm");
  assert.equal((await call(base, room.capability)).status, 404);
  assert.equal(
    await (await call("account/rooms/00000000-0000-4000-a000-000000000001", owner)).json(),
    null,
  );
  const idleRoom = await (
    await call("rooms", owner, "POST", { document: "00000000-0000-4000-a000-000000000003" })
  ).json();
  const idleBase = "rooms/" + idleRoom.room;
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
  const idleStub = namespace.get(namespace.idFromName(idleRoom.room));
  const idleTest = (path: string, body = {}) =>
    idleStub.fetch("https://test/__test/" + path, { method: "POST", body: JSON.stringify(body) });
  await idleTest("alarm");
  assert.equal((await call(idleBase, idleRoom.capability)).status, 200, "Recent room remains Live");
  await idleTest("sql", { sql: "UPDATE meta SET value='0' WHERE key='last_active'" });
  await idleTest("alarm");
  assert.equal((await call(idleBase, idleRoom.capability)).status, 410, "Idle room ends");
  assert.equal(
    await (await call("account/rooms/00000000-0000-4000-a000-000000000003", owner)).json(),
    null,
    "Idle expiry releases account index and quota",
  );
  console.log(
    "Passed: compaction conflicts/bounds/hash, archive/base/backfill, quota relief and row cap, push budget, permanent/transient close policy, Stop and tombstone purge.",
  );
}

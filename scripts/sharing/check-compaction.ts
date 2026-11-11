/** Qualification against the real Rust relay. The wrapper exists only in Miniflare:
 * no test route, clock override or storage bypass is compiled into the Worker. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import type { Miniflare } from "miniflare";

export const documentName = (id: string) => id;
export const objectKey = (id: string, suffix: string) => `${id}/${suffix}`;

export function relayHarness(source: string) {
  return `import relay, {CloudDocument as Room, DocumentAccount as Account} from ${JSON.stringify(source)};
export default relay;
export class DocumentAccount {
  constructor(state,env){this.state=state;this.inner=new Account(state,env);this.heldRegistrations=0;}
  async fetch(req){
    const path = new URL(req.url).pathname;
    if (path === '/__test/sql') {
      const {sql,args=[]} = await req.json();
      return Response.json(this.state.storage.sql.exec(sql,...args).toArray());
    }
    if (path === '/__test/fail-descriptions') { this.failDescriptions = (await req.json()).enabled; return new Response('ready'); }
    if (path === '/internal/describe' && this.failDescriptions) return new Response('retry',{status:503});
    // Holds the next member registration until released: a document awaiting it lets other requests in.
    if (path === '/__test/hold-registration') {
      this.registration = new Promise(resolve => this.releaseRegistration = resolve);
      return new Response('ready');
    }
    if (path === '/__test/held-registrations') return Response.json(this.heldRegistrations);
    if (path === '/__test/release-registration') { this.releaseRegistration?.(); return new Response('ready'); }
    // The next registration is stored, then its reply is held until finished, or replaced by a failure.
    if (path === '/__test/hold-registration-response') {
      this.registrationResponse = new Promise(resolve => this.finishRegistrationResponse = resolve);
      return new Response('ready');
    }
    if (path === '/__test/held-registration-responses') return Response.json(this.heldRegistrationResponses ?? 0);
    if (path === '/__test/finish-registration-response') { this.finishRegistrationResponse?.(); return new Response('ready'); }
    if (path === '/__test/fail-registration-response') { this.failRegistrationResponse = true; return new Response('ready'); }
    // Releases fail while enabled: refused, or committed with the reply lost.
    if (path === '/__test/release-failure') {
      const {enabled,committed=false} = await req.json();
      this.failRelease = enabled; this.commitFailedRelease = committed;
      return new Response('ready');
    }
    if (path === '/internal/release' && this.failRelease) {
      if (this.commitFailedRelease) await this.inner.fetch(req);
      return new Response('unavailable',{status:503});
    }
    if (path === '/internal/member' && this.registration) {
      const registration = this.registration;
      this.registration = undefined;
      this.heldRegistrations++;
      await registration;
    }
    try {
      const response = await this.inner.fetch(req);
      if (path === '/internal/member' && this.registrationResponse) {
        const held = this.registrationResponse;
        this.registrationResponse = undefined;
        this.heldRegistrationResponses = (this.heldRegistrationResponses ?? 0) + 1;
        await held;
      }
      if (path === '/internal/member' && this.failRegistrationResponse) {
        this.failRegistrationResponse = false;
        return new Response('reply lost',{status:500});
      }
      return response;
    } catch(error){console.error('Test account failure:',String(error));throw error;}
  }
  webSocketMessage(ws,message){return this.inner.webSocketMessage(ws,message);}
  webSocketClose(ws,code,reason,clean){return this.inner.webSocketClose(ws,code,reason,clean);}
  webSocketError(ws,error){return this.inner.webSocketError(ws,error);}
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
    if (path === '/__test/hold-body') { this.bodyHold = new Promise(resolve=>this.releaseBody=resolve); return new Response('ready'); }
    if (path === '/__test/body-waiting') return Response.json(!!this.bodyWaiting);
    if (path === '/__test/release-body') { this.releaseBody?.(); return new Response('ready'); }
    if (req.headers.get('X-Test-Hold-Body') === '1' && req.body) {
      const reader=req.body.getReader();
      const body=new ReadableStream({pull:async controller=>{
        this.bodyWaiting=true;
        await this.bodyHold;
        this.bodyWaiting=false;
        const next=await reader.read();
        if(next.done) controller.close(); else controller.enqueue(next.value);
      }},{highWaterMark:0});
      req=new Request(req.url,{method:req.method,headers:req.headers,body});
    }
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
      const values = args.map(arg => arg?.blob ? new Uint8Array(arg.blob) : arg);
      const rows = this.state.storage.sql.exec(sql,...values).toArray();
      return Response.json(rows.map(row => Object.fromEntries(Object.entries(row).map(([key,value]) =>
        [key,value instanceof ArrayBuffer ? Array.from(new Uint8Array(value)) : value]))));
    }
    if (path === '/__test/fixture') {
      const {count,size=1} = await req.json();
      this.state.storage.transactionSync(() => {
        this.state.storage.sql.exec('DELETE FROM records WHERE seq>0; DELETE FROM chunks WHERE seq>0');
        for (let n=1;n<=count;n++) {
          this.state.storage.sql.exec('INSERT INTO records(seq,spans,size) VALUES(?,?,?)', n, (()=>{const bytes=new Uint8Array(16), view=new DataView(bytes.buffer);view.setBigUint64(0,1n);view.setUint32(8,n-1);view.setUint32(12,n);return bytes;})(),size);
          for(let part=0;part*1800000<size;part++) this.state.storage.sql.exec('INSERT INTO chunks VALUES(?,?,?)',n,part,new Uint8Array(Math.min(1800000,size-part*1800000)).fill(n%256));
        }
        this.state.storage.sql.exec("DELETE FROM heads");
        this.state.storage.sql.exec("INSERT INTO heads VALUES(?,?),(?,?)",'0000000000000001',Math.max(count,JSON.parse(this.state.storage.sql.exec("SELECT base_version AS value FROM document_state").toArray()[0]?.value ?? '[]').find(([p])=>p==='0000000000000001')?.[1] ?? 0),'0000000000000002',5);
        this.state.storage.sql.exec("UPDATE document_state SET update_bytes=?,update_rows=?",count*size,count);
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
    if (path === '/__test/hold-alarms') { this.holdAlarms=(await req.json()).enabled; return new Response('ready'); }
    if (path === '/__test/alarm') return await this.inner.alarm();
    if (path === '/__test/alarm-time') return Response.json(await this.state.storage.getAlarm());
    // Resets the object as a crash would: in-flight requests fail and memory is lost.
    if (path === '/__test/hold-library') { this.libraryHold = new Promise(resolve => this.releaseLibrary = resolve); return new Response('ready'); }
    if (path === '/__test/library-waiting') return Response.json(!!this.libraryWaiting);
    if (path === '/__test/release-library') { this.releaseLibrary?.(); this.libraryHold=undefined; return new Response('ready'); }
    if (path.endsWith('/library') && this.libraryHold) { this.libraryWaiting=true; await this.libraryHold; this.libraryWaiting=false; }
    if (path === '/__test/library-unavailable') { this.libraryUnavailable = (await req.json()).enabled; return new Response('ready'); }
    if (path.endsWith('/library') && this.libraryUnavailable) return new Response('unavailable',{status:503});
    if (path === '/__test/reset') this.state.abort('Test reset');
    try { return await this.inner.fetch(req); } catch(error) { console.error('Test document failure:',String(error)); throw error; }
  }
  alarm() { return this.holdAlarms ? new Response('held') : this.inner.alarm(); }
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
const spanBlob = (spans: [string, number, number][]) => {
  const bytes = new Uint8Array(16 * spans.length), view = new DataView(bytes.buffer);
  spans.forEach(([peer, start, end], i) => {
    view.setBigUint64(i * 16, BigInt("0x" + peer));
    view.setUint32(i * 16 + 8, start);
    view.setUint32(i * 16 + 12, end);
  });
  return {blob: Array.from(bytes)};
};
const storedSpans = (bytes: number[]) => {
  const view = new DataView(new Uint8Array(bytes).buffer);
  return Array.from({length: bytes.length / 16}, (_, i) => [
    view.getBigUint64(i * 16).toString(16).padStart(16, "0"),
    view.getUint32(i * 16 + 8), view.getUint32(i * 16 + 12),
  ]);
};
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
  const setHeads = async (text: string) => {
    await sql("DELETE FROM heads");
    for (const [peer, end] of JSON.parse(text)) await sql("INSERT INTO heads VALUES(?,?)", [peer, end]);
  };
  const heads = async () => JSON.stringify((await sql("SELECT peer,end FROM heads ORDER BY peer")).map(r=>[r.peer,r.end]));
  const checkUsage = async () => {
    const [actual] = await sql("SELECT coalesce(sum(size),0) AS bytes,count(*) AS rows FROM records WHERE seq>0");
    const [stored] = await sql("SELECT update_bytes AS bytes,update_rows AS rows FROM document_state");
    assert.deepEqual(stored, actual, "Running totals match accepted updates");
  };
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
  assert.equal((await call(base + "/open", owner, "POST", { seedSha256: hash(seed) })).status, 200);
  room.access = identity("room-member");
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
  await setHeads('[["0000000000000001",4],["0000000000000002",5]]');
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
  await test("hold-body");
  const streamingBase = compact(5, new Uint8Array([1]), {"X-Test-Hold-Body":"1"});
  try {
    for (let n=0;!(await (await test("body-waiting")).json());n++) {
      assert(n<100, "Compaction starts reading its streamed body");
      await new Promise(resolve=>setTimeout(resolve,10));
    }
    const blob = new Uint8Array([31,32,33]);
    const independent = Promise.all([
      call(base + "/blob/" + hash(blob), room.access, "PUT", blob, {"Content-Length": String(blob.length)}), test("alarm"),
    ]);
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const replies = await Promise.race([independent,new Promise<never>((_,reject)=>{
        timer=setTimeout(()=>reject(new Error("Streaming compaction blocked blob uploads or the alarm")),2000);
      })]);
      assert.equal(replies[0].status,200);
      assert.equal(replies[1].status,200);
    } finally { clearTimeout(timer); }
  } finally { await test("release-body"); }
  assert.equal((await streamingBase).status,409);
  stage = "bounded catch-up turns";
  await test("hold-alarms", {enabled: true});
  const sliced = await socket("203.0.113.29");
  sliced.ws.send(hello());
  assert.equal((await sliced.next()).tag, 2);
  assert.equal(sliced.messages.length, 0, "Hello queues catch-up without reading payloads");
  await test("alarm");
  assert.equal((await sliced.next()).records[0].bytes.length, 1 << 20);
  assert.deepEqual((await sql("SELECT cursor FROM catchups ORDER BY rowid")).map(r=>r.cursor), [1],
    "One turn stops at its byte budget");
  assert.equal((await compact(3)).status, 204, "Compaction may advance between catch-up turns");
  await test("alarm");
  assert.equal((await sliced.next()).records[0].bytes.length, body.length, "Cursor restarts at a replaced base");
  await test("alarm");
  assert.equal((await sliced.next()).records[0].bytes.length, 1 << 20);
  await test("alarm");
  assert.equal((await sliced.next()).tag, 4);
  sliced.ws.close();
  await sliced.closed;
  const pair = await Promise.all([socket("203.0.113.30"),socket("203.0.113.31")]);
  for (const client of pair) {
    client.ws.send(hello());
    assert.equal((await client.next()).tag, 2);
  }
  await test("alarm");
  assert.deepEqual((await sql("SELECT cursor FROM catchups ORDER BY cursor")).map(r=>r.cursor), [-1,0],
    "The room shares one turn budget across reconnecting sockets");
  for (const client of pair) client.ws.close();
  await Promise.all(pair.map(client=>client.closed));
  await test("hold-alarms", {enabled: false});
  // Restore the un-compacted fixture for the fold qualification below.
  await sql("DELETE FROM records WHERE seq=0; DELETE FROM chunks WHERE seq=0; UPDATE document_state SET base_version=NULL");
  await fixture(4, 1 << 20);
  assert.equal((await (await call(base, room.access)).json()).compact, true);
  stage = "connected";
  const connected = await socket();
  await connected.joined(4);
  stage = "repeated Hello";
  const repeated = await socket("203.0.113.28");
  await repeated.joined(4);
  repeated.ws.send(hello());
  assert.equal((await repeated.next()).code, 1, "A second Hello must be a protocol refusal, never another backfill");
  assert.equal(await repeated.closed, 1008);
  // The seed upload stored the document's root with its heads. A live document without it
  // is a fault in the document, never something a member supplies: nothing is installed.
  const [storedRoot] = await sql("SELECT root AS value FROM document_state");
  assert.equal(storedRoot.value, '[["0000000000000002",5]]', "The seed upload stores the root");
  await sql("UPDATE document_state SET root=NULL");
  const rootless = await compact(3);
  assert.equal(rootless.status, 500, "Compaction without a stored root is an internal failure");
  assert.equal((await rootless.json()).code, "unavailable");
  assert.equal((await sql("SELECT count(*) AS n FROM records WHERE seq=0"))[0].n, 0, "A rootless document installs no base");
  assert.deepEqual(await sql("SELECT root,base_version FROM document_state WHERE root IS NOT NULL OR base_version IS NOT NULL"), [], "A refused base records no versions");
  await sql("UPDATE document_state SET root=?", [storedRoot.value]);
  assert.equal((await compact(3)).status, 204, "Install partial base");
  await checkUsage();
  assert.equal(connected.messages.length, 0, "Base is never broadcast");
  const rows = await sql("SELECT seq,spans FROM records ORDER BY seq");
  assert.deepEqual(
    rows.map((r) => r.seq),
    [0, 4],
  );
  assert.deepEqual(
    storedSpans(rows[0].spans),
    [["0000000000000001", 0, 3]],
    "Seed-only peers produce no empty spans",
  );
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
    spanBlob([
      ["0000000000000001", 3, 4],
      ["0000000000000002", 5, 6],
    ]),
  ]);
  await setHeads('[["0000000000000001",4],["0000000000000002",6]]');
  assert.equal((await compact(2)).status, 409, "Nonmonotonic base");
  assert.equal((await compact(3)).status, 409, "Base must strictly advance");
  await sql("UPDATE document_state SET update_rows=update_rows+1");
  assert.equal((await compact(4)).status, 204, "A later base advances from the previous one");
  await checkUsage();
  assert.deepEqual(
    (await sql("SELECT seq FROM records ORDER BY seq")).map((row) => row.seq),
    [0, 5],
    "A record with any uncovered span is retained whole",
  );
  assert.equal(
    await heads(),
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
  await setHeads('[["0000000000000001",16],["0000000000000002",5]]');
  assert.equal((await compact(16)).status, 204);
  stage = "quota freed";
  limited.ws.send(push(11, 16, 17));
  assert.equal((await limited.next()).status, 0, "Quota freed");
  await checkUsage();
  stage = "quota echo";
  await limited.next(); // harmless echo
  const sequences = await sql("SELECT seq FROM records ORDER BY seq");
  assert.equal(sequences.length, 2);
  assert.equal(sequences[0].seq, 0, "Push preserves the base");
  assert(sequences[1].seq > 0, "Normal seq allocation remains positive");
  await fixture(32768, 0);
  await checkUsage();
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
  await setHeads('[["0000000000000001",16],["0000000000000002",5]]');
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
  const wrongApp = await socket("203.0.113.27");
  const mismatch = hello(); mismatch[2] = mismatch[2]! ^ 1;
  wrongApp.ws.send(mismatch);
  assert.equal((await wrongApp.next()).code, 1, "An unsupported app digest tells the client to update");
  const transient = await socket("203.0.113.8");
  await sql("UPDATE heads SET peer='broken' WHERE peer='0000000000000001'");
  transient.ws.send(hello());
  assert.equal(await transient.closed, 1011);
  assert.equal(transient.messages.length, 0);
  await sql("DELETE FROM heads");
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
  assert.equal((await sql("SELECT count(*) AS n FROM records"))[0].n, 0);
  assert.equal((await sql("SELECT count(*) AS n FROM chunks"))[0].n, 0);
  assert.equal((await call(base,owner)).status,410);
  await test("alarm");
  assert.equal((await call(base,owner)).status,410,"deletion tombstone preserves UUID ownership");
  const [tombstone] = await sql("SELECT * FROM document_state");
  assert.equal(tombstone.phase,"ended");
  assert.equal(tombstone.document,room.room);
  assert.ok(tombstone.owner_uid);
  for (const [key,value] of Object.entries(tombstone)) {
    if (!["id","owner_uid","document","phase"].includes(key)) assert.equal(value,null,key+" is purged");
  }
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
  assert.equal((await call(idleBase + "/open", owner, "POST", { seedSha256: hash(seed) })).status, 200);
  idleRoom.access = identity("idleRoom-member");
  assert.equal((await call(idleBase+"/members",idleRoom.access,"POST",{capability:idleRoom.capability})).status,200);
  const idleStub = namespace.get(namespace.idFromName(documentName(idleRoom.room)));
  const idleTest = (path: string, body = {}) =>
    idleStub.fetch("https://test/__test/" + path, { method: "POST", body: JSON.stringify(body) });
  await idleTest("alarm");
  assert.equal((await call(idleBase, idleRoom.access)).status, 200, "Recent room remains Live");
  await idleTest("alarm");
  assert.equal((await call(idleBase, owner)).status,200,"persistent document never idle expires");
  console.log("Passed: one Hello per socket, budgeted catch-up cursors across compaction, running usage totals, binary spans and uploads/alarms during streamed compaction.");
  console.log("Passed: opaque fold-only compaction, the base limit, blob cleanup, socket quotas/refusals and persistent retention.");
}

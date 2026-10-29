/** Real Rust WASM + SQLite DO/R2 qualification, with locally signed identities.
 * Only the local runtime's certificate service is substituted. Production auth
 * has no test key, token bypass, configurable certificate URL or asserted UID path.
 * Run through `bun run verify sharing` after installing worker-build (README). */
import assert from "node:assert/strict";
import { checkCompaction, relayHarness } from "./check-compaction";
import host from "../../apps/sharing/http";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtemp, rm, readFile, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Miniflare, Response as LocalResponse } from "miniflare";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { runInNewContext } from "node:vm";
const repository = resolve(fileURLToPath(new URL("../..", import.meta.url)));
// Miniflare's synchronous proxy uses a Node worker-thread HTTP dispatcher. Run
// qualification in Node: Bun's undici shim intermittently resets proxy requests
// under sustained socket/storage traffic. Bun remains the verification entry point.
if (process.versions.bun) {
  const output = join(repository, ".tinys/sharing/check-accounts.mjs");
  await mkdir(join(repository, ".tinys/sharing"), { recursive: true });
  const built = await Bun.build({ entrypoints: [fileURLToPath(import.meta.url)], target: "node",
    external: ["miniflare", "typescript"], define: { "import.meta.url": JSON.stringify(import.meta.url) } });
  if (!built.success) throw new Error(built.logs.join("\n"));
  await writeFile(output, await built.outputs[0]!.text());
  const child = Bun.spawn(["node", output], { stdin: "inherit", stdout: "inherit", stderr: "inherit" });
  process.exit(await child.exited);
}
import { SharingHost } from "../../packages/tinys/src/wire/constants.generated";
import type { AccountRoom, CreatedRoom, RoomInfo } from "../../packages/tinys/src/wire/sharing.generated";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const root = await mkdtemp(join(tmpdir(), "tinys-account-relay-"));
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const pem = publicKey.export({ type: "pkcs1", format: "pem" }).toString();
const uid = (name: string, provider = "apple.com") => {
  const now = Math.floor(Date.now() / 1000);
  const parts = [{ alg: "RS256", kid: "local-runtime-only" }, { sub: name, aud: "hitslopapp", iss: "https://securetoken.google.com/hitslopapp", iat: now, auth_time: now, exp: now + 3600, firebase: { sign_in_provider: provider }, jti: crypto.randomUUID() }]
    .map(value => Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return parts + "." + sign("RSA-SHA256", Buffer.from(parts), privateKey).toString("base64url");
};
const a = uid("creator-a"), secondSession = uid("creator-a"), b = uid("creator-b");
const document = "00000000-0000-4000-8000-000000000001";
const seed = Buffer.alloc(512, 42), asset = Buffer.from("export default {};");
const app = "a".repeat(64);
const relayConfig = ts.parseConfigFileTextToJson("wrangler.jsonc", await readFile(join(repository, "apps/sharing/wrangler.jsonc"), "utf8")).config as {
  ratelimits: { name: string; simple: { limit: number; period: 60 } }[];
};
const harness = join(root, "relay-harness.mjs");
await writeFile(harness, relayHarness(join(repository, "generated/sharing/relay/index.js")));
function runtime(roomsAvailable = true) {
  return new Miniflare({
    durableObjectsPersist: join(root, "objects"), r2Persist: join(root, "r2"),
    workers: [{
      name: "relay", modules: true, scriptPath: harness, modulesRoot: "/",
      compatibilityDate: "2025-11-05", modulesRules: [{ type: "ESModule", include: ["**/*.js"], fallthrough: true }, { type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true }],
      bindings: { BROWSER_ORIGIN: "https://share.tinys.click", FIREBASE_PROJECT: "hitslopapp", CREATION_ENABLED: "1" },
      durableObjects: { ...(roomsAvailable ? { ROOMS: { className: "SlopRoom", useSQLite: true } } : {}), ACCOUNTS: { className: "SharingAccount", useSQLite: true } },
      r2Buckets: ["SEEDS"],
      analyticsEngineDatasets: { METRICS: { dataset: "hitslop_sharing" } },
      ratelimits: Object.fromEntries(relayConfig.ratelimits.map(({ name, simple }) => [name, { simple: name === "ACCOUNT_CREATE_LIMIT" ? { ...simple, limit: 1000 } : simple }])),
      outboundService: "certificates",
    }, {
      name: "certificates", modules: true,
      script: `export default { fetch(request) {
        if (request.url !== "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com") return new Response(null,{status:502});
        return Response.json(${JSON.stringify({ "local-runtime-only": pem })},{headers:{"Cache-Control":"public,max-age=3600"}});
      } }`,
    }],
  });
}
let worker = runtime();
async function call(path: string, token: string | undefined, method = "GET", body?: unknown, headers: Record<string, string> = {}) {
  const binary = body instanceof Uint8Array;
  const response = await worker.dispatchFetch("https://share.tinys.click/v1/" + path, {
    method, headers: { ...headers, ...(token ? { Authorization: "Bearer " + token } : {}), ...(body === undefined ? {} : { "Content-Type": binary ? "application/octet-stream" : "application/json" }) },
    ...(body === undefined ? {} : { body: binary ? body : JSON.stringify(body) }),
  });
  if (response.status === 101) return response;
  const bytes = await response.arrayBuffer();
  return new LocalResponse(response.status === 204 ? null : bytes, {status: response.status, headers: response.headers});
}
async function json<T>(path: string, token: string, method = "GET", body?: unknown): Promise<T> {
  const response = await call(path, token, method, body);
  assert.equal(response.status, 200, path + " must succeed");
  return await response.json() as T;
}
const seedHeaders = (bytes: Uint8Array) => ({
  "X-Sharing-SHA256": hash(bytes), "X-Sharing-App": app, "X-Sharing-Version": "[]",
});
try {
  await checkCompaction(worker, call, uid("compaction-owner"), uid);
  const unused = { async fetch(): Promise<Response> { throw new Error("Static auth/AASA routes must not use assets or relay"); } };
  const callbackURL = "https://share.tinys.click/auth/apple";
  const callback = await host.fetch(new Request(callbackURL), { ASSETS: unused, RELAY: unused });
  assert.equal(callback.status, 200);
  assert.equal(callback.headers.get("Content-Type"), "text/html; charset=utf-8");
  assert.equal(callback.headers.get("Cache-Control"), "no-store");
  assert.equal(callback.headers.get("Referrer-Policy"), "no-referrer");
  const callbackPage = await callback.text();
  assert.ok(callbackPage.includes("tinys://auth/apple"));
  const script = /<script>([\s\S]*?)<\/script>/.exec(callbackPage)![1]!;
  const digest = createHash("sha256").update(script).digest("base64");
  assert.equal(callback.headers.get("Content-Security-Policy"),
    `default-src 'none'; script-src 'sha256-${digest}'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`);
  for (const fragment of ["#state=example&id_token=a.b.c&code=a%2Bb%26c", "#state=example&error=user_cancelled_authorize", ""]) {
    let destination: string | undefined;
    runInNewContext(script, { location: { hash: fragment, replace(url: string) { destination = url; } } });
    assert.equal(destination, fragment ? "tinys://auth/apple" + fragment : undefined);
  }
  const callbackHead = await host.fetch(new Request(callbackURL, { method: "HEAD" }), { ASSETS: unused, RELAY: unused });
  assert.equal(callbackHead.status, 200);
  assert.equal(await callbackHead.text(), "");
  assert.equal((await host.fetch(new Request(callbackURL, { method: "POST" }), { ASSETS: unused, RELAY: unused })).status, 405);
  assert.equal((await host.fetch(new Request(`https://copy.${SharingHost.domain}/auth/apple`), { ASSETS: unused, RELAY: unused })).status, 404);
  assert.equal((await host.fetch(new Request("https://other.example/auth/apple"), { ASSETS: unused, RELAY: unused })).status, 403);
  const associationURL = "https://share.tinys.click/.well-known/apple-app-site-association";
  const association = await host.fetch(new Request(associationURL), { ASSETS: unused, RELAY: unused });
  assert.equal(association.status, 200);
  assert.equal(association.headers.get("Content-Type"), "application/json");
  assert.equal(association.headers.get("Location"), null);
  assert.equal(association.headers.get("Cache-Control"), "public, max-age=3600");
  assert.deepEqual(await association.json(), {
    applinks: { details: [{ appIDs: ["78UAXU8QG8.com.hitslop.app"], components: [{ "/": "/join", comment: "Live sharing links" }] }] },
    appclips: { apps: ["78UAXU8QG8.com.hitslop.app.Clip"] },
  });
  const head = await host.fetch(new Request(associationURL, { method: "HEAD" }), { ASSETS: unused, RELAY: unused });
  assert.equal(head.status, 200);
  assert.equal(await head.text(), "");
  assert.equal((await host.fetch(new Request(associationURL, { method: "POST" }), { ASSETS: unused, RELAY: unused })).status, 405);
  assert.equal((await host.fetch(new Request(associationURL.replace("share.tinys.click", "copy.dotslop.com")), { ASSETS: unused, RELAY: unused })).status, 404);
  const join = await host.fetch(new Request("https://share.tinys.click/join"), {
    ASSETS: { async fetch(request: Request) { assert.equal(new URL(request.url).pathname, "/index.html"); return new Response("join"); } }, RELAY: unused,
  });
  assert.equal(await join.text(), "join");
  assert.equal(join.headers.get("Content-Security-Policy"), SharingHost.hostPolicy);
  assert.equal((await call("account", undefined, "DELETE")).status, 401);
  assert.equal((await call("account", "not-an-identity", "DELETE")).status, 401);
  assert.equal((await call("rooms", undefined, "POST", { document })).status, 401);
  for (const token of ["malformed", uid("untrusted").slice(0, -8) + "invalid"]) {
    assert.equal((await call("account/rooms/" + document, token)).status, 401);
  }
  const rooms = await Promise.all([1, 2].map(() => json<CreatedRoom>("rooms", a, "POST", { document })));
  const r = rooms[0]!;
  assert.equal(r.phase, "uploading");
  assert(r.room === rooms[1]!.room && r.capability === rooms[1]!.capability, "Concurrent create must be idempotent");
  assert.equal(await json("account/rooms/" + document, b), null);
  const other = await json<CreatedRoom>("rooms", b, "POST", { document });
  assert.notEqual(other.room, r.room);
  const base = "rooms/" + r.room;
  assert.equal((await call(base, b)).status, 403);
  assert.equal((await call(base, b, "DELETE")).status, 403);
  assert.equal((await call(base, r.capability, "DELETE")).status, 403);
  assert.equal((await call(base + "/open", r.capability, "POST")).status, 403);
  assert.equal((await call(base + "/open", a, "POST")).status, 409);
  assert.equal((await call(base, r.capability)).status, 409);
  assert.equal((await call(base + "/seed", r.capability, "PUT", seed, seedHeaders(seed))).status, 403);
  assert.equal((await call(base + "/seed", a, "PUT", seed, { ...seedHeaders(seed), "X-Sharing-SHA256": "f".repeat(64) })).status, 422);
  assert.equal((await call(base + "/seed", a, "PUT", seed, seedHeaders(seed))).status, 200);
  const replacement = Buffer.alloc(768, 43);
  assert.equal((await call(base + "/seed", secondSession, "PUT", replacement, seedHeaders(replacement))).status, 200);
  const info = await json<RoomInfo>(base, secondSession);
  assert.equal(info.seedHash, hash(replacement));
  assert.equal(info.seedSize, replacement.length);
  assert.equal(info.capability, r.capability);
  const manifest = { app, resources: [{ key: "ui.js", hash: hash(asset), size: asset.length, media_type: "text/javascript" }] };
  assert.equal((await call(base + "/resources", a, "POST", { app, resources: [] })).status, 200);
  assert.equal((await call(base + "/resources", a, "POST", manifest)).status, 200);
  assert.equal((await call(base + "/asset/" + hash(asset), r.capability, "PUT", asset)).status, 403);
  assert.equal((await call(base + "/open", a, "POST")).status, 409);
  assert.equal((await call(base + "/seed", r.capability)).status, 409);
  assert.equal((await call(base + "/asset/" + hash(asset), a, "PUT", asset)).status, 200);
  const live = await json<RoomInfo>(base + "/open", a, "POST");
  assert.equal(live.phase, "live");
  assert.equal((await json<CreatedRoom>("rooms", a, "POST", { document })).room, r.room);
  const guest = await json<RoomInfo>(base, r.capability);
  assert(!guest.capability, "Guest metadata must not return the owner capability field");
  assert.equal(guest.seedHash, hash(replacement));
  const downloaded = await call(base + "/seed", r.capability);
  assert.equal(downloaded.status, 200);
  assert.deepEqual(Buffer.from(await downloaded.arrayBuffer()), replacement);
  assert.equal((await call(base + "/resources", r.capability)).status, 200);
  assert.equal((await call("auth/probe", a)).status, 404);
  assert.equal((await call(base + "/stats", r.capability)).status, 404);
  assert.equal((await call("account/reserve", a, "POST", { document })).status, 404);
  const joined = await worker.dispatchFetch("https://share.tinys.click/v1/" + base + "/sync", {
    headers: { Upgrade: "websocket", Origin: "https://share.tinys.click", "Sec-WebSocket-Protocol": SharingHost.socketProtocol + ", cap." + r.capability },
  });
  assert.equal(joined.status, 101, "A capability guest can connect to the live room");
  joined.webSocket!.accept();
  joined.webSocket!.close();
  assert.equal((await call(base + "/seed", a, "PUT", seed, seedHeaders(seed))).status, 409);
  assert.equal((await call(base + "/resources", a, "POST", manifest)).status, 409);
  await worker.dispose();
  worker = runtime();
  const lookup = await json<AccountRoom>("account/rooms/" + document, secondSession);
  assert.equal(lookup.room, r.room);
  assert.equal((await json<RoomInfo>("rooms/" + lookup.room, secondSession)).capability, r.capability);
  assert.equal((await call(base, secondSession, "DELETE")).status, 200);
  assert.equal((await call(base, r.capability)).status, 410);
  assert.equal((await call(base + "/seed", r.capability)).status, 410);
  const next = await json<CreatedRoom>("rooms", a, "POST", { document });
  assert.notEqual(next.room, r.room, "Re-share creates a fresh random room");
  assert.equal((await json<AccountRoom>("account/rooms/" + document, a)).room, next.room);
  const rejoinedRoom = await json<CreatedRoom>("rooms", b, "POST", { document: "00000000-0000-4000-8000-000000000002" });
  const rejoinedBase = "rooms/" + rejoinedRoom.room;
  assert.equal((await call(rejoinedBase + "/seed", b, "PUT", seed, seedHeaders(seed))).status, 200);
  assert.equal((await call(rejoinedBase + "/resources", b, "POST", { app, resources: [] })).status, 200);
  assert.equal((await call(rejoinedBase + "/open", b, "POST")).status, 200);
  await call(rejoinedBase, b, "DELETE");
  const replacementRoom = await json<CreatedRoom>("rooms", b, "POST", { document: "00000000-0000-4000-8000-000000000002" });
  const replacementBase = "rooms/" + replacementRoom.room;
  assert.notEqual(replacementRoom.room, rejoinedRoom.room);
  assert.equal((await call(rejoinedBase, rejoinedRoom.capability)).status, 410);
  assert.equal((await call(replacementBase + "/seed", b, "PUT", seed, seedHeaders(seed))).status, 200);
  assert.equal((await call(replacementBase + "/resources", b, "POST", { app, resources: [] })).status, 200);
  assert.equal((await call(replacementBase + "/open", b, "POST")).status, 200);
  assert.equal((await json<RoomInfo>(replacementBase, replacementRoom.capability)).document,
    "00000000-0000-4000-8000-000000000002");
  const rejoined = await worker.dispatchFetch("https://share.tinys.click/v1/" + replacementBase + "/sync", {
    headers: { Upgrade: "websocket", "Sec-WebSocket-Protocol": SharingHost.socketProtocol, Authorization: "Bearer " + replacementRoom.capability },
  });
  assert.equal(rejoined.status, 101, "A guest can reconnect with the new room for the same document");
  rejoined.webSocket!.accept();
  rejoined.webSocket!.close();
  await call(replacementBase, b, "DELETE");
  // Hold an Uploading seed body across Stop, then complete its delayed write.
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  let started!: () => void;
  const ready = new Promise<void>(resolve => { started = resolve; });
  let firstChunk = true;
  const body = new ReadableStream({ async pull(controller) {
    if (firstChunk) {
      firstChunk = false;
      controller.enqueue(seed.subarray(0, 1));
      started();
    } else {
      await gate;
      controller.enqueue(seed.subarray(1));
      controller.close();
    }
  } }, { highWaterMark: 0 });
  const lateInit = { method: "PUT", headers: { Authorization: "Bearer " + a, ...seedHeaders(seed) }, body, duplex: "half" as const };
  const lateUpload = worker.dispatchFetch("https://share.tinys.click/v1/rooms/" + next.room + "/seed", lateInit);
  await ready;
  try {
    assert.equal((await call("rooms/" + next.room, a, "DELETE")).status, 200);
  } finally { release(); }
  assert.equal((await lateUpload).status, 410, "An upload spanning Stop must be refused");
  const bucket = await worker.getR2Bucket("SEEDS", "relay");
  assert.equal((await bucket.list({ prefix: next.room + "/" })).objects.length, 0, "Stop must remove late writes");
  assert.equal((await call("rooms/" + next.room + "/open", a, "POST")).status, 410);
  assert.equal((await call(base, a, "DELETE")).status, 410, "A cleaned tombstone no longer retains owner identity");
  await call("rooms/" + other.room, b, "DELETE");
  // Quota counts only rooms that have not ended; retrying an indexed room uses no slot.
  const quota = [] as CreatedRoom[];
  for (let n = 1; n <= 10; n++) {
    quota.push(await json<CreatedRoom>("rooms", a, "POST", { document: `00000000-0000-4000-9000-${String(n).padStart(12, "0")}` }));
  }
  assert.equal((await call("rooms", a, "POST", { document })).status, 429);
  assert.equal((await json<CreatedRoom>("rooms", secondSession, "POST", { document: "00000000-0000-4000-9000-000000000001" })).room, quota[0]!.room);
  await call("rooms/" + quota[0]!.room, a, "DELETE");
  const freed = await json<CreatedRoom>("rooms", a, "POST", { document });
  const liveBase = "rooms/" + freed.room;
  assert.equal((await call(liveBase + "/seed", a, "PUT", seed, seedHeaders(seed))).status, 200);
  assert.equal((await call(liveBase + "/resources", a, "POST", { app, resources: [] })).status, 200);
  assert.equal((await call(liveBase + "/open", a, "POST")).status, 200);
  // Uploading rooms can already hold private data and need the same purge as Live.
  assert.equal((await call("rooms/" + quota[1]!.room + "/seed", a, "PUT", seed, seedHeaders(seed))).status, 200);
  const unrelated = await json<CreatedRoom>("rooms", b, "POST", { document });
  const socket = await worker.dispatchFetch("https://share.tinys.click/v1/" + liveBase + "/sync", {
    headers: { Upgrade: "websocket", "Sec-WebSocket-Protocol": SharingHost.socketProtocol, Authorization: "Bearer " + freed.capability },
  });
  assert.equal(socket.status, 101);
  socket.webSocket!.accept();
  const closed = new Promise<void>(resolve => socket.webSocket!.addEventListener("close", () => resolve()));
  const caughtUp = new Promise<void>(resolve => socket.webSocket!.addEventListener("message", event => {
    if (new Uint8Array(event.data as ArrayBuffer)[0] === 3) resolve();
  }));
  socket.webSocket!.send(new Uint8Array([0,1,...Array.from({length:32},()=>0xaa),0]));
  await caughtUp;
  // Each endpoint has an independent per-IP budget; changing IP gets a fresh one.
  for (const [route, method, body, headers] of [
    [liveBase, "GET", undefined, {}],
    [liveBase + "/seed", "GET", undefined, {}],
    [liveBase + "/sync", "GET", undefined, { Upgrade: "websocket", "Sec-WebSocket-Protocol": SharingHost.socketProtocol }],
    [liveBase + "/blob/" + hash(seed), "PUT", seed, {}],
    [liveBase + "/compact", "POST", seed, {}],
  ] as const) {
    let limited = false;
    // A burst can straddle one fixed minute boundary in the local limiter.
    const attempts = 2 * Math.max(...relayConfig.ratelimits.map(limit => limit.simple.limit)) + 1;
    for (let n = 0; n < attempts; n++) {
      // Refused socket credentials exercise the edge budget without filling the room.
      const token = route.endsWith("/sync") ? "f".repeat(64) : freed.capability;
      const response = await call(route, token, method, body, { ...headers, "CF-Connecting-IP": "192.0.2.10" });
      if (response.status === 101) { response.webSocket!.accept(); response.webSocket!.close(); }
      if (response.status === 429) {
        assert.equal((await response.json() as { code: string }).code, "limited");
        limited = true;
        break;
      }
    }
    assert(limited, route + " must enforce its guest budget");
    const fresh = await call(route, route.endsWith("/sync") ? "f".repeat(64) : freed.capability, method, body, { ...headers, "CF-Connecting-IP": "192.0.2.11" });
    assert.notEqual(fresh.status, 429, "Budgets are per client IP");
    if (fresh.status === 101) { fresh.webSocket!.accept(); fresh.webSocket!.close(); }
  }
  assert.equal((await call("account", a, "DELETE")).status, 200);
  assert.equal((await call(liveBase, freed.capability)).status,410,"Account deletion ends the socket room");
  await Promise.race([closed, new Promise((_, reject) => setTimeout(() => reject(new Error("Account deletion must close guest sockets")), 5000))]);
  for (const room of [freed, ...quota.slice(1)]) {
    assert.equal((await call("rooms/" + room.room, room.capability)).status, 410);
    assert.equal((await call("rooms/" + room.room + "/seed", room.capability)).status, 410);
    assert.equal((await bucket.list({ prefix: room.room + "/" })).objects.length, 0);
  }
  assert.equal((await json<RoomInfo>("rooms/" + unrelated.room, b)).phase, "uploading", "Deletion is scoped to the verified account");
  await call("account", b, "DELETE");
  assert.equal(await json("account/rooms/" + document, secondSession), null);
  assert.equal((await call("account", secondSession, "DELETE")).status, 200, "Account deletion is idempotent");
  // Persisted deletion survives a cold restart, and a new sign-in gets all quota back.
  await worker.dispose();
  worker = runtime();
  const signedInAgain = uid("creator-a");
  for (let n = 0; n < 10; n++) {
    const created = await json<CreatedRoom>("rooms", signedInAgain, "POST", { document: `00000000-0000-4000-9000-${String(n).padStart(12, "0")}` });
    assert(!quota.some(r => r.room === created.room));
  }
  assert.equal((await call("account", signedInAgain, "DELETE")).status, 200);
  // A failed Room init may reserve an account slot without ever creating a Room.
  // Remove the binding only in this disposable runtime to force that network failure.
  await worker.dispose();
  worker = runtime(false);
  assert.equal((await call("rooms", a, "POST", { document })).status, 500);
  const orphan = await json<AccountRoom>("account/rooms/" + document, a);
  assert(orphan.room);
  await worker.dispose();
  worker = runtime();
  assert.equal((await call("account", a, "DELETE")).status, 200, "Delete must clean slots left by failed Room initialization");
  assert.equal(await json("account/rooms/" + document, a), null);
  assert.equal((await call("account", a, "DELETE")).status, 200);
  console.log("Passed: verified identities, create/idempotent retry, replaceable seed/manifest, open, guest join, second-session cold recovery, other-UID/guest-admin refusal, Stop/upload cleanup, quota release, re-share, stale-room 410, account deletion of Live/Uploading rooms, socket revocation, cold re-sign-in, failed-init cleanup and per-IP guest limits.");
} finally { await worker.dispose(); await rm(root, { recursive: true, force: true }); }

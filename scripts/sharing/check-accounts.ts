/** Real Rust WASM + SQLite DO/R2 qualification, with locally signed identities.
 * Only the local runtime's certificate service is substituted. Production auth
 * has no test key, token bypass, configurable certificate URL or asserted UID path.
 * Run through `bun run verify sharing` after installing worker-build (README). */
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Miniflare } from "miniflare";
import { repository } from "../lib/artifacts";
import { SharingHost } from "../../packages/hitslop/src/wire/constants.generated";
import type { AccountRoom, CreatedRoom, RoomInfo } from "../../packages/hitslop/src/wire/sharing.generated";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const root = await mkdtemp(join(tmpdir(), "hitslop-account-relay-"));
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const pem = publicKey.export({ type: "pkcs1", format: "pem" }).toString();
const uid = (name: string) => {
  const now = Math.floor(Date.now() / 1000);
  const parts = [{ alg: "RS256", kid: "local-runtime-only" }, { sub: name, aud: "hitslopapp", iss: "https://securetoken.google.com/hitslopapp", iat: now, auth_time: now, exp: now + 3600, jti: crypto.randomUUID() }]
    .map(value => Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return parts + "." + sign("RSA-SHA256", Buffer.from(parts), privateKey).toString("base64url");
};
const a = uid("creator-a"), secondSession = uid("creator-a"), b = uid("creator-b");
const document = "00000000-0000-4000-8000-000000000001";
const seed = Buffer.alloc(512, 42), asset = Buffer.from("export default {};");
const app = "a".repeat(64);
function runtime() {
  return new Miniflare({
    durableObjectsPersist: join(root, "objects"), r2Persist: join(root, "r2"),
    workers: [{
      name: "relay", modules: true, scriptPath: join(repository, "generated/sharing/relay/index.js"),
      compatibilityDate: "2025-11-05", modulesRules: [{ type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true }],
      bindings: { BROWSER_ORIGIN: "https://share.hitslop.com", FIREBASE_PROJECT: "hitslopapp", CREATION_ENABLED: "1" },
      durableObjects: { ROOMS: { className: "SlopRoom", useSQLite: true }, ACCOUNTS: { className: "SharingAccount", useSQLite: true } },
      r2Buckets: ["SEEDS"],
      ratelimits: Object.fromEntries(["ACCOUNT_CREATE_LIMIT"].map(name => [name, { simple: { limit: 1000, period: 60 } }])),
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
  return worker.dispatchFetch("https://share.hitslop.com/v1/" + path, {
    method, headers: { ...headers, ...(token ? { Authorization: "Bearer " + token } : {}), ...(body === undefined ? {} : { "Content-Type": binary ? "application/octet-stream" : "application/json" }) },
    ...(body === undefined ? {} : { body: binary ? body : JSON.stringify(body) }),
  });
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
  const joined = await worker.dispatchFetch("https://share.hitslop.com/v1/" + base + "/sync", {
    headers: { Upgrade: "websocket", Origin: "https://share.hitslop.com", "Sec-WebSocket-Protocol": SharingHost.socketProtocol + ", cap." + r.capability },
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
  const lateUpload = worker.dispatchFetch("https://share.hitslop.com/v1/rooms/" + next.room + "/seed", {
    method: "PUT", headers: { Authorization: "Bearer " + a, ...seedHeaders(seed) }, body,
  });
  await ready;
  try {
    assert.equal((await call("rooms/" + next.room, a, "DELETE")).status, 200);
  } finally { release(); }
  assert.equal((await lateUpload).status, 410, "An upload spanning Stop must be refused");
  const bucket = await worker.getR2Bucket("SEEDS", "relay");
  assert.equal((await bucket.list({ prefix: next.room + "/" })).objects.length, 0, "Stop must remove late writes");
  assert.equal((await call("rooms/" + next.room + "/open", a, "POST")).status, 410);
  assert.equal((await call(base, a, "DELETE")).status, 200, "Repeated Stop is idempotent");
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
  await call("rooms/" + freed.room, a, "DELETE");
  for (const room of quota.slice(1)) await call("rooms/" + room.room, a, "DELETE");
  console.log("Passed: verified identities, create/idempotent retry, replaceable seed/manifest, open, guest join, second-session cold recovery, other-UID/guest-admin refusal, Stop/upload cleanup, quota release, re-share and stale-room 410.");
} finally { await worker.dispose(); await rm(root, { recursive: true, force: true }); }

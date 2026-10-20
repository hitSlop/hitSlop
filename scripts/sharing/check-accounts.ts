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
import type { Invitation, RoomIdentity, RoomReservation } from "../../packages/hitslop/src/wire/sharing.generated";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const root = await mkdtemp(join(tmpdir(), "hitslop-account-relay-"));
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const pem = publicKey.export({ type: "pkcs1", format: "pem" }).toString();
const uid = (name: string) => {
  const now = Math.floor(Date.now() / 1000);
  const parts = [{ alg: "RS256", kid: "local-runtime-only" }, { sub: name, aud: "hitslopapp", iss: "https://securetoken.google.com/hitslopapp", iat: now, auth_time: now, exp: now + 3600 }]
    .map(value => Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return parts + "." + sign("RSA-SHA256", Buffer.from(parts), privateKey).toString("base64url");
};
const a = uid("creator-a"), b = uid("creator-b");
const document = "00000000-0000-4000-8000-000000000001";
const seed = Buffer.alloc(512, 42), asset = Buffer.from("export default {};");
const app = "a".repeat(64);
function runtime() {
  return new Miniflare({
    durableObjectsPersist: join(root, "objects"), r2Persist: join(root, "r2"),
    workers: [{
      name: "relay", modules: true, scriptPath: join(repository, "generated/sharing/relay/index.js"),
      compatibilityDate: "2025-11-05", modulesRules: [{ type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true }],
      bindings: { BROWSER_ORIGIN: "https://share.hitslop.com", FIREBASE_PROJECT: "hitslopapp", AUTH_PROBE_ENABLED: "1", CREATION_ENABLED: "1" },
      durableObjects: { ROOMS: { className: "SlopRoom", useSQLite: true }, ACCOUNTS: { className: "SharingAccount", useSQLite: true } },
      r2Buckets: ["SEEDS"],
      ratelimits: Object.fromEntries(["CREATE_LIMIT", "AUTH_LIMIT", "ACCOUNT_CREATE_LIMIT"].map(name => [name, { simple: { limit: 1000, period: 60 } }])),
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
async function call(path: string, token: string | undefined, method = "GET", body?: unknown) {
  const binary = body instanceof Uint8Array;
  return worker.dispatchFetch("https://share.hitslop.com/v1/" + path, {
    method, headers: { ...(token ? { Authorization: "Bearer " + token } : {}), ...(body === undefined ? {} : { "Content-Type": binary ? "application/octet-stream" : "application/json" }) },
    ...(body === undefined ? {} : { body: binary ? body : JSON.stringify(body) }),
  });
}
async function json<T>(path: string, token: string, method = "GET", body?: unknown): Promise<T> {
  const response = await call(path, token, method, body);
  assert.equal(response.status, 200, path + " must succeed");
  return await response.json() as T;
}
const id = (r: RoomReservation): RoomIdentity => ({ document: r.document, generation: r.generation, room: r.room });
try {
  assert.equal((await call("account/reserve", undefined, "POST", { document, generation: 1 })).status, 401);
  const reservations = await Promise.all([1, 2].map(() => json<RoomReservation>("account/reserve", a, "POST", { document, generation: 1 })));
  const r = reservations[0]!;
  assert(r.capability && r.room === reservations[1]!.room && r.capability === reservations[1]!.capability, "Concurrent reserve must return one stable capability");
  assert.equal(await json("account/access/" + document, b), null);
  const other = await json<RoomReservation>("account/reserve", b, "POST", { document, generation: 1 });
  assert(other.room !== r.room, "Independent identities must have separate registries");
  assert.equal((await call("account/stop", b, "POST", id(r))).status, 409);
  const base = "rooms/" + r.room + "/";
  const init = { identity: id(r), init: { app, seed: [], seed_hash: hash(seed) } };
  assert.equal((await call(base + "init", b, "POST", init)).status, 409);
  assert.equal((await call(base + "init", r.capability, "POST", init)).status, 401);
  assert.equal((await call(base + "init", a, "POST", init)).status, 200);
  assert.equal((await call(base + "init", a, "POST", init)).status, 200);
  assert.equal((await call(base + "init", a, "POST", { ...init, init: { ...init.init, seed_hash: "f".repeat(64) } })).status, 409);
  assert.equal((await call("account/publish", a, "POST", id(r))).status, 409);
  assert.equal((await call(base + "seed", r.capability, "PUT", seed)).status, 403);
  assert.equal((await call(base + "stop", r.capability, "DELETE")).status, 401);
  assert.equal((await call(base + "seed", a, "PUT", seed)).status, 200);
  assert.equal((await call(base + "resources", a, "POST", { app, resources: [{ key: "ui.js", hash: hash(asset), size: asset.length, media_type: "text/javascript" }] })).status, 200);
  assert.equal((await call(base + "asset/" + hash(asset), r.capability, "PUT", asset)).status, 403);
  assert.equal((await call("account/publish", a, "POST", id(r))).status, 409);
  assert.equal((await call(base + "seed", r.capability)).status, 409);
  assert.equal((await call(base + "asset/" + hash(asset), a, "PUT", asset)).status, 200);
  const published = await json<RoomReservation>("account/publish", a, "POST", id(r));
  const invitation: Invitation | null = published.invitation;
  assert(invitation && invitation.seed_hash === hash(seed) && invitation.capability === r.capability, "Publication must return the reserved canonical seed and capability");
  assert.equal((await call(base + "seed", r.capability)).status, 200);
  assert.equal((await call(base + "seed", b, "PUT", seed)).status, 403);
  await worker.dispose();
  worker = runtime();
  const recovered = await json<RoomReservation>("account/access/" + document, a);
  assert(recovered.invitation?.capability === r.capability && recovered.invitation.seed_hash === hash(seed), "Owner recovery must survive a cold runtime");
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  let started!: () => void;
  const ready = new Promise<void>(resolve => { started = resolve; });
  const body = new ReadableStream({ async start(controller) {
    controller.enqueue(asset.subarray(0, 1)); started();
    await gate; controller.enqueue(asset.subarray(1)); controller.close();
  } });
  const lateUpload = worker.dispatchFetch("https://share.hitslop.com/v1/" + base + "asset/" + hash(asset), {
    method: "PUT", headers: { Authorization: "Bearer " + a }, body,
  });
  await ready;
  const stopped = await json<RoomReservation>("account/stop", a, "POST", id(r));
  release();
  assert.equal((await lateUpload).status, 410, "An upload spanning Stop must be refused");
  assert(stopped.phase === "ended" && !stopped.capability && !stopped.invitation, "Stop must forget the recoverable secret");
  const bucket = await worker.getR2Bucket("SEEDS", "relay");
  assert.equal((await bucket.list({ prefix: r.room + "/" })).objects.length, 0, "A late upload must not leave retained files after Stop");
  assert.equal((await call(base + "seed", r.capability)).status, 410);
  assert.equal((await call(base + "asset/" + hash(asset), a, "PUT", asset)).status, 410);
  assert.equal((await call("account/publish", a, "POST", id(r))).status, 410);
  const stale = await json<RoomReservation>("account/reserve", a, "POST", { document, generation: 1 });
  assert.equal(stale.phase, "ended");
  const next = await json<RoomReservation>("account/reserve", a, "POST", { document, generation: 2 });
  assert(next.room !== r.room && next.previous_room === r.room, "Re-share must reserve a new room and identify the ended predecessor");
  assert.equal((await call("account/stop", a, "POST", id(r))).status, 409);
  // Cancel a reservation before init, then try to race its delayed init.
  await json("account/stop", a, "POST", id(next));
  assert.equal((await call("rooms/" + next.room + "/init", a, "POST", { identity: id(next), init: init.init })).status, 409);
  await json("account/stop", b, "POST", id(other));
  console.log("Passed: real WASM identity verification, two isolated creators, concurrent reservation, immutable init, partial publication, guest-admin refusal, cold owner recovery, Stop/re-share and delayed-init refusal.");
} finally { await worker.dispose(); await rm(root, { recursive: true, force: true }); }

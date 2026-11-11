/** Real Rust WASM + SQLite DO/R2 qualification, with locally signed identities.
 * Only the local runtime's certificate service is substituted. Production auth
 * has no test key, token bypass, configurable certificate URL or asserted UID path.
 * Run through `bun run verify sharing` after installing worker-build (README). */
import assert from "node:assert/strict";
import { checkCompaction, relayHarness, decode, helloFrame, push, documentName, objects, resent } from "./check-compaction";
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
  const output = join(repository, "generated/sharing/check-accounts.mjs");
  await mkdir(join(repository, "generated/sharing"), { recursive: true });
  const built = await Bun.build({ entrypoints: [fileURLToPath(import.meta.url)], target: "node",
    external: ["miniflare", "typescript"], define: { "import.meta.url": JSON.stringify(import.meta.url) } });
  if (!built.success) throw new Error(built.logs.join("\n"));
  await writeFile(output, await built.outputs[0]!.text());
  const child = Bun.spawn(["node", output], { stdin: "inherit", stdout: "inherit", stderr: "inherit" });
  process.exit(await child.exited);
}
import { SharingHost } from "../../packages/pop/src/wire/constants.generated";
import type { AccessState, RoomInfo } from "../../packages/pop/src/wire/sharing.generated";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const root = await mkdtemp(join(tmpdir(), "pop-account-relay-"));
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const pem = publicKey.export({ type: "pkcs1", format: "pem" }).toString();
const uid = (name: string, provider = "apple.com") => {
  const now = Math.floor(Date.now() / 1000);
  const parts = [{ alg: "RS256", kid: "local-runtime-only" }, { sub: name, aud: relayConfig.vars.FIREBASE_PROJECT, iss: `https://securetoken.google.com/${relayConfig.vars.FIREBASE_PROJECT}`, iat: now, auth_time: now, exp: now + 3600, firebase: { sign_in_provider: provider }, jti: crypto.randomUUID() }]
    .map(value => Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return parts + "." + sign("RSA-SHA256", Buffer.from(parts), privateKey).toString("base64url");
};
const document = "00000000-0000-4000-8000-000000000001";
const seed = Buffer.alloc(512, 42);
const app = "a".repeat(64);
const environments = ts.parseConfigFileTextToJson("wrangler.jsonc", await readFile(join(repository, "apps/sharing/wrangler.jsonc"), "utf8")).config;
const relayConfig = environments as {
  compatibility_date: string;
  vars: Record<string, string>;
  ratelimits: { name: string; simple: { limit: number; period: 60 } }[];
};
const a = uid("creator-a"), secondSession = uid("creator-a"), b = uid("creator-b");
const staging = environments.env.staging;
assert.notEqual(staging.name, environments.name, "Staging uses a separate Worker");
for (const kind of ["ratelimits", "r2_buckets", "analytics_engine_datasets"]) {
  const key = kind === "ratelimits" ? "namespace_id" : kind === "r2_buckets" ? "bucket_name" : "dataset";
  const production = new Set(environments[kind].map((binding: Record<string, string>) => binding[key]));
  assert.ok(staging[kind].every((binding: Record<string, string>) => !production.has(binding[key])),
    "Staging isolates " + kind);
}
const harness = join(root, "relay-harness.mjs");
await writeFile(harness, relayHarness(join(repository, "generated/sharing/relay/index.js")));
function runtime(roomsAvailable = true, identityAvailable = true) {
  return new Miniflare({
    durableObjectsPersist: join(root, "objects"), r2Persist: join(root, "r2"),
    workers: [{
      name: "relay", modules: true, scriptPath: harness, modulesRoot: "/",
      compatibilityDate: relayConfig.compatibility_date, modulesRules: [{ type: "ESModule", include: ["**/*.js"], fallthrough: true }, { type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true }],
      bindings: relayConfig.vars,
      durableObjects: { ...(roomsAvailable ? { DOCUMENTS: { className: "SharedDocument", useSQLite: true } } : {}), ACCOUNTS: { className: "AccountLibrary", useSQLite: true } },
      r2Buckets: ["SEEDS"],
      analyticsEngineDatasets: { METRICS: { dataset: "dotpop_documents" } },
      ratelimits: Object.fromEntries(relayConfig.ratelimits.map(({ name, simple }) => [name, { simple: name === "ACCOUNT_CREATE_LIMIT" ? { ...simple, limit: 1000 } : simple }])),
      outboundService: "certificates",
    }, {
      name: "certificates", modules: true,
      script: `let requests = 0; export default { fetch(request) {
        if(new URL(request.url).pathname === '/__test/requests') return Response.json(requests);
        requests++;
        if(!${identityAvailable}) return new Response(null,{status:503});
        if(request.url!=='https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com')return new Response(null,{status:502});
        return Response.json(${JSON.stringify({ "local-runtime-only": pem })});
      } }`,
    }],
  });
}
let worker = runtime();
async function call(path: string, token: string | undefined, method = "GET", body?: unknown, headers: Record<string, string> = {}) {
  const binary = body instanceof Uint8Array;
  return resent(async () => {
    const response = await worker.dispatchFetch("https://share.dotpop.click/v1/" + path, {
      method, headers: { ...headers, ...(token ? { Authorization: "Bearer " + token } : {}), ...(body === undefined ? {} : { "Content-Type": binary ? "application/octet-stream" : "application/json" }) },
      ...(body === undefined ? {} : { body: binary ? body : JSON.stringify(body) }),
    });
    if (response.status === 101) return response;
    const bytes = await response.arrayBuffer();
    return new LocalResponse(response.status === 204 ? null : bytes, {status: response.status, headers: response.headers});
  });
}
async function json<T>(path: string, token: string, method = "GET", body?: unknown): Promise<T> {
  const response = await call(path, token, method, body);
  assert.equal(response.status, 200, path + " must succeed");
  return await response.json() as T;
}
const openSeed = (bytes: Uint8Array, attachments: string[] = []) => ({
  seed: hash(bytes), version: [], app, attachments,
});
const uploadSeed = (base: string, token: string, bytes: Uint8Array) =>
  call(base + "/blob/" + hash(bytes), token, "PUT", bytes, {"Content-Length": String(bytes.length)});
try {
  const cappedDocument = "00000000-0000-4000-8000-000000000087";
  const cappedBase = "documents/" + cappedDocument;
  const stagingOwner = uid("staging-owner");
  assert.equal((await call("documents", stagingOwner, "POST", {document: cappedDocument})).status, 204);
  for (let n = 0; n <= 2 * (256 + 1); n++) {
    const bytes = Buffer.from("bounded-staging-" + n);
    const response = await call(cappedBase + "/blob/" + hash(bytes), stagingOwner, "PUT", bytes, {
      "Content-Length": String(bytes.length),
      "CF-Connecting-IP": "198.19." + Math.floor(n / 250) + "." + (n % 250 + 1),
    });
    assert.equal(response.status, n < 514 ? 200 : 413,
      "Unopened room refuses distinct staged blob 515 beyond its 514-blob cap");
  }
  const expiryDocument = "00000000-0000-4000-8000-000000000086";
  const expiryBase = "documents/" + expiryDocument;
  assert.equal((await call("documents", stagingOwner, "POST", {document: expiryDocument})).status, 204);
  assert.equal((await uploadSeed(expiryBase, stagingOwner, seed)).status, 200);
  assert.equal((await uploadSeed(expiryBase, stagingOwner, Buffer.from("expired attachment"))).status, 200);
  const stagingNamespace = await objects(worker, "DOCUMENTS");
  const stagingStub = stagingNamespace.get(stagingNamespace.idFromName(expiryDocument));
  const stagingHook = (path: string, body?: unknown) => stagingStub.fetch("https://test/__test/" + path,
    {method: "POST", ...(body === undefined ? {} : {body: JSON.stringify(body)})});
  const stagingSQL = async (sql: string, args: unknown[] = []) =>
    (await stagingHook("sql", {sql, args})).json() as Promise<any[]>;
  await stagingHook("hold-alarms", {enabled: true});
  await stagingHook("fire-alarm");
  const [{staged_upload_ms: lastUpload}] = await stagingSQL("SELECT staged_upload_ms FROM document_state");
  const expiry = lastUpload + 24 * 60 * 60 * 1000;
  const scheduled = await (await stagingHook("alarm-time")).json() as number;
  assert.ok(scheduled >= expiry - 1000 && scheduled <= expiry + 1000,
    "Staging expiry is scheduled without another request");
  await stagingHook("fire-alarm");
  assert.equal((await json<any[]>(expiryBase + "/blobs", stagingOwner)).length, 2,
    "Recent staging survives an early alarm");
  await stagingSQL("UPDATE document_state SET staged_upload_ms=?", [Date.now() - 24 * 60 * 60 * 1000 - 1000]);
  await stagingHook("delete-failure", {enabled: true});
  await stagingHook("fire-alarm");
  assert.deepEqual(await json(expiryBase + "/blobs", stagingOwner), [], "Expired staging clears the inventory");
  assert.equal((await json<RoomInfo>(expiryBase, stagingOwner)).phase, "uploading", "Expiry keeps the room retryable");
  assert.equal((await stagingSQL("SELECT staged_upload_ms FROM document_state"))[0].staged_upload_ms, null);
  assert.equal((await stagingSQL("SELECT count(*) AS n FROM garbage"))[0].n, 2,
    "Failed R2 deletes remain durably queued");
  assert.ok(await (await stagingHook("alarm-time")).json(), "Failed deletes schedule their retry");
  await stagingHook("delete-failure", {enabled: false});
  await stagingHook("fire-alarm");
  assert.equal((await stagingSQL("SELECT count(*) AS n FROM garbage"))[0].n, 0);
  const stagingBucket = await worker.getR2Bucket("SEEDS", "relay");
  assert.equal((await stagingBucket.list({prefix: expiryDocument + "/"})).objects.length, 0);
  assert.equal(await (await stagingHook("alarm-time")).json(), null, "Empty staging does not loop alarms");
  assert.equal((await uploadSeed(expiryBase, stagingOwner, seed)).status, 200, "The owner can stage again after expiry");
  assert.equal((await call(expiryBase + "/open", stagingOwner, "POST", openSeed(seed))).status, 200);
  console.log("Passed: staging count cap, scheduled 24-hour expiry, durable R2 deletion retries and upload after expiry.");
  assert.equal((await call("documents", a, "POST", new Uint8Array((1 << 20) + 1))).status, 413,
    "Oversized creation body is a bounded request refusal");
  assert.equal((await call("documents", a, "POST", new TextEncoder().encode("{not json"))).status, 400,
    "A malformed body is a client error, never a 500");
  const certificateService = await worker.getWorker("certificates");
  assert.equal(await (await certificateService.fetch("https://test/__test/requests")).json(), 1,
    "A certificate response without max-age stays cached across requests");
  const oldSocket = await call("documents/" + document + "/sync", a, "GET", undefined, {
    Upgrade: "websocket", "Sec-WebSocket-Protocol": "pop.sync.v2",
  });
  assert.equal(oldSocket.status, 426, "Old socket protocols are told to update");
  assert.equal((await oldSocket.json() as {code:string}).code, "requires_update");
  const oldLibrary = await call("account/library", a, "GET", undefined, {Upgrade:"websocket", "Sec-WebSocket-Protocol":"pop.library.v1"});
  assert.equal(oldLibrary.status,426);
  assert.equal((await oldLibrary.json() as {code:string}).code,"requires_update");
  const retryDocument = "00000000-0000-4000-8000-000000000090";
  const retryBase = "documents/" + retryDocument;
  assert.equal((await call("documents", a, "POST", {document: retryDocument})).status, 204);
  const missingSeed = await call(retryBase + "/open", a, "POST", openSeed(seed));
  assert.equal(missingSeed.status, 409, "Open refuses an absent seed blob");
  const put = (bytes: Uint8Array, n: number) => call(retryBase + "/blob/" + hash(bytes), a, "PUT", bytes, {
    "Content-Length": String(bytes.length), "CF-Connecting-IP": "198.18." + Math.floor(n / 250) + "." + (n % 250 + 1),
  });
  const abandoned: string[] = [];
  for (let n = 0; n < 256; n++) {
    const bytes = Buffer.from("abandoned-upload-" + n);
    abandoned.push(hash(bytes));
    assert.equal((await put(bytes, n)).status, 200);
  }
  const replacement = Buffer.from("retry's different attachment");
  const retryUpload = await put(replacement, 256);
  assert.equal(retryUpload.status, 200,
    "Staged blobs must not exhaust the retry's attachment budget: " + await retryUpload.text());
  assert.equal((await put(seed, 257)).status, 200);
  assert.equal((await call(retryBase + "/open", a, "POST", openSeed(seed, [...abandoned, hash(replacement)]))).status,
    413, "Only the attachment set named by Open is charged to the budget");
  assert.equal((await call(retryBase + "/open", a, "POST", openSeed(seed, [hash(replacement)]))).status, 200);
  const retryBucket = await worker.getR2Bucket("SEEDS", "relay");
  for (const id of abandoned) assert.equal(await retryBucket.get(retryDocument + "/blobs/" + id), null);
  assert.deepEqual((await json<any[]>(retryBase + "/blobs", a)).map(item => item.hash), [hash(replacement)]);
  const largeDocument = "00000000-0000-4000-8000-000000000089", largeBase = "documents/" + largeDocument;
  assert.equal((await call("documents", a, "POST", {document: largeDocument})).status, 204);
  const largeSeed = Buffer.alloc(11 << 20, 21);
  assert.equal((await uploadSeed(largeBase, a, largeSeed)).status, 200, "The seed has its own limit above the attachment limit");
  assert.equal((await call(largeBase + "/open", a, "POST", openSeed(largeSeed))).status, 200);
  assert.deepEqual(await json(largeBase + "/blobs", a), [], "The seed is outside the attachment inventory");
  const tooLargeAttachment = Buffer.alloc(11 << 20, 22);
  assert.equal((await uploadSeed(largeBase, a, tooLargeAttachment)).status, 413, "Live attachments keep the per-file budget");
  assert.equal((await uploadSeed(largeBase, a, Buffer.alloc((16 << 20) + 1))).status, 400, "Staging keeps the seed size limit");
  console.log("Passed: Open refuses missing seed blobs; a changed upload set retries within the staging cap and removes unused blobs.");
  console.log("Passed: Open budgets only named attachments; the seed uses its own size limit and stays outside the live attachment inventory.");
  await checkCompaction(worker, call, uid("compaction-owner"), uid);
  // The join page, Apple callback, association file and icon deploy as static assets from
  // apps/sharing/public, with `_headers` and `_redirects`; this reads the files that ship.
  // The Worker itself serves nothing but the API.
  const publicRoot = join(repository, "apps/sharing/public");
  const text = (path: string) => readFile(join(publicRoot, path), "utf8");
  const rules = new Map<string, Map<string, string>>();
  let current: Map<string, string> | undefined;
  for (const line of (await text("_headers")).split("\n")) {
    if (!line.trim() || line.startsWith("#")) continue;
    if (!/^\s/.test(line)) rules.set(line.trim(), current = new Map());
    else { const [name, ...value] = line.trim().split(":"); current!.set(name!.trim(), value.join(":").trim()); }
  }
  const rule = (path: string, header: string) => rules.get(path)?.get(header);
  assert.equal(rule("/*", "X-Content-Type-Options"), "nosniff");
  assert.equal(rule("/*", "Referrer-Policy"), "no-referrer");
  const callbackPage = await text("auth/apple.html");
  assert.ok(callbackPage.includes("dotpop://auth/apple"));
  const script = /<script>([\s\S]*?)<\/script>/.exec(callbackPage)![1]!;
  const digest = createHash("sha256").update(script).digest("base64");
  assert.equal(rule("/auth/apple", "Content-Security-Policy"),
    `default-src 'none'; script-src 'sha256-${digest}'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`);
  assert.equal(rule("/auth/apple", "Cache-Control"), "no-store");
  for (const fragment of ["#state=example&id_token=a.b.c&code=a%2Bb%26c", "#state=example&error=user_cancelled_authorize", ""]) {
    let destination: string | undefined;
    runInNewContext(script, { location: { hash: fragment, replace(url: string) { destination = url; } } });
    assert.equal(destination, fragment ? "dotpop://auth/apple" + fragment : undefined);
  }
  assert.deepEqual(JSON.parse(await text(".well-known/apple-app-site-association")), {
    applinks: { details: [{ appIDs: ["78UAXU8QG8.com.hitslop.app"], components: [{ "/": "/join", comment: "Live sharing links" }] }] },
  });
  assert.equal(rule("/.well-known/apple-app-site-association", "Content-Type"), "application/json");
  assert.equal(rule("/.well-known/apple-app-site-association", "Cache-Control"), "public, max-age=3600");
  const invitation = `${SharingHost.origin}/join#${document}.${"b".repeat(64)}`;
  const joinPage = await text("join.html");
  assert.match(joinPage, /<title>Open a shared pop<\/title>/);
  for (const tag of [
    '<meta property="og:title" content="Open a shared pop"',
    '<meta property="og:site_name" content="dotpop"',
    '<meta property="og:type" content="website"',
    '<meta property="og:description" content="Open this invitation in dotpop to edit together."',
    `<meta property="og:image" content="${SharingHost.origin}/share-icon.png"`,
    `<link rel="apple-touch-icon" href="${SharingHost.origin}/share-icon.png"`,
  ]) assert.ok(joinPage.includes(tag), `Preview metadata is present without running JavaScript: ${tag}`);
  assert.match(joinPage, /<a id="open-dotpop" hidden>Open in dotpop<\/a>/);
  assert.ok(!joinPage.includes(invitation) && !joinPage.includes("b".repeat(64)), "The static page holds no invitation credential");
  const joinScript = /<script>([\s\S]*?)<\/script>/.exec(joinPage)![1]!;
  const joinDigest = createHash("sha256").update(joinScript).digest("base64");
  assert.equal(rule("/join", "Content-Security-Policy"),
    `default-src 'none'; script-src 'sha256-${joinDigest}'; style-src 'unsafe-inline'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`);
  for (const fragment of [invitation.slice(invitation.indexOf("#")), "", "#"]) {
    const link = { href: undefined as string | undefined, hidden: true };
    runInNewContext(joinScript, {
      location: { hash: fragment },
      document: { getElementById(id: string) { assert.equal(id, "open-dotpop"); return link; } },
    });
    assert.equal(link.href, fragment.length > 1 ? "dotpop://join" + fragment : undefined);
    assert.equal(link.hidden, fragment.length <= 1);
  }
  assert.equal((await text("_redirects")).trim(), "/ https://dotpop.click 302");
  const image = await readFile(join(publicRoot, "share-icon.png"));
  assert.equal(image.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
  assert.ok(image.readUInt32BE(16) >= 900 && image.readUInt32BE(20) >= 900, "Messages receives a high-resolution preview image");
  // Nothing but the API answers from the Worker: pages are static assets, and there is no
  // browser surface (identity configuration, authored origins, CORS preflight).
  for (const [path, init] of [["/join", {}], ["/auth/config", {}], ["/assets/ui.js", {}], ["/attachments/" + "b".repeat(64), {}],
    ["/_resources/" + document, { method: "POST" }], ["/v1/documents", { method: "OPTIONS", headers: { Origin: SharingHost.origin } }]] as const)
    assert.equal((await worker.dispatchFetch("https://share.dotpop.click" + path, init)).status, 404, "The Worker serves no " + path);

  assert.equal((await call("account", undefined, "DELETE")).status,401);
  assert.equal((await call("documents", uid("anonymous","anonymous"), "POST",{document})).status,401);
  const [first,retry]=await Promise.all([call("documents",a,"POST",{document}),call("documents",secondSession,"POST",{document})]);
  for (const created of [first,retry]) {
    assert.equal(created.status,204,"Creation is idempotent across one account's sessions");
    assert.equal(await created.text(),"","Creation replies with no body");
  }
  assert.equal((await call("documents",b,"POST",{document})).status,403,"UUID cannot be claimed by another account");
  const base="documents/"+document;
  assert.equal((await uploadSeed(base,a,seed)).status,200);
  assert.equal((await call(base+"/blobs",a)).status,200,"The creator lists uploaded blobs before Open");
  assert.equal((await call(base+"/blobs",b)).status,403,"Others cannot");
  assert.equal((await call(base+"/open",a,"POST",openSeed(seed))).status,200);
  // The live account feed is isolated by verified identity and survives reconnects.
  const library = async (token:string) => {
    const reply = await call("account/library", token, "GET", undefined, {Upgrade:"websocket"});
    assert.equal(reply.status,101);
    assert.equal(reply.headers.get("Sec-WebSocket-Protocol"),null);
    const socket = reply.webSocket!; socket.accept();
    const queue:any[]=[]; const waiters:((value:any)=>void)[]=[];
    socket.addEventListener("message", (event:any)=>{ if(event.data === "pong")return; const frame=JSON.parse(event.data); const waiter=waiters.shift(); if(waiter)waiter(frame);else queue.push(frame); });
    const next=()=>queue.length ? Promise.resolve(queue.shift()) : new Promise<any>((resolve,reject)=>{
      const timer=setTimeout(()=>reject(new Error("Expected library frame")),5000);
      waiters.push(frame=>{clearTimeout(timer);resolve(frame);});
    });
    return {socket,next};
  };
  const libraryNamespace = await objects(worker, "DOCUMENTS");
  const libraryDocument = libraryNamespace.get(libraryNamespace.idFromName(document));
  await libraryDocument.fetch("https://test/__test/alarm");
  const ownerLibrary=await library(a), outsiderLibrary=await library(b);
  const initialLibrary=await ownerLibrary.next();
  assert.equal(initialLibrary.type,"snapshot"); assert.equal(initialLibrary.last,true);
  assert.equal(initialLibrary.entries[0].document,document);
  assert.deepEqual((await outsiderLibrary.next()).entries,[],"An unrelated account sees no private entries");
  const bootstrapAccounts=await objects(worker, "ACCOUNTS");
  const bootstrapAccount=bootstrapAccounts.get(bootstrapAccounts.idFromName(hash(Buffer.from("creator-a"))));
  await bootstrapAccount.fetch("https://test/__test/fail-deliveries",{method:"POST",body:JSON.stringify({enabled:true})});
  await json(base,a,"PATCH",{title:"Live title",templateId:"fixture"});
  await libraryDocument.fetch("https://test/__test/alarm");
  const owed=await (await libraryDocument.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT count(*) AS value FROM library_outbox"})})).json() as {value:number}[];
  assert.ok(owed[0]!.value>0,"Failed publication stays durable without another edit");
  await bootstrapAccount.fetch("https://test/__test/fail-deliveries",{method:"POST",body:JSON.stringify({enabled:false})});
  await libraryDocument.fetch("https://test/__test/alarm");
  let changed=await ownerLibrary.next();
  while(changed.type === "upsert" && changed.entry.title !== "Live title")changed=await ownerLibrary.next();
  assert.equal(changed.type,"upsert"); assert.equal(changed.entry.title,"Live title");
  ownerLibrary.socket.close(); outsiderLibrary.socket.close();
  const reconnected=await library(secondSession);
  assert.equal((await reconnected.next()).entries[0].title,"Live title","Reconnect reads the durable account projection");
  reconnected.socket.close();
  assert.equal((await call(base+"/seed",b)).status,403,"private bytes require owner identity");
  assert.equal((await call(base+"/sharing","not-a-valid-identity")).status,401,"invalid identity is retryable authentication, not lost membership");
  assert.equal((await call(base+"/sharing",b)).status,403,"a valid nonmember is denied access");
  assert.equal((await call(base+"/seed",secondSession)).status,200,"same account on another device shares private bytes");
  assert.equal((await json<AccessState>(base+"/sharing",secondSession)).role,"owner");
  const forgedIdentity = {"X-Pop-Verified-Identity":JSON.stringify({uid:"creator-a",expires:Math.floor(Date.now()/1000)+3600})};
  const forged = await call(base+"/seed", b, "GET", undefined, forgedIdentity);
  assert.equal(forged.status,403);
  assert.deepEqual(await forged.json(),{code:"denied",message:"Access revoked"});
  assert.equal((await call(base+"/seed",undefined,"GET",undefined,forgedIdentity)).status,401);
  const enabled=await json<AccessState>(base+"/sharing",a,"POST");assert(enabled.capability);assert(enabled.sharing);
  // Only the authorized access reply carries the invitation secret; document metadata never does,
  // even for the owner while sharing is on.
  for (const info of [await json<RoomInfo>(base,a), await json<RoomInfo>(base+"/open",a,"POST",openSeed(seed))]) {
    assert.equal(info.document,document);
    assert.equal(info.phase,"live");
    assert.equal(typeof info.compact,"boolean","Every reply states whether compaction is due");
    assert.ok(!("capability" in info) && !JSON.stringify(info).includes(enabled.capability!),"Document metadata never carries the invitation");
  }
  const anonymousJoin = await call(base+"/members",uid("anonymous", "anonymous"),"POST",{capability:enabled.capability});
  assert.equal(anonymousJoin.status,401);
  assert.deepEqual(await anonymousJoin.json(),{code:"sign_in_required",message:"Sign in to join"});
  const invitedIdentity = uid("invited-member");
  await json(base+"/members",invitedIdentity,"POST",{capability:enabled.capability});
  assert.equal((await call(base+"/seed",invitedIdentity)).status,200,"Signed-in identities may join");
  assert.equal((await call(base+"/seed",enabled.capability!)).status,401,"Invitation secrets cannot authorize document reads");
  const member=await json<AccessState>(base+"/members",b,"POST",{capability:enabled.capability});assert.equal(member.role,"editor");
  assert.equal((member as AccessState & {title:string}).title,"Live title","Join carries metadata without a second request");
  assert.equal((await call(base+"/metadata",b)).status,404,"The metadata route is removed");
  assert.equal((await call(base+"/seed",b)).status,200);
  await libraryDocument.fetch("https://test/__test/alarm");
  const memberLibrary=await library(b);
  const memberSnapshot=await memberLibrary.next();
  assert.equal(memberSnapshot.entries[0].document,document);
  assert.equal(memberSnapshot.entries[0].role,"editor");

  const connect=async(token:string)=>{
    const response=await call(base+"/sync",token,"GET",undefined,{Upgrade:"websocket"});
    assert.equal(response.status,101);const socket=response.webSocket;assert(socket);socket.accept();
    const queue:ReturnType<typeof decode>[]=[];const waiters:((frame:ReturnType<typeof decode>)=>void)[]=[];
    socket.addEventListener("message",(event:any)=>{const frame=decode(event.data);const resolve=waiters.shift();if(resolve)resolve(frame);else queue.push(frame);});
    const next=async()=>queue.shift()??await new Promise<ReturnType<typeof decode>>((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error("Expected socket frame")),5000);waiters.push(frame=>{clearTimeout(timer);resolve(frame);});});
    socket.send(helloFrame());
    assert.equal((await next()).tag,2);assert.equal((await next()).tag,4);
    return {socket,next};
  };
  assert.equal((await call(base+"/sync",undefined,"GET",undefined,{
    Upgrade:"websocket", "Sec-WebSocket-Protocol":`pop.sync.v2, cap.${enabled.capability}`,
  })).status,426,"Credentials in a socket subprotocol are never read");
  const ownerConnection=await connect(a),memberConnection=await connect(invitedIdentity);
  const ownerSocket=ownerConnection.socket,memberSocket=memberConnection.socket;let ownerClosed=false;
  ownerSocket.addEventListener("close",()=>ownerClosed=true);
  const memberEnded=new Promise<void>(resolve=>memberSocket.addEventListener("close",()=>resolve()));
  const documents = await objects(worker, "DOCUMENTS");
  const documentStub = documents.get(documents.idFromName(documentName(document)));
  const revoked=await json<AccessState>(base+"/sharing",a,"DELETE");assert(!revoked.sharing);assert.ok(!("generation" in revoked));
  assert.equal((await call(base+"/seed",enabled.capability!)).status,401);assert.equal((await call(base+"/seed",invitedIdentity)).status,403);assert.equal((await call(base+"/seed",b)).status,403);
  for (const [suffix,method] of [["","GET"],["/sharing","GET"],["/seed","GET"],["/blobs","GET"],["/blob/"+"b".repeat(64),"GET"],["/blob/"+"b".repeat(64),"PUT"],["/compact","POST"],["/sync","GET"]]) {
    const reply=await call(base+suffix,b,method);
    assert.equal(reply.status,403,"Revoked membership on "+suffix);
    assert.deepEqual(await reply.json(),{code:"denied",message:"Access revoked"});
  }
  assert.equal((await call(base+"/seed",a)).status,200);assert(!ownerClosed,"revocation does not close private owner connection");
  assert.equal((await memberConnection.next()).code,3,"revocation delivers the real access-ended frame");
  try { memberSocket.send(push(2,0,1,new Uint8Array([8]))); } catch { /* Peer is already closing. */ }
  memberSocket.close(); // Acknowledge the server close, as the native client does.
  await Promise.race([memberEnded,new Promise((_,reject)=>setTimeout(()=>reject(new Error("Member close handshake did not complete")),5000))]);
  ownerSocket.send(push(1,0,1));
  const ownerAck=await ownerConnection.next();assert.equal(ownerAck.tag,6);assert.equal(ownerAck.status,0,"owner still writes after invitation revocation");
  assert.equal((await ownerConnection.next()).tag,3,"owner still receives live accepted updates");
  const heads=await (await documentStub.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT peer,end FROM heads ORDER BY peer"})})).json() as {peer:string;end:number}[];
  assert.deepEqual(heads.map(h=>[h.peer,h.end]),[["0000000000000001",1]],"a revoked member push never enters the accepted frontier");
  ownerSocket.close();
  const reopened=await json<AccessState>(base+"/sharing",a,"POST");assert.notEqual(reopened.capability,enabled.capability);
  const staying=uid("staying-member");
  await json(base+"/members",b,"POST",{capability:reopened.capability});
  await json(base+"/members",staying,"POST",{capability:reopened.capability});
  const attachment=new Uint8Array([3,1,4,1,5,9]);
  assert.equal((await call(base+"/blob/"+hash(attachment),a,"PUT",attachment,{"Content-Length":String(attachment.length)})).status,200);
  const readAttachment=async(token:string)=>{
    const response=await call("documents/"+document+"/blob/"+hash(attachment),token);
    const bytes=new Uint8Array(await response.arrayBuffer());
    if (response.status===200) assert.deepEqual(bytes,attachment);
    return response.status;
  };
  const stubSQL=async<T>(stub:typeof documentStub,query:string,args:unknown[]=[])=>
    await (await stub.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:query,args})})).json() as T[];
  const accountNamespace=await objects(worker, "ACCOUNTS");
  const accountFor=(name:string)=>accountNamespace.get(accountNamespace.idFromName(hash(Buffer.from(name))));
  const hook=(account:typeof documentStub,path:string,body:unknown={})=>account.fetch("https://test/__test/"+path,{method:"POST",body:JSON.stringify(body)});
  const flush=(stub:typeof documentStub)=>stub.fetch("https://test/__test/alarm");
  const snapshot=async(token:string)=>{
    const feed=await library(token);const entries:any[]=[];
    for(;;){const page=await feed.next();assert.equal(page.type,"snapshot");entries.push(...page.entries);if(page.last)break;}
    feed.socket.close();return entries;
  };
  const listed=async(token:string,id:string)=>(await snapshot(token)).some(e=>e.document===id);
  const until=async(check:()=>Promise<boolean>,message:string)=>{
    for(let n=0;n<500;n++){if(await check())return;await new Promise(resolve=>setTimeout(resolve,10));}
    assert.fail(message);
  };
  const liveDocument=async(id:string)=>{
    const base="documents/"+id;
    assert.equal((await call("documents",a,"POST",{document:id})).status,204);
    assert.equal((await uploadSeed(base,a,seed)).status,200);
    assert.equal((await call(base+"/open",a,"POST",openSeed(seed))).status,200);
    const stub=documents.get(documents.idFromName(documentName(id)));
    await hook(stub,"hold-alarms",{enabled:true});
    return {id,base,stub,invite:await json<AccessState>(base+"/sharing",a,"POST")};
  };
  const leaver=accountFor("creator-b");
  assert.equal(await readAttachment(b),200);
  await hook(leaver,"fail-deliveries",{enabled:true});
  assert.equal((await call(base+"/members/me",b,"DELETE")).status,200,"Leave is independent of account delivery");
  assert.equal(await readAttachment(b),403);
  assert.equal(await readAttachment(staying),200);
  await flush(documentStub);
  await hook(leaver,"fail-deliveries",{enabled:false});
  await flush(documentStub);
  let removal=await memberLibrary.next();while(removal.type!=="remove")removal=await memberLibrary.next();
  assert.equal(removal.document,document);memberLibrary.socket.close();
  assert.ok(!await listed(b,document),"Leave removes the member's library entry");

  const idle=await liveDocument("00000000-0000-4000-8000-000000000095");
  const who=uid("outbox-member"),account=accountFor("outbox-member");
  const joinIdle=()=>call(idle.base+"/members",who,"POST",{capability:idle.invite.capability});
  const pending=async()=>await stubSQL<any>(idle.stub,"SELECT payload FROM library_outbox WHERE uid=?",["outbox-member"]);
  const desired=async()=>JSON.parse((await pending())[0]!.payload);
  const apply=async(payload:unknown)=>{
    const reply=await account.fetch("https://test/internal/apply-entry",{method:"POST",body:JSON.stringify(payload)});
    assert.equal(reply.status,200);return await reply.json();
  };
  await hook(account,"fail-deliveries",{enabled:true});
  assert.equal((await joinIdle()).status,200,"Join succeeds while the account is unavailable");
  const firstEntry=await desired();
  assert.equal((await call(idle.base+"/seed",who)).status,200,"Membership is already authoritative");
  await flush(idle.stub);
  assert.ok(!await listed(who,idle.id));
  await hook(account,"fail-deliveries",{enabled:false});
  await flush(idle.stub);
  assert.ok(await listed(who,idle.id),"Retry adds the member's library entry");

  assert.equal((await call(idle.base+"/members/me",who,"DELETE")).status,200);
  const removed=await desired();
  await flush(idle.stub);
  assert.ok(!await listed(who,idle.id));
  await apply(firstEntry);await apply(firstEntry);
  assert.ok(!await listed(who,idle.id),"Late or duplicated upserts cannot resurrect removal");
  assert.equal((await joinIdle()).status,200);
  const rejoined=await desired();
  await flush(idle.stub);
  await apply(removed);await apply(removed);
  assert.ok(await listed(who,idle.id),"A late leave delivery must not drop the rejoin's library entry");
  assert.ok(rejoined.revision>removed.revision);

  // A held old delivery must not acknowledge a newer replacement in the outbox.
  await hook(account,"hold-delivery");
  assert.equal((await call(idle.base+"/members/me",who,"DELETE")).status,200);
  const delivering=flush(idle.stub);
  await until(async()=>!!await (await account.fetch("https://test/__test/delivery-waiting")).json(),"Delivery is held");
  assert.equal((await joinIdle()).status,200,"A pending delivery never blocks a rejoin");
  await hook(account,"release-delivery");await delivering;
  await flush(idle.stub);
  assert.ok(await listed(who,idle.id),"The replacement upsert survives the old acknowledgment");

  // Account deletion before first delivery fences and eventually leaves the authoritative member.
  const lateAccount=accountFor("late-deleted"),lateToken=uid("late-deleted");
  await hook(lateAccount,"fail-deliveries",{enabled:true});
  assert.equal((await call(idle.base+"/members",lateToken,"POST",{capability:idle.invite.capability})).status,200);
  assert.equal((await call("account",lateToken,"DELETE")).status,200);
  await hook(lateAccount,"fail-deliveries",{enabled:false});
  await flush(idle.stub);
  assert.equal((await call(idle.base+"/seed",lateToken)).status,403,"Late entry delivery cleans up a deleted account's membership");

  assert.equal((await call(idle.base+"/sharing",a,"DELETE")).status,200);
  await flush(idle.stub);
  assert.ok(!await listed(who,idle.id),"Stop sharing removes every member's library entry");
  await apply(rejoined);
  assert.ok(!await listed(who,idle.id),"A stopped invitation cannot be resurrected by old delivery");
  assert.equal((await call(idle.base+"/seed",a)).status,200,"The owner keeps access");

  const deleted=await liveDocument("00000000-0000-4000-8000-000000000094");
  await call(deleted.base+"/members",who,"POST",{capability:deleted.invite.capability});
  await flush(deleted.stub);
  await hook(account,"fail-deliveries",{enabled:true});
  assert.equal((await call(deleted.base,a,"DELETE")).status,200,"Delete does not depend on account delivery");
  await hook(account,"fail-deliveries",{enabled:false});
  await flush(deleted.stub);
  assert.ok(!await listed(who,deleted.id),"Deletion removes a member entry");
  assert.equal((await call("documents",a,"POST",{document:deleted.id})).status,410);

  // Open chooses an immutable staged seed; a second attempt cannot replace a live seed.
  const competing="00000000-0000-4000-8000-000000000093",competingBase="documents/"+competing;
  const laterSeed=Buffer.alloc(512,43);
  await call("documents",a,"POST",{document:competing});
  await uploadSeed(competingBase,a,seed);
  await uploadSeed(competingBase,a,laterSeed);
  assert.equal((await call(competingBase+"/open",a,"POST")).status,400,"Open requires the seed it makes live");
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(seed, ["f".repeat(64)]))).status,409,
    "Open refuses a missing attachment");
  assert.equal((await json<RoomInfo>(competingBase,a)).phase,"uploading");
  const laterKey = competing + "/blobs/" + hash(laterSeed);
  await retryBucket.delete(laterKey);
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(laterSeed))).status,409,"Open checks R2 presence");
  await retryBucket.put(laterKey, new Uint8Array(seed), {sha256: hash(seed)});
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(laterSeed))).status,409,"Open checks available R2 checksums even at the same size");
  await retryBucket.put(laterKey, new Uint8Array(100));
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(laterSeed))).status,409,"Open checks the recorded size");
  await retryBucket.put(laterKey, new Uint8Array(laterSeed), {sha256: hash(laterSeed)});
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(laterSeed))).status,200);
  assert.equal((await uploadSeed(competingBase,a,laterSeed)).status,200,"A live seed retry is harmless");
  assert.equal((await call(competingBase+"/seed",a,"PUT",seed)).status,404,"No dedicated seed upload route");
  assert.deepEqual(Buffer.from(await (await call(competingBase+"/seed",a)).arrayBuffer()),laterSeed);
  assert.equal(await retryBucket.get(competing + "/blobs/" + hash(seed)),null);
  assert.equal((await call(competingBase+"/open",a,"POST",openSeed(seed))).status,409,
    "A live document's Open still names only its own seed");
  const crowded="00000000-0000-4000-8000-000000000096",crowdedDocument=await liveDocument(crowded);
  const crowdedJoin=(token:string)=>call(crowdedDocument.base+"/members",token,"POST",{capability:crowdedDocument.invite.capability});
  await stubSQL(crowdedDocument.stub,"WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<63) INSERT INTO members SELECT 'filler-'||i FROM n");
  const racers=[uid("crowd-one"),uid("crowd-two")];
  const statuses=await Promise.all(racers.map(async token=>(await crowdedJoin(token)).status));
  assert.deepEqual([...statuses].sort((a,b)=>a-b),[200,429],"Exactly one racing join takes the last place");
  const admitted=racers[statuses.indexOf(200)]!,refused=racers[statuses.indexOf(429)]!;
  assert.equal((await crowdedJoin(admitted)).status,200,"Rejoining at the cap counts once");
  await flush(crowdedDocument.stub);
  assert.ok(await listed(admitted,crowded));assert.ok(!await listed(refused,crowded));
  assert.equal((await call(base+"/seed",a)).status,200);
  assert.equal((await call(base,a,"PATCH",{title:"Repairable private document",templateId:"fixture"})).status,200);
  // A metadata write still reading its body when the owner deletes the document is refused
  // and stores nothing: deletion is final even for a request admitted before it.
  const lateDocument="00000000-0000-4000-8000-000000000097";
  assert.equal((await call("documents",a,"POST",{document:lateDocument})).status,204);
  const lateStub=documents.get(documents.idFromName(documentName(lateDocument)));
  let finishPatch!: () => void;
  const patchBody=new ReadableStream<Uint8Array>({ start(controller) {
    controller.enqueue(new TextEncoder().encode(JSON.stringify({title:"Written after deletion",templateId:"late"})));
    finishPatch=()=>controller.close();
  } });
  const delayedPatch=worker.dispatchFetch(`https://share.dotpop.click/v1/documents/${lateDocument}`,{method:"PATCH",
    headers:{Authorization:`Bearer ${a}`,"Content-Type":"application/json","X-Test-Observe-Body":"1"},body:patchBody,duplex:"half" as const});
  let patchReading=false;
  for (let attempt=0; attempt<500; attempt++) {
    if (Number(await (await lateStub.fetch("https://test/__test/body-reads")).text()) > 0) { patchReading=true; break; }
    await new Promise(resolve=>setTimeout(resolve,10));
  }
  assert.ok(patchReading,"The metadata write is waiting for its body after authorization");
  assert.equal((await call("documents/"+lateDocument,a,"DELETE")).status,200);
  finishPatch();
  const late=await delayedPatch;
  assert.equal(late.status,410,"A metadata write that outlives deletion is refused");
  assert.equal((await late.json() as {code:string}).code,"sharing_ended");
  const lateMeta=await (await lateStub.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT title,template_id FROM document_state WHERE title IS NOT NULL OR template_id IS NOT NULL"})})).json();
  assert.deepEqual(lateMeta,[],"A deleted document keeps no metadata written after deletion");
  assert.equal((await call("documents/"+lateDocument+"/metadata",a)).status,404);
  const repairDocument="00000000-0000-4000-8000-000000000098";
  const repairBase="documents/"+repairDocument;
  assert.equal((await call("documents",a,"POST",{document:repairDocument})).status,204);
  const repairStub=documents.get(documents.idFromName(repairDocument));
  await hook(repairStub,"hold-alarms",{enabled:true});
  await hook(bootstrapAccount,"fail-deliveries",{enabled:true});
  assert.equal((await uploadSeed(repairBase,a,seed)).status,200);
  assert.equal((await call(repairBase+"/open",a,"POST",openSeed(seed))).status,200);
  assert.ok((await stubSQL<any>(repairStub,"SELECT count(*) AS n FROM library_outbox"))[0].n>0,"An undelivered entry is durable before restart");
  await worker.dispose();worker=runtime();
  const restartedDocuments=await objects(worker, "DOCUMENTS");
  for(const id of [document,repairDocument]) {
    const stub=restartedDocuments.get(restartedDocuments.idFromName(id));await flush(stub);
  }
  const restartedAccounts=await objects(worker, "ACCOUNTS");
  const restartedMember=restartedAccounts.get(restartedAccounts.idFromName(hash(Buffer.from("outbox-member"))));
  assert.equal((await restartedMember.fetch("https://test/internal/apply-entry",{method:"POST",body:JSON.stringify(firstEntry)})).status,200);
  assert.ok(!await listed(who,idle.id),"Removal high-water marks survive an account restart");
  const restored = await library(secondSession);
  const restoredEntries:any[] = [];
  for (;;) {
    const page = await restored.next();
    assert.equal(page.type,"snapshot");
    restoredEntries.push(...page.entries);
    if (page.last) break;
  }
  restored.socket.close();
  assert(restoredEntries.some(row=>row.document===document&&row.role==="owner"));
  assert(restoredEntries.some(row=>row.document===repairDocument&&row.role==="owner"),"Acknowledged uploads remain discoverable through the live library after restart");

  const departed = await library(b);
  assert.deepEqual((await departed.next()).entries,[],"Leave removes the member library entry after restart");
  departed.socket.close();
  assert.equal((await call(base+"/seed",secondSession)).status,200,"restart retains private document bytes");
  assert.equal((await call(base,secondSession,"DELETE")).status,200);
  assert.equal((await call(base+"/seed",secondSession)).status,410);
  assert.equal((await call("documents",secondSession,"POST",{document})).status,410,"deleted UUID cannot be resurrected");
  assert.equal((await call("documents",b,"POST",{document})).status,410,"deleted UUID cannot be stolen");
  const deletionDocument="00000000-0000-4000-8000-000000000099";
  assert.equal((await call("documents",a,"POST",{document:deletionDocument})).status,204);
  assert.equal((await call("account",a,"DELETE")).status,200,"account deletion handles integer owner registration and purges owned documents");
  assert.equal((await call("documents/"+deletionDocument,a)).status,410);
  assert.equal((await call("account",a,"DELETE")).status,200,"account deletion retry is idempotent");
  const prolific = uid("prolific");
  for (let index = 0; index < 12; index++) {
    const id = `00000000-0000-4000-8000-0000000002${String(index).padStart(2, "0")}`;
    assert.equal((await call("documents", prolific, "POST", { document: id })).status, 204, "no account document cap: " + index);
  }
  for (const path of ["documents/" + document + "/blob/" + "b".repeat(64) + "/extra", "account/library/" + document, "account/library/" + document + "/extra", "account/extra"]) {
    assert.equal((await call(path, undefined, "GET")).status, 404, "unlisted routes never reach a handler: " + path);
  }
  assert.equal((await call("documents/" + document + "/members", prolific, "DELETE")).status, 404, "only members/me leaves");
  // An initialization committed before the account stored its registration.
  const intentName="creation-intent", intentToken=uid(intentName);
  const intentId="00000000-0000-4000-8000-000000000088";
  const intentAccounts=await objects(worker, "ACCOUNTS");
  const intentAccount=intentAccounts.get(intentAccounts.idFromName(hash(Buffer.from(intentName))));
  const intentSQL=async(sql:string,args:unknown[]=[])=>
    (await intentAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql,args})})).json();
  await intentSQL("INSERT INTO entries(document,revision,payload,visible,owned,initializing) VALUES(?,0,NULL,0,1,1)",[intentId]);
  const intentDocs=await objects(worker, "DOCUMENTS");
  const intentDoc=intentDocs.get(intentDocs.idFromName(intentId));
  assert.equal((await intentDoc.fetch(`https://test/internal/documents/${intentId}/init`,{method:"POST",body:JSON.stringify({owner_uid:intentName,document:intentId})})).status,204);
  await worker.dispose(); worker=runtime();
  const recovering=await library(intentToken);
  assert.deepEqual((await recovering.next()).entries,[],"Unfinished uploads remain hidden after intent recovery");
  recovering.socket.close();
  const recoveredAccounts=await objects(worker, "ACCOUNTS");
  const recoveredAccount=recoveredAccounts.get(recoveredAccounts.idFromName(hash(Buffer.from(intentName))));
  const recoveredRows=await (await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT owned FROM entries WHERE document=?",args:[intentId]})})).json();
  assert.deepEqual(recoveredRows,[{owned:1}],"Restart recovers the owned registration from its durable intent");
  // Deletion must also cover an intent not yet recovered by a library connection.
  await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"DELETE FROM entries WHERE document=?",args:[intentId]})});
  await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"INSERT INTO entries(document,revision,payload,visible,owned,initializing) VALUES(?,0,NULL,0,1,1)",args:[intentId]})});
  assert.equal((await call("account",intentToken,"DELETE")).status,200);
  assert.equal((await call("documents/"+intentId,intentToken)).status,410,"Account deletion includes unfinished initialization intents");
  await worker.dispose(); worker=runtime(true,false);
  for (const [path,method] of [["documents", "POST"],["account/library","GET"],["documents/"+repairDocument,"GET"],["documents/"+repairDocument+"/sharing","GET"],["documents/"+repairDocument+"/seed","GET"],["documents/"+repairDocument+"/sync","GET"]]) {
    const outage=await call(path!,b,method!);
    assert.equal(outage.status,503,"Identity-provider outages are retryable on "+path);
    assert.equal((await outage.json() as {code:string}).code,"unavailable");
  }
  console.log("Passed: revisioned library upserts/removals, late and duplicate delivery, rejoin ordering, account deletion before delivery, stop-sharing, deletion tombstones and racing joins at the member cap.");
  console.log("Passed: persistent identity, invitation metadata, private attachments, durable outbox retry/restart and creation reservations.");
} finally { await worker.dispose(); await rm(root,{recursive:true,force:true}); }

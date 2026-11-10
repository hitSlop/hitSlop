/** Real Rust WASM + SQLite DO/R2 qualification, with locally signed identities.
 * Only the local runtime's certificate service is substituted. Production auth
 * has no test key, token bypass, configurable certificate URL or asserted UID path.
 * Run through `bun run verify sharing` after installing worker-build (README). */
import assert from "node:assert/strict";
import { checkCompaction, relayHarness, decode, helloFrame, push, documentName } from "./check-compaction";
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
  const output = join(repository, ".pop/sharing/check-accounts.mjs");
  await mkdir(join(repository, ".pop/sharing"), { recursive: true });
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
  const parts = [{ alg: "RS256", kid: "local-runtime-only" }, { sub: name, aud: "hitslopapp", iss: "https://securetoken.google.com/hitslopapp", iat: now, auth_time: now, exp: now + 3600, firebase: { sign_in_provider: provider }, jti: crypto.randomUUID() }]
    .map(value => Buffer.from(JSON.stringify(value)).toString("base64url")).join(".");
  return parts + "." + sign("RSA-SHA256", Buffer.from(parts), privateKey).toString("base64url");
};
const a = uid("creator-a"), secondSession = uid("creator-a"), b = uid("creator-b");
const document = "00000000-0000-4000-8000-000000000001";
const seed = Buffer.alloc(512, 42);
const app = "a".repeat(64);
const relayConfig = ts.parseConfigFileTextToJson("wrangler.jsonc", await readFile(join(repository, "apps/sharing/wrangler.jsonc"), "utf8")).config as {
  ratelimits: { name: string; simple: { limit: number; period: 60 } }[];
};
const environments = ts.parseConfigFileTextToJson("wrangler.jsonc", await readFile(join(repository, "apps/sharing/wrangler.jsonc"), "utf8")).config;
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
function runtime(roomsAvailable = true) {
  return new Miniflare({
    durableObjectsPersist: join(root, "objects"), r2Persist: join(root, "r2"),
    workers: [{
      name: "relay", modules: true, scriptPath: harness, modulesRoot: "/",
      compatibilityDate: "2025-11-05", modulesRules: [{ type: "ESModule", include: ["**/*.js"], fallthrough: true }, { type: "CompiledWasm", include: ["**/*.wasm"], fallthrough: true }],
      bindings: { FIREBASE_PROJECT: "hitslopapp", CREATION_ENABLED: "1" },
      durableObjects: { ...(roomsAvailable ? { DOCUMENTS: { className: "CloudDocument", useSQLite: true } } : {}), ACCOUNTS: { className: "DocumentAccount", useSQLite: true } },
      r2Buckets: ["SEEDS"],
      analyticsEngineDatasets: { METRICS: { dataset: "dotpop_documents" } },
      ratelimits: Object.fromEntries(relayConfig.ratelimits.map(({ name, simple }) => [name, { simple: name === "ACCOUNT_CREATE_LIMIT" ? { ...simple, limit: 1000 } : simple }])),
      outboundService: "certificates",
    }, {
      name: "certificates", modules: true,
      script: `export default { fetch(request) {
        if(request.url!=='https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com')return new Response(null,{status:502});
        return Response.json(${JSON.stringify({ "local-runtime-only": pem })},{headers:{"Cache-Control":"public,max-age=3600"}});
      } }`,
    }],
  });
}
let worker = runtime();
async function call(path: string, token: string | undefined, method = "GET", body?: unknown, headers: Record<string, string> = {}) {
  const binary = body instanceof Uint8Array;
  const response = await worker.dispatchFetch("https://share.dotpop.click/v1/" + path, {
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
  assert.equal((await call("documents", a, "POST", new Uint8Array((1 << 20) + 1))).status, 413,
    "Oversized creation body is a bounded request refusal");
  assert.equal((await call("documents", a, "POST", new TextEncoder().encode("{not json"))).status, 400,
    "A malformed body is a client error, never a 500");
  const oldSocket = await call("documents/" + document + "/sync", a, "GET", undefined, {
    Upgrade: "websocket", "Sec-WebSocket-Protocol": "pop.sync.v1",
  });
  assert.equal(oldSocket.status, 426, "Old socket protocols are told to update");
  assert.equal((await oldSocket.json() as {code:string}).code, "requires_update");
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
  assert.ok(!joinPage.includes("<script") && !joinPage.includes(invitation) && !joinPage.includes("b".repeat(64)), "The join page runs no code and holds no invitation credential");
  assert.match(rule("/join", "Content-Security-Policy") ?? "", /^default-src 'none'; style-src 'unsafe-inline'; /, "The join page allows no script");
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
  assert.equal((await call(base+"/seed",a,"PUT",seed,seedHeaders(seed))).status,200);
  assert.equal((await call(base+"/blobs",a)).status,200,"The creator lists uploaded blobs before Open");
  assert.equal((await call(base+"/blobs",b)).status,403,"Others cannot");
  assert.equal((await call(base+"/open",a,"POST",{seedSha256:hash(seed)})).status,200);
  // The live account feed is isolated by verified identity and survives reconnects.
  const library = async (token:string) => {
    const reply = await call("account/library", token, "GET", undefined, {Upgrade:"websocket", "Sec-WebSocket-Protocol":"pop.library.v1"});
    assert.equal(reply.status,101);
    assert.equal(reply.headers.get("Sec-WebSocket-Protocol"),"pop.library.v1");
    const socket = reply.webSocket!; socket.accept();
    const queue:any[]=[]; const waiters:((value:any)=>void)[]=[];
    socket.addEventListener("message", (event:any)=>{ if(event.data === "pong")return; const frame=JSON.parse(event.data); const waiter=waiters.shift(); if(waiter)waiter(frame);else queue.push(frame); });
    const next=()=>queue.length ? Promise.resolve(queue.shift()) : new Promise<any>((resolve,reject)=>{
      const timer=setTimeout(()=>reject(new Error("Expected library frame")),5000);
      waiters.push(frame=>{clearTimeout(timer);resolve(frame);});
    });
    return {socket,next};
  };
  const libraryNamespace = await worker.getDurableObjectNamespace("DOCUMENTS");
  const libraryDocument = libraryNamespace.get(libraryNamespace.idFromName(document));
  await libraryDocument.fetch("https://test/__test/alarm");
  const ownerLibrary=await library(a), outsiderLibrary=await library(b);
  const initialLibrary=await ownerLibrary.next();
  assert.equal(initialLibrary.type,"snapshot"); assert.equal(initialLibrary.last,true);
  assert.equal(initialLibrary.entries[0].document,document);
  assert.deepEqual((await outsiderLibrary.next()).entries,[],"An unrelated account sees no private entries");
  const bootstrapAccounts=await worker.getDurableObjectNamespace("ACCOUNTS");
  const bootstrapAccount=bootstrapAccounts.get(bootstrapAccounts.idFromName(hash(Buffer.from("creator-a"))));
  await bootstrapAccount.fetch("https://test/__test/fail-descriptions",{method:"POST",body:JSON.stringify({enabled:true})});
  await json(base,a,"PATCH",{title:"Live title",templateId:"fixture"});
  await libraryDocument.fetch("https://test/__test/alarm");
  const owed=await (await libraryDocument.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT value FROM meta WHERE key='library_pending'"})})).json() as {value:string}[];
  assert.equal(owed[0]!.value,"1","Failed publication stays durable without another edit");
  await bootstrapAccount.fetch("https://test/__test/fail-descriptions",{method:"POST",body:JSON.stringify({enabled:false})});
  await libraryDocument.fetch("https://test/__test/alarm");
  let changed=await ownerLibrary.next();
  while(changed.type === "upsert" && changed.entry.title !== "Live title")changed=await ownerLibrary.next();
  assert.equal(changed.type,"upsert"); assert.equal(changed.entry.title,"Live title");
  ownerLibrary.socket.close(); outsiderLibrary.socket.close();
  await bootstrapAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"DELETE FROM projections"})});
  await libraryDocument.fetch("https://test/__test/hold-library",{method:"POST"});
  const stalledSnapshot=call("account/library",secondSession,"GET",undefined,{Upgrade:"websocket","Sec-WebSocket-Protocol":"pop.library.v1"});
  for(let n=0;n<500;n++) {
    if(await (await libraryDocument.fetch("https://test/__test/library-waiting")).json()) break;
    assert(n<499,"Bootstrap reached the stalled document");
    await new Promise(resolve=>setTimeout(resolve,10));
  }
  const creationDuringBootstrap=await Promise.race([
    call("documents",a,"POST",{document:"00000000-0000-4000-8000-000000000089"}).then(reply=>reply.status),
    new Promise(resolve=>setTimeout(()=>resolve("blocked"),1500)),
  ]);
  assert.equal(creationDuringBootstrap,204,"Bootstrap lookup does not hold the account operations lock");
  assert.equal((await stalledSnapshot).status,503,"A stalled bootstrap has a bounded failure");
  await libraryDocument.fetch("https://test/__test/release-library",{method:"POST"});
  await libraryDocument.fetch("https://test/__test/library-unavailable",{method:"POST",body:JSON.stringify({enabled:true})});
  const unavailableSnapshot=await call("account/library",secondSession,"GET",undefined,{Upgrade:"websocket","Sec-WebSocket-Protocol":"pop.library.v1"});
  assert.equal(unavailableSnapshot.status,503,"An unresolved lookup must not produce authoritative absence");
  await libraryDocument.fetch("https://test/__test/library-unavailable",{method:"POST",body:JSON.stringify({enabled:false})});
  const reconnected=await library(secondSession);
  assert.equal((await reconnected.next()).entries[0].title,"Live title", "Existing registrations backfill without a reset or Firestore");
  reconnected.socket.close();
  assert.equal((await call(base+"/seed",b)).status,403,"private bytes require owner identity");
  assert.equal((await call(base+"/sharing","not-a-valid-identity")).status,401,"invalid identity is retryable authentication, not lost membership");
  assert.equal((await call(base+"/sharing",b)).status,403,"a valid nonmember is denied access");
  assert.equal((await call(base+"/seed",secondSession)).status,200,"same account on another device shares private bytes");
  assert.equal((await json<AccessState>(base+"/sharing",secondSession)).role,"owner");
  const enabled=await json<AccessState>(base+"/sharing",a,"POST");assert(enabled.capability);assert(enabled.sharing);
  // Only the authorized access reply carries the invitation secret; document metadata never does,
  // even for the owner while sharing is on.
  for (const info of [await json<RoomInfo>(base,a), await json<RoomInfo>(base+"/open",a,"POST",{seedSha256:hash(seed)})]) {
    assert.equal(info.document,document);
    assert.equal(info.phase,"live");
    assert.equal(typeof info.compact,"boolean","Every reply states whether compaction is due");
    assert.ok(!("capability" in info) && !JSON.stringify(info).includes(enabled.capability!),"Document metadata never carries the invitation");
  }
  const guest = uid("guest", "anonymous");
  await json(base+"/members",guest,"POST",{capability:enabled.capability});
  assert.equal((await call(base+"/seed",guest)).status,200,"Verified anonymous identities may join");
  assert.equal((await call(base+"/seed",enabled.capability!)).status,403,"Invitation secrets cannot authorize document reads");
  const member=await json<AccessState>(base+"/members",b,"POST",{capability:enabled.capability});assert.equal(member.role,"editor");
  assert.equal((await call(base+"/seed",b)).status,200);
  await libraryDocument.fetch("https://test/__test/alarm");
  const memberLibrary=await library(b);
  const memberSnapshot=await memberLibrary.next();
  assert.equal(memberSnapshot.entries[0].document,document);
  assert.equal(memberSnapshot.entries[0].role,"editor");

  const connect=async(token:string)=>{
    const response=await call(base+"/sync",token,"GET",undefined,{Upgrade:"websocket","Sec-WebSocket-Protocol":SharingHost.socketProtocol});
    assert.equal(response.status,101);const socket=response.webSocket;assert(socket);socket.accept();
    const queue:ReturnType<typeof decode>[]=[];const waiters:((frame:ReturnType<typeof decode>)=>void)[]=[];
    socket.addEventListener("message",(event:any)=>{const frame=decode(event.data);const resolve=waiters.shift();if(resolve)resolve(frame);else queue.push(frame);});
    const next=async()=>queue.shift()??await new Promise<ReturnType<typeof decode>>((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error("Expected socket frame")),5000);waiters.push(frame=>{clearTimeout(timer);resolve(frame);});});
    socket.send(helloFrame());
    assert.equal((await next()).tag,2);assert.equal((await next()).tag,4);
    return {socket,next};
  };
  assert.equal((await call(base+"/sync",undefined,"GET",undefined,{
    Upgrade:"websocket", "Sec-WebSocket-Protocol":`${SharingHost.socketProtocol}, cap.${enabled.capability}`,
  })).status,426,"Credentials in a socket subprotocol are never read");
  const ownerConnection=await connect(a),guestConnection=await connect(guest);
  const ownerSocket=ownerConnection.socket,guestSocket=guestConnection.socket;let ownerClosed=false;
  ownerSocket.addEventListener("close",()=>ownerClosed=true);
  const guestEnded=new Promise<void>(resolve=>guestSocket.addEventListener("close",()=>resolve()));
  const documents = await worker.getDurableObjectNamespace("DOCUMENTS");
  const documentStub = documents.get(documents.idFromName(documentName(document)));
  const revoked=await json<AccessState>(base+"/sharing",a,"DELETE");assert(!revoked.sharing);assert(revoked.generation>enabled.generation);
  assert.equal((await call(base+"/seed",enabled.capability!)).status,403);assert.equal((await call(base+"/seed",guest)).status,403);assert.equal((await call(base+"/seed",b)).status,403);
  assert.equal((await call(base+"/seed",a)).status,200);assert(!ownerClosed,"revocation does not close private owner connection");
  assert.equal((await guestConnection.next()).code,3,"revocation delivers the real access-ended frame");
  try { guestSocket.send(push(2,0,1,new Uint8Array([8]))); } catch { /* Peer is already closing. */ }
  guestSocket.close(); // Acknowledge the server close, as the native/browser client does.
  await Promise.race([guestEnded,new Promise((_,reject)=>setTimeout(()=>reject(new Error("Guest close handshake did not complete")),5000))]);
  ownerSocket.send(push(1,0,1));
  const ownerAck=await ownerConnection.next();assert.equal(ownerAck.tag,6);assert.equal(ownerAck.status,0,"owner still writes after invitation revocation");
  assert.equal((await ownerConnection.next()).tag,3,"owner still receives live accepted updates");
  const heads=await (await documentStub.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT value FROM meta WHERE key='heads'"})})).json() as {value:string}[];
  assert.deepEqual(JSON.parse(heads[0]!.value),[["0000000000000001",1]],"a revoked guest push never enters the accepted frontier");
  ownerSocket.close();
  const reopened=await json<AccessState>(base+"/sharing",a,"POST");assert.notEqual(reopened.capability,enabled.capability);
  const staying=uid("staying-guest","anonymous");
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
  const accountNamespace=await worker.getDurableObjectNamespace("ACCOUNTS");
  const joiner=(name:string)=>{
    const account=accountNamespace.get(accountNamespace.idFromName(hash(Buffer.from(name))));
    const hook=(path:string,body?:unknown)=>account.fetch("https://test/__test/"+path,{method:"POST",...(body===undefined?{}:{body:JSON.stringify(body)})});
    const count=async(kind:string)=>Number(await (await account.fetch("https://test/__test/held-"+kind)).text());
    // A registration held before the account handles it, or stored with its reply held.
    const held=async()=>await count("registrations")>0,stored=async()=>await count("registration-responses")>0;
    return {name,token:uid(name,"anonymous"),account,hook,held,stored};
  };
  const registered = async(token:string,id:string) => {
    const name = JSON.parse(Buffer.from(token.split(".")[1]!, "base64url").toString()).sub;
    const account = accountNamespace.get(accountNamespace.idFromName(hash(Buffer.from(name))));
    return (await stubSQL<{n:number}>(account,"SELECT count(*) AS n FROM documents WHERE document=?",[id]))[0]!.n > 0;
  };
  const queuedAt=async(stub:typeof documentStub,name:string)=>(await stubSQL<{n:number}>(stub,"SELECT count(*) AS n FROM pending_registry WHERE uid=?",[name]))[0]!.n>0;
  assert.equal(await readAttachment(b),200,"A member reads attachments");
  // Leave revokes before it releases the account's hint. Here the account commits the release
  // but its reply is lost: leaving still ends access at once, and the release stays queued.
  const leaver=joiner("creator-b");
  await leaver.hook("release-failure",{enabled:true,committed:true});
  assert.equal((await call(base+"/members/me",b,"DELETE")).status,200,"Leave succeeds whatever the account replies");
  assert.equal((await call(base+"/seed",b)).status,403,"leave removes identity membership without deleting canonical bytes");
  let removal = await memberLibrary.next();
  while(removal.type !== "remove")removal = await memberLibrary.next();
  assert.equal(removal.document, document, "Leave removes the entry live");
  memberLibrary.socket.close();

  assert.equal(await readAttachment(b),403,"Leaving ends that member's attachment access");
  assert.equal(await readAttachment(staying),200,"Another member keeps attachment access");
  assert.ok(!await registered(b,document)&&await queuedAt(documentStub,leaver.name),"The release without a reply stays queued");
  await leaver.hook("release-failure",{enabled:false});
  assert.equal((await documentStub.fetch("https://test/__test/alarm")).status,200);
  assert.ok(!await queuedAt(documentStub,leaver.name),"Cleanup completes the queued release");
  // Membership on one document: joins, leaves and registry cleanup run one at a time. A join
  // registers with its account before it inserts the member, so a member always has its
  // account hint; a hint without a member is a stale entry that a retried cleanup releases.
  const sleep=(ms:number)=>new Promise(resolve=>setTimeout(resolve,ms));
  const until=async(check:()=>Promise<boolean>,message:string)=>{
    for(let n=0;n<500;n++) { if(await check()) return; await sleep(10); }
    assert.fail(message);
  };
  const liveDocument=async(id:string)=>{
    const base="documents/"+id;
    assert.equal((await call("documents",a,"POST",{document:id})).status,204);
    assert.equal((await call(base+"/seed",a,"PUT",seed,seedHeaders(seed))).status,200);
      assert.equal((await call(base+"/open",a,"POST",{seedSha256:hash(seed)})).status,200);
    return {id,base,stub:documents.get(documents.idFromName(documentName(id))),invite:await json<AccessState>(base+"/sharing",a,"POST")};
  };
  const idle="00000000-0000-4000-8000-000000000095";
  const {base:idleBase,stub:idleStub,invite:idleInvite}=await liveDocument(idle);
  const joinIdle=(token:string)=>call(idleBase+"/members",token,"POST",{capability:idleInvite.capability});
  const cleanup=async()=>assert.equal((await idleStub.fetch("https://test/__test/alarm")).status,200);
  const queued=(name:string)=>queuedAt(idleStub,name);
  const memberRows=async(name:string)=>(await stubSQL<{n:number}>(idleStub,"SELECT count(*) AS n FROM members WHERE uid=?",[name]))[0]!.n;
  // A non-member's leave makes no account call (its account refuses releases) and does not
  // disturb a join in progress.
  const waiter=joiner("idle-join"),stranger=joiner("stranger");
  await stranger.hook("release-failure",{enabled:true});
  await waiter.hook("hold-registration");
  const waitingJoin=joinIdle(waiter.token);
  await until(waiter.held,"The join awaits its account registration");
  const strangerLeave=call(idleBase+"/members/me",stranger.token,"DELETE");
  await waiter.hook("release-registration");
  assert.equal((await strangerLeave).status,200,"A non-member's leave succeeds without an account call");
  assert.equal((await waitingJoin).status,200,"A non-member's leave does not disturb a concurrent join");
  assert.equal((await call(idleBase+"/seed",waiter.token)).status,200);
  // The account stores a registration but its reply fails: the join fails closed, and cleanup
  // releases the stale hint, keeping it queued while the account refuses the release.
  const lost=joiner("lost-reply");
  await lost.hook("release-failure",{enabled:true});
  await lost.hook("fail-registration-response");
  assert.equal((await joinIdle(lost.token)).status,409);
  assert.equal((await call(idleBase+"/seed",lost.token)).status,403,"A failed registration reply authorizes nothing");
  assert.ok(await registered(lost.token,idle)&&await queued(lost.name),"The stored hint is queued for release");
  await cleanup();
  assert.ok(await registered(lost.token,idle)&&await queued(lost.name),"A refused release stays queued");
  await lost.hook("release-failure",{enabled:false});
  await cleanup();
  assert.ok(!await registered(lost.token,idle)&&!await queued(lost.name),"A retried cleanup releases the stale hint");
  // No reply in time is an unknown outcome: the join fails closed, and its queued release
  // (whose alarm may already have run it) removes what the account stored.
  const slow=joiner("slow-account");
  await slow.hook("hold-registration-response");
  assert.equal((await joinIdle(slow.token)).status,503,"A registration without a timely reply fails closed");
  assert.ok(await slow.stored()&&await memberRows(slow.name)===0,"The account stored it; the document did not admit it");
  await slow.hook("finish-registration-response");
  await cleanup();
  assert.ok(!await registered(slow.token,idle)&&!await queued(slow.name),"Cleanup releases the hint of a timed-out registration");
  // A reset after the account stored a registration, before the join recorded anything more:
  // the release was queued with its alarm before the call, so cleanup still finds the hint.
  const resetDocument=await liveDocument("00000000-0000-4000-8000-000000000092");
  const reset=joiner("reset-join");
  await reset.hook("hold-registration-response");
  const resetJoin=call(resetDocument.base+"/members",reset.token,"POST",{capability:resetDocument.invite.capability});
  await until(reset.stored,"The account stores the registration");
  try { await resetDocument.stub.fetch("https://test/__test/reset"); } catch { /* The reset fails its own request. */ }
  assert.notEqual((await resetJoin).status,200,"A join interrupted by a reset fails");
  assert.equal((await call(resetDocument.base+"/seed",reset.token)).status,403,"A join interrupted by a reset admits no one");
  await reset.hook("finish-registration-response");
  const restarted=documents.get(documents.idFromName(documentName(resetDocument.id))); // A reset breaks old stubs.
  assert.ok(await registered(reset.token,resetDocument.id),"The account kept the registration");
  assert.ok(await queuedAt(restarted,reset.name),"Its release outlives the reset");
  assert.notEqual(await (await restarted.fetch("https://test/__test/alarm-time")).json(),null,
    "Its cleanup alarm outlives the reset");
  assert.equal((await restarted.fetch("https://test/__test/alarm")).status,200);
  assert.ok(!await registered(reset.token,resetDocument.id)&&!await queuedAt(restarted,reset.name),
    "Cleanup releases the hint the interrupted join left");
  // A delayed cleanup of a failed attempt never removes the registration of the rejoin that
  // succeeds meanwhile. The rejoin's registration is stored, its reply held; the cleanup has
  // read the queued row and waits for the rejoin, then finds a member.
  const rejoin=joiner("rejoin");
  await rejoin.hook("fail-registration-response");
  assert.equal((await joinIdle(rejoin.token)).status,409);
  assert.ok(await queued(rejoin.name));
  await rejoin.hook("hold-registration-response");
  const rejoinRetry=joinIdle(rejoin.token);
  await until(rejoin.stored,"The rejoin's registration is stored");
  const delayed=cleanup();
  await stubSQL(idleStub,"SELECT 1"); // Answered once the cleanup has read the row and waits.
  await rejoin.hook("finish-registration-response");
  assert.equal((await rejoinRetry).status,200);
  await delayed;
  assert.ok(await registered(rejoin.token,idle),"A successful rejoin keeps its registration through a delayed cleanup");
  assert.equal((await call(idleBase+"/seed",rejoin.token)).status,200,"The rejoined member still works");
  assert.ok(!await queued(rejoin.name));
  // Concurrent joins by one identity run in turn: the first fails, the second succeeds, and
  // cleanup leaves one member with its registration.
  const twice=joiner("same-identity");
  await twice.hook("hold-registration");
  await twice.hook("fail-registration-response");
  const firstJoin=joinIdle(twice.token);
  await until(twice.held,"The first join awaits its account registration");
  const secondJoin=joinIdle(twice.token);
  await sleep(50); // Lets the second join reach the document.
  await twice.hook("release-registration");
  assert.equal((await firstJoin).status,409);
  assert.equal((await secondJoin).status,200);
  await cleanup();
  assert.equal(await memberRows(twice.name),1);
  assert.ok(await registered(twice.token,idle),"The failed attempt's cleanup keeps the successful join's registration");
  // Account deletion while that account's join is pending. Deletion holds the account's lock
  // while its leave waits for the document's; the join's registration needs neither, so the
  // fence refuses it and nothing deadlocks. The leave then removes the existing member.
  const fenced=(account:typeof documentStub)=>until(async()=>
    (await stubSQL<{n:number}>(account,"SELECT count(*) AS n FROM deleting"))[0]!.n>0,"Account deletion is fenced");
  const deleting=joiner("deleting-pending-join");
  assert.equal((await joinIdle(deleting.token)).status,200);
  await deleting.hook("hold-registration");
  const deletingJoin=joinIdle(deleting.token);
  await until(deleting.held,"The rejoin awaits its account registration");
  const accountDeletion=call("account",deleting.token,"DELETE");
  await fenced(deleting.account);
  await sleep(50); // Lets deletion's leave reach the document and wait for the join.
  await deleting.hook("release-registration");
  assert.equal((await deletingJoin).status,409,"The fenced account refuses the pending registration");
  assert.equal((await accountDeletion).status,200,"Account deletion completes during a pending join");
  assert.equal(await memberRows(deleting.name),0,"A deleted account keeps no membership");
  assert.equal((await call(idleBase+"/seed",deleting.token)).status,403);
  // The account stored a join's registration just before its deletion: deletion's leave waits
  // for that join to insert the member, then removes it.
  const stored=joiner("deleting-stored-join");
  await stored.hook("hold-registration-response");
  const storedJoin=joinIdle(stored.token);
  await until(stored.stored,"The account stores the registration");
  const storedDeletion=call("account",stored.token,"DELETE");
  await fenced(stored.account);
  await sleep(50); // Lets deletion's leave reach the document and wait for the join.
  await stored.hook("finish-registration-response");
  assert.equal((await storedJoin).status,200,"The join completes before deletion's leave");
  assert.equal((await storedDeletion).status,200);
  assert.equal(await memberRows(stored.name),0,"Deletion's leave removes a member whose join was pending");
  // Revocation does not wait for a pending join; the join fails closed and cleanup releases
  // the hint it stored.
  const revokedJoiner=joiner("idle-refused");
  await revokedJoiner.hook("hold-registration");
  const revokedJoin=joinIdle(revokedJoiner.token);
  await until(revokedJoiner.held,"The join awaits its account registration");
  assert.equal((await call(idleBase+"/sharing",a,"DELETE")).status,200,"Revocation does not wait for a pending join");
  await revokedJoiner.hook("release-registration");
  assert.equal((await revokedJoin).status,403);
  assert.equal(await memberRows(revokedJoiner.name),0,"A join revoked during its registration is not a member");
  await cleanup();
  assert.ok(!await registered(revokedJoiner.token,idle),"Cleanup releases the revoked join's hint");
  // Deletion likewise ends a document at once. The pending join fails closed; deletion's
  // registry cleanup waits for it, and the next cleanup releases its hint.
  const stopping=await liveDocument("00000000-0000-4000-8000-000000000094");
  const stoppingMember=joiner("stopping-member"),stopper=joiner("stopping-join");
  assert.equal((await call(stopping.base+"/members",stoppingMember.token,"POST",{capability:stopping.invite.capability})).status,200);
  await stopper.hook("hold-registration");
  const stoppingJoin=call(stopping.base+"/members",stopper.token,"POST",{capability:stopping.invite.capability});
  await until(stopper.held,"The join awaits its account registration");
  const stoppingDeletion=call(stopping.base,a,"DELETE");
  await until(async()=>JSON.stringify(await stubSQL(stopping.stub,"SELECT value FROM meta WHERE key='phase'")).includes("ended"),"Deletion ends the document");
  await stopper.hook("release-registration");
  assert.equal((await stoppingJoin).status,403,"A join pending at deletion fails closed");
  assert.equal((await stoppingDeletion).status,200);
  assert.equal((await stopping.stub.fetch("https://test/__test/alarm")).status,200);
  for (const token of [stoppingMember.token,stopper.token])
    assert.ok(!await registered(token,"00000000-0000-4000-8000-000000000094"),"Deletion releases every member's and pending join's hint");
  // Open names the seed it makes live: an older bootstrap attempt cannot publish the seed
  // a later attempt uploaded. Nothing else fences it; attachments are the creator's to upload first.
  const competing="00000000-0000-4000-8000-000000000093",competingBase="documents/"+competing;
  const laterSeed=Buffer.alloc(512,43);
  await call("documents",a,"POST",{document:competing});
  await call(competingBase+"/seed",a,"PUT",seed,seedHeaders(seed));
  await call(competingBase+"/seed",a,"PUT",laterSeed,seedHeaders(laterSeed));
  assert.equal((await call(competingBase+"/open",a,"POST")).status,400,"Open requires the seed it makes live");
  assert.equal((await call(competingBase+"/open",a,"POST",{seedSha256:hash(seed)})).status,409,
    "An older bootstrap cannot publish a newer seed");
  assert.equal((await json<RoomInfo>(competingBase,a)).phase,"uploading");
  assert.equal((await call(competingBase+"/open",a,"POST",{seedSha256:hash(laterSeed)})).status,200);
  assert.equal((await call(competingBase+"/open",a,"POST",{seedSha256:hash(seed)})).status,409,
    "A live document's Open still names only its own seed");
  // Two new identities race for the last of 64 member places. Members go straight into the
  // document: 63 real joins would only repeat the join path. The first join's registration
  // is held, so it has counted 63 members; the second arrives meanwhile.
  const crowded="00000000-0000-4000-8000-000000000096",crowdedDocument=await liveDocument(crowded);
  const crowdedJoin=(token:string)=>call(crowdedDocument.base+"/members",token,"POST",{capability:crowdedDocument.invite.capability});
  const crowdedMembers=async()=>(await stubSQL<{n:number}>(crowdedDocument.stub,"SELECT count(*) AS n FROM members"))[0]!.n;
  await stubSQL(crowdedDocument.stub,"WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<63) INSERT INTO members SELECT 'filler-'||i,? FROM n",[crowdedDocument.invite.generation]);
  const held=joiner("crowd-held"),racing=joiner("crowd-racing");
  await held.hook("hold-registration");
  const heldJoin=crowdedJoin(held.token);
  await until(held.held,"The first join awaits its account registration after counting 63 members");
  const racingJoin=crowdedJoin(racing.token);
  await sleep(50); // Lets the second join reach the document.
  await held.hook("release-registration");
  const statuses=[(await heldJoin).status,(await racingJoin).status];
  assert.deepEqual([...statuses].sort((x,y)=>x-y),[200,429],"Exactly one racing join takes the last place");
  assert.equal(await crowdedMembers(),64,"The member limit holds under concurrent joins");
  const [admitted,refused]=statuses[0]===200?[held,racing]:[racing,held];
  assert.ok(await registered(admitted.token,crowded),"The admitted member's account lists the document");
  assert.ok(!await registered(refused.token,crowded),"A join refused for the limit leaves no account registration");
  assert.equal((await crowdedJoin(admitted.token)).status,200,"A member joining again at the limit keeps its place");
  assert.ok(await crowdedMembers()===64&&await registered(admitted.token,crowded),"A rejoining member counts once and keeps its registration");
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
  const lateMeta=await (await lateStub.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT key FROM meta WHERE key IN ('title','template_id')"})})).json();
  assert.deepEqual(lateMeta,[],"A deleted document keeps no metadata written after deletion");
  assert.equal((await call("documents/"+lateDocument+"/metadata",a)).status,410);
  const repairDocument="00000000-0000-4000-8000-000000000098";
  const repairBase="documents/"+repairDocument;
  assert.equal((await call("documents",a,"POST",{document:repairDocument})).status,204);
  assert.equal((await call(repairBase+"/seed",a,"PUT",seed,seedHeaders(seed))).status,200);
  assert.equal((await call(repairBase+"/open",a,"POST",{seedSha256:hash(seed)})).status,200);
  await worker.dispose();worker=runtime();
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
  const metadata=await (await call(base+"/metadata",secondSession)).json();
  assert.deepEqual(metadata,{title:"Repairable private document",templateId:"fixture"});
  const departed = await library(b);
  assert.deepEqual((await departed.next()).entries,[],"Leave removes the guest library entry after restart");
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
  const intentAccounts=await worker.getDurableObjectNamespace("ACCOUNTS");
  const intentAccount=intentAccounts.get(intentAccounts.idFromName(hash(Buffer.from(intentName))));
  const intentSQL=async(sql:string,args:unknown[]=[])=>
    (await intentAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql,args})})).json();
  await intentSQL("INSERT INTO creation_intents VALUES(?)",[intentId]);
  const intentDocs=await worker.getDurableObjectNamespace("DOCUMENTS");
  const intentDoc=intentDocs.get(intentDocs.idFromName(intentId));
  assert.equal((await intentDoc.fetch(`https://test/internal/documents/${intentId}/init`,{method:"POST",body:JSON.stringify({owner_uid:intentName,document:intentId})})).status,204);
  await worker.dispose(); worker=runtime();
  const recovering=await library(intentToken);
  assert.deepEqual((await recovering.next()).entries,[],"Unfinished uploads remain hidden after intent recovery");
  recovering.socket.close();
  const recoveredAccounts=await worker.getDurableObjectNamespace("ACCOUNTS");
  const recoveredAccount=recoveredAccounts.get(recoveredAccounts.idFromName(hash(Buffer.from(intentName))));
  const recoveredRows=await (await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"SELECT owned FROM documents WHERE document=?",args:[intentId]})})).json();
  assert.deepEqual(recoveredRows,[{owned:1}],"Restart recovers the owned registration from its durable intent");
  // Deletion must also cover an intent not yet recovered by a library connection.
  await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"DELETE FROM documents WHERE document=?",args:[intentId]})});
  await recoveredAccount.fetch("https://test/__test/sql",{method:"POST",body:JSON.stringify({sql:"INSERT INTO creation_intents VALUES(?)",args:[intentId]})});
  assert.equal((await call("account",intentToken,"DELETE")).status,200);
  assert.equal((await call("documents/"+intentId,intentToken)).status,410,"Account deletion includes unfinished initialization intents");
  console.log("Passed: persistent UUID/private identity, anonymous links, generation-bound members, member-bound attachment reads, fail-closed and reset-safe joins, revoke-first leaves and registry cleanup, the member limit under racing joins, owner-preserving revocation/leave, restart-safe registry repair, deletion tombstones.");
} finally { await worker.dispose(); await rm(root,{recursive:true,force:true}); }

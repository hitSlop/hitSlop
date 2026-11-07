/** Real Rust WASM + SQLite DO/R2 qualification, with locally signed identities.
 * Only the local runtime's certificate service is substituted. Production auth
 * has no test key, token bypass, configurable certificate URL or asserted UID path.
 * Run through `bun run verify sharing` after installing worker-build (README). */
import assert from "node:assert/strict";
import { checkCompaction, relayHarness, decode, helloFrame, push, documentName } from "./check-compaction";
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
import type { AccessState, RegisteredDocuments, RoomInfo } from "../../packages/pop/src/wire/sharing.generated";

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
      bindings: { BROWSER_ORIGIN: "https://share.dotpop.click", FIREBASE_PROJECT: "hitslopapp", CREATION_ENABLED: "1" },
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
  const unused = { async fetch(): Promise<Response> { throw new Error("Static auth/AASA routes must not use assets or relay"); } };
  const configURL = `${SharingHost.origin}/auth/config`;
  assert.equal((await host.fetch(new Request(configURL), { ASSETS: unused, RELAY: unused })).status,503,
    "Missing public Firebase configuration is retryable");
  const configured = await host.fetch(new Request(configURL), {
    ASSETS: unused, RELAY: unused, FIREBASE_PROJECT: "test-project", FIREBASE_WEB_API_KEY: "test-public-key",
  });
  assert.equal(configured.status,200);
  assert.deepEqual(await configured.json(),{projectId:"test-project",apiKey:"test-public-key"});
  assert.equal(configured.headers.get("Cache-Control"),"no-store");
  assert.equal((await host.fetch(new Request(`https://copy.${SharingHost.domain}/auth/config`), {
    ASSETS: unused, RELAY: unused, FIREBASE_PROJECT:"test-project", FIREBASE_WEB_API_KEY:"test-public-key",
  })).status,404,"Authored origins cannot read host identity configuration");
  const callbackURL = "https://share.dotpop.click/auth/apple";
  const callback = await host.fetch(new Request(callbackURL), { ASSETS: unused, RELAY: unused });
  assert.equal(callback.status, 200);
  assert.equal(callback.headers.get("Content-Type"), "text/html; charset=utf-8");
  assert.equal(callback.headers.get("Cache-Control"), "no-store");
  assert.equal(callback.headers.get("Referrer-Policy"), "no-referrer");
  const callbackPage = await callback.text();
  assert.ok(callbackPage.includes("dotpop://auth/apple"));
  const script = /<script>([\s\S]*?)<\/script>/.exec(callbackPage)![1]!;
  const digest = createHash("sha256").update(script).digest("base64");
  assert.equal(callback.headers.get("Content-Security-Policy"),
    `default-src 'none'; script-src 'sha256-${digest}'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`);
  for (const fragment of ["#state=example&id_token=a.b.c&code=a%2Bb%26c", "#state=example&error=user_cancelled_authorize", ""]) {
    let destination: string | undefined;
    runInNewContext(script, { location: { hash: fragment, replace(url: string) { destination = url; } } });
    assert.equal(destination, fragment ? "dotpop://auth/apple" + fragment : undefined);
  }
  const callbackHead = await host.fetch(new Request(callbackURL, { method: "HEAD" }), { ASSETS: unused, RELAY: unused });
  assert.equal(callbackHead.status, 200);
  assert.equal(await callbackHead.text(), "");
  assert.equal((await host.fetch(new Request(callbackURL, { method: "POST" }), { ASSETS: unused, RELAY: unused })).status, 405);
  assert.equal((await host.fetch(new Request(`https://copy.${SharingHost.domain}/auth/apple`), { ASSETS: unused, RELAY: unused })).status, 404);
  assert.equal((await host.fetch(new Request("https://other.example/auth/apple"), { ASSETS: unused, RELAY: unused })).status, 403);
  const associationURL = "https://share.dotpop.click/.well-known/apple-app-site-association";
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
  assert.equal((await host.fetch(new Request(associationURL.replace("share.dotpop.click", "copy.dotslop.com")), { ASSETS: unused, RELAY: unused })).status, 404);
  const assets = { async fetch(request: Request) {
    const path = new URL(request.url).pathname;
    assert.ok(path === "/index.html" || path === "/share-icon.png", "Only the public preview assets are requested");
    const bytes = await readFile(join(repository, "generated/sharing/assets", path));
    return new Response(bytes, { headers: { "Content-Type": path.endsWith(".png") ? "image/png" : "text/html" } });
  } };
  const invitation = `${SharingHost.origin}/join#${document}.${"b".repeat(64)}`;
  let preview: string | undefined;
  for (const url of [SharingHost.origin + "/", SharingHost.origin + "/join", invitation]) {
    const response = await host.fetch(new Request(url), { ASSETS: assets, RELAY: unused });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("Content-Security-Policy"), SharingHost.hostPolicy);
    const html = await response.text();
    assert.match(html, /<title>Open a shared pop<\/title>/);
    for (const tag of [
      '<meta property="og:title" content="Open a shared pop"',
      '<meta property="og:site_name" content="dotpop"',
      '<meta property="og:type" content="website"',
      '<meta property="og:description" content="Open this invitation in dotpop to edit together."',
      `<meta property="og:image" content="${SharingHost.origin}/share-icon.png"`,
      `<link rel="apple-touch-icon" href="${SharingHost.origin}/share-icon.png"`,
    ]) assert.ok(html.includes(tag), `Preview metadata is present without running JavaScript: ${tag}`);
    assert.ok(!html.includes(invitation) && !html.includes("b".repeat(64)), "Preview metadata contains no invitation credential");
    if (preview) assert.equal(html, preview, "All invitations have the same public preview");
    preview = html;
  }
  for (const method of ["GET", "HEAD"]) {
    const response = await host.fetch(new Request(`${SharingHost.origin}/share-icon.png`, { method }), { ASSETS: assets, RELAY: unused });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("Content-Type"), "image/png");
    const image = Buffer.from(await response.arrayBuffer());
    if (method === "HEAD") assert.equal(image.length, 0);
    else {
      assert.equal(image.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
      assert.ok(image.readUInt32BE(16) >= 900 && image.readUInt32BE(20) >= 900, "Messages receives a high-resolution preview image");
    }
  }
  assert.equal((await host.fetch(new Request(`${SharingHost.origin}/share-icon.png`, { method: "POST" }), { ASSETS: unused, RELAY: unused })).status, 405);
  for (const url of [`${SharingHost.origin}/unlisted.png`, `https://copy.${SharingHost.domain}/share-icon.png`])
    assert.equal((await host.fetch(new Request(url), { ASSETS: unused, RELAY: unused })).status, 404);

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
  assert.equal((await call(base+"/resources",a,"POST",{app,resources:[]})).status,200);
  assert.equal((await call(base+"/open",a,"POST")).status,200);
  assert.equal((await call(base+"/seed",b)).status,403,"private bytes require owner identity");
  assert.equal((await call(base+"/sharing","not-a-valid-identity")).status,401,"invalid identity is retryable authentication, not lost membership");
  assert.equal((await call(base+"/sharing",b)).status,403,"a valid nonmember is denied access");
  assert.equal((await call(base+"/seed",secondSession)).status,200,"same account on another device shares private bytes");
  assert.equal((await json<AccessState>(base+"/sharing",secondSession)).role,"owner");
  const enabled=await json<AccessState>(base+"/sharing",a,"POST");assert(enabled.capability);assert(enabled.sharing);
  // Only the authorized access reply carries the invitation secret; document metadata never does,
  // even for the owner while sharing is on.
  for (const info of [await json<RoomInfo>(base,a), await json<RoomInfo>(base+"/open",a,"POST")]) {
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
  const browserUpgrade = await call(base+"/sync",undefined,"GET",undefined,{
    Upgrade:"websocket", Origin:SharingHost.origin,
    "Sec-WebSocket-Protocol":`${SharingHost.socketProtocol}, jwt.${guest}`,
  });
  assert.equal(browserUpgrade.status,101,"Browsers authenticate with a verified JWT subprotocol");
  assert.equal(browserUpgrade.headers.get("Sec-WebSocket-Protocol"),SharingHost.socketProtocol,"Never echo credentials");
  assert(browserUpgrade.webSocket);
  browserUpgrade.webSocket.accept(); browserUpgrade.webSocket.close();
  assert.equal((await call(base+"/sync",undefined,"GET",undefined,{
    Upgrade:"websocket", Origin:SharingHost.origin,
    "Sec-WebSocket-Protocol":`${SharingHost.socketProtocol}, cap.${enabled.capability}`,
  })).status,403,"Capability socket authentication is refused");
  const ownerConnection=await connect(a),guestConnection=await connect(guest);
  const ownerSocket=ownerConnection.socket,guestSocket=guestConnection.socket;let ownerClosed=false;
  ownerSocket.addEventListener("close",()=>ownerClosed=true);
  const guestEnded=new Promise<void>(resolve=>guestSocket.addEventListener("close",()=>resolve()));
  const documents = await worker.getDurableObjectNamespace("DOCUMENTS");
  const documentStub = documents.get(documents.idFromName(documentName(document)));
  let finishGrant!: () => void;
  const grantBody = new ReadableStream<Uint8Array>({ start(controller) {
    controller.enqueue(new TextEncoder().encode(JSON.stringify({ app })));
    finishGrant = () => controller.close();
  } });
  const grantInit = { method:"POST", headers: { Origin:SharingHost.origin, Authorization:`Bearer ${guest}`,
    "Content-Type":"application/json", "X-Test-Observe-Body":"1" }, body:grantBody, duplex:"half" as const };
  const delayedGrant = worker.dispatchFetch(`https://${document}.${SharingHost.domain}/_resources/${document}`, grantInit);
  let readStarted = false;
  for (let attempt=0; attempt<500; attempt++) {
    const progress = await documentStub.fetch("https://test/__test/body-reads");
    if (Number(await progress.text()) > 0) { readStarted = true; break; }
    await new Promise(resolve=>setTimeout(resolve,10));
  }
  assert.ok(readStarted,"The resource grant is waiting for request body EOF after initial authorization");
  const revoked=await json<AccessState>(base+"/sharing",a,"DELETE");assert(!revoked.sharing);assert(revoked.generation>enabled.generation);
  finishGrant();
  assert.equal((await delayedGrant).status,403,"Revocation during a resource request body must not mint a fresh cookie");
  const resourcePreflight = await worker.dispatchFetch(`https://${document}.${SharingHost.domain}/_resources/${document}`, {
    method:"OPTIONS", headers: { Origin:SharingHost.origin,
      "Access-Control-Request-Method":"POST", "Access-Control-Request-Headers":"authorization" },
  });
  assert.equal(resourcePreflight.status,204);
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
  await json(base+"/members",b,"POST",{capability:reopened.capability});
  assert.equal((await call(base+"/members/me",b,"DELETE")).status,200);
  assert.equal((await call(base+"/seed",b)).status,403,"leave removes identity membership without deleting canonical bytes");
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
  assert.equal((await call(repairBase+"/resources",a,"POST",{app,resources:[]})).status,200);
  assert.equal((await call(repairBase+"/open",a,"POST")).status,200);
  await worker.dispose();worker=runtime();
  const registry=await (await call("account/documents",secondSession)).json() as RegisteredDocuments;
  assert(registry.documents.some(row=>row.document===document&&row.role==="owner"));
  assert(registry.documents.some(row=>row.document===repairDocument&&row.role==="owner"),"an acknowledged cloud upload remains discoverable for client catalog repair");
  const metadata=await (await call(base+"/metadata",secondSession)).json();
  assert.deepEqual(metadata,{title:"Repairable private document",templateId:"fixture"});
  assert.deepEqual(await (await call("account/documents",b)).json(),{documents:[]},"leave removes the guest registry hint");
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
  const listed = (await (await call("account/documents", prolific)).json()) as RegisteredDocuments;
  assert.equal(listed.documents.length, 12);
  assert.equal(listed.next, undefined, "a listing that fits one page has no cursor");
  const after = listed.documents[4]!.document;
  const rest = (await (await call("account/documents?after=" + after, prolific)).json()) as RegisteredDocuments;
  assert.deepEqual(rest.documents, listed.documents.slice(5), "a cursor lists the registrations after it in document order");
  assert.equal((await call("account/documents?after=invalid", prolific)).status, 400, "an invalid cursor is refused");
  // Past one page (ACCOUNT_DOCUMENTS_PAGE = 1000), the real handler pages by cursor: every
  // registration exactly once, in document order. Rows go straight into the account's
  // registry; account creation's own rate limit would otherwise bound the fixture.
  const pagedName = "paged-account";
  const paged = uid(pagedName);
  const accounts = await worker.getDurableObjectNamespace("ACCOUNTS");
  const pagedAccount = accounts.get(accounts.idFromName(hash(Buffer.from(pagedName))));
  const registrations = 2345;
  const accountSQL = async (sql: string, args: unknown[] = []) =>
    (await pagedAccount.fetch("https://test/__test/sql", { method: "POST", body: JSON.stringify({ sql, args }) })).json() as Promise<{ n: number }[]>;
  await accountSQL("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<?) INSERT INTO documents SELECT printf('00000000-0000-4000-8000-%012x',i), CASE WHEN i%3=0 THEN 0 ELSE 1 END, 0 FROM n", [registrations]);
  assert.equal((await accountSQL("SELECT count(*) AS n FROM documents"))[0]!.n, registrations);
  const expected = Array.from({ length: registrations }, (_, index) => ({
    document: `00000000-0000-4000-8000-${(index + 1).toString(16).padStart(12, "0")}`,
    role: (index + 1) % 3 === 0 ? "editor" : "owner",
  }));
  const pages: RegisteredDocuments[] = [];
  let cursor: string | undefined;
  do {
    const page = await json<RegisteredDocuments>("account/documents" + (cursor ? "?after=" + cursor : ""), paged);
    pages.push(page);
    cursor = page.next;
    assert.ok(pages.length <= 4, "the cursor reaches the last page");
  } while (cursor);
  assert.deepEqual(pages.map((page) => page.documents.length), [1000, 1000, 345], "full pages, then the remainder");
  for (const page of pages.slice(0, -1))
    assert.equal(page.next, page.documents.at(-1)!.document, "a full page's cursor is its last document");
  assert.equal(pages.at(-1)!.next, undefined, "the last page has no cursor");
  assert.deepEqual(pages.flatMap((page) => page.documents), expected, "every registration once, in order");
  for (const path of ["documents/" + document + "/blob/" + "b".repeat(64) + "/extra", "account/documents/" + document, "account/documents/" + document + "/extra", "account/extra"]) {
    assert.equal((await call(path, undefined, "GET")).status, 404, "unlisted routes never reach a handler: " + path);
  }
  assert.equal((await call("documents/" + document + "/members", prolific, "DELETE")).status, 404, "only members/me leaves");
  console.log("Passed: persistent UUID/private identity, anonymous links, generation-bound members, owner-preserving revocation/leave, restart-safe registry repair, deletion tombstones.");
} finally { await worker.dispose(); await rm(root,{recursive:true,force:true}); }

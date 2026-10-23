/** Explicit deployed sharing qualification, using only disposable infrastructure fixtures.
 * Run after build/deploy: bun scripts/sharing/check-live.ts
 * No invitation, capability or identity token is printed or included in reports. */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdir, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type BrowserContext, type Page } from "playwright";
import { Database } from "bun:sqlite";
import { buildTemplate } from "../../packages/hitslop/src/cli/template";
import { execute } from "../../packages/hitslop/src/cli/engine";
import { repository } from "../lib/artifacts";
import { SharingHost } from "../../packages/hitslop/src/wire/constants.generated";
import type { Batch } from "../../packages/hitslop/src/wire/core.generated";

const root=join(repository,".hitslop/sharing",`check-${crypto.randomUUID()}`);
await mkdir(root,{recursive:true,mode:0o700});
const checks:string[]=[];
const note=(value:string)=>{checks.push(value);console.log(value);};
const report=join(root,"report.json");
const template=await buildTemplate(join(repository,"tests/apps/sharing"),join(root,"fixture.slop"),{renderViews:false});
const invitationFile=process.env.HITSLOP_SHARING_INVITATION_FILE;
const child=spawn(join(repository,"target/debug/examples/sharing"),[template,root,invitationFile ?? process.env.HITSLOP_SHARING_TOKEN_FILE ?? join(repository,".hitslop/sharing/identity-token"),...(invitationFile?["--guest"]:[])],{stdio:["pipe","pipe","pipe"]});
let failure="";child.stderr.on("data",bytes=>{failure+=String(bytes);});
const messages:any[]=[];let waiting:((value:any)=>void)|undefined;
createInterface({input:child.stdout}).on("line",line=>{const value=JSON.parse(line);if(waiting){const next=waiting;waiting=undefined;next(value);}else messages.push(value);});
const next=async()=>{
  if(messages.length)return messages.shift();
  return await new Promise<any>((resolve,reject)=>{
    const timer=setTimeout(()=>{waiting=undefined;reject(new Error("Native sharing driver timed out: "+failure));},120_000);
    waiting=value=>{clearTimeout(timer);resolve(value);};
  });
};
const control=async(value:object)=>{child.stdin.write(JSON.stringify(value)+"\n");const result=await next();assert(!result.error,"Native operation failed");return result;};
const edit=(batch:Batch)=>control({method:"edit",batch});
async function until(check:()=>Promise<boolean>,label:string,timeout=45_000){
  const deadline=Date.now()+timeout;
  while(!await check()){assert(Date.now()<deadline,label);await new Promise(resolve=>setTimeout(resolve,40));}
}
async function openProfile(name:string){
  const profile=join(root,name);
  await mkdir(join(profile,"Default"),{recursive:true});
  await writeFile(join(profile,"Default/Preferences"),JSON.stringify({profile:{block_third_party_cookies:true,cookie_controls_mode:1}}));
  return chromium.launchPersistentContext(profile,{channel:"chrome",headless:true,acceptDownloads:true});
}
async function restrict(page:Page,context:BrowserContext){
  const cdp=await context.newCDPSession(page);await cdp.send("Network.enable");
  await cdp.send("Network.setCookieControls",{enableThirdPartyCookieRestriction:true});
}
let contexts:BrowserContext[]=[];
let stopped=false;
try {
  const lookup="https://share.hitslop.com/v1/account/rooms/00000000-0000-4000-8000-000000000000";
  const unknownKey=[{alg:"RS256",kid:"hitslop-deliberately-unknown-test-key"},{sub:"test-user"}].map(part=>Buffer.from(JSON.stringify(part)).toString("base64url")).join(".")+"."+Buffer.alloc(256).toString("base64url");
  for(const token of [undefined,"malformed",unknownKey]) {
    const response=await fetch(lookup,{headers:token?{Authorization:"Bearer "+token}:{}});
    assert.equal(response.status,401,"Account lookup must refuse absent or untrusted identities");
    assert.equal((await response.json() as {code:string}).code,"sign_in_required");
  }
  note("The account lookup refuses anonymous, malformed and unknown-key identities; valid native identity remains a separate qualification");
  const ready=await next();assert(ready.ready,"Native client failed to publish");
  note(invitationFile ? "A signed-out native guest joins the disposable owner-app invitation without a Firebase session" : "A verified account creates and publishes the disposable native fixture");
  const invitation=(await readFile(join(root,"invitation.txt"),"utf8")).trim();
  const first=await openProfile("chrome-a");contexts.push(first);
  const page=await first.newPage();await restrict(page,first);
  await page.goto(invitation);
  const app=page.frameLocator("iframe");
  await app.getByRole("button",{name:"Add row",exact:true}).waitFor({timeout:45_000});
  const frame=page.frames().find(frame=>frame.url().includes("."+SharingHost.domain))!;
  const copy=new URL(frame.url()).hostname.split(".")[0]!;
  assert(!new URL(page.url()).hash,"Invitation fragment must be removed");
  assert.equal(await frame.evaluate(()=>document.cookie),"","Read cookie must be HttpOnly");
  assert(await app.getByAltText("Private app image").evaluate(async img=>{await (img as HTMLImageElement).decode();return (img as HTMLImageElement).naturalWidth>0;}),"Private app image did not decode");
  assert(await frame.evaluate(async()=>{await document.fonts.ready;return document.fonts.check("16px Fixture");}),"Private app font did not load");
  const denied=await fetch(frame.url()+"assets/ui.js");assert.equal(denied.status,403);
  note("Private app, CSS, font and PNG boot under third-party cookie restriction; anonymous asset reads fail");
  const add=async(text:string)=>{await app.getByRole("textbox",{name:"New row",exact:true}).fill(text);await app.getByRole("button",{name:"Add row",exact:true}).click();};
  const label="browser-"+Date.now();await add(label);
  await until(async()=>(await control({method:"state"})).state.value.rows.some((row:any)=>row.text===label),"Browser edit did not reach the native owner");
  const native="native-"+Date.now();await edit({intents:[{type:"insert",path:["rows"],value:{text:native}}]});
  await app.getByText(native,{exact:true}).waitFor();
  note("Browser and native edits converge through their existing Rust owners");
  // Pausing the native transport leaves the real owner and local save path running.
  await control({method:"pause"});
  const offlineNative="native-offline-"+Date.now();
  await edit({intents:[{type:"insert",path:["rows"],value:{text:offlineNative}}]});
  assert.equal(await app.getByText(offlineNative,{exact:true}).count(),0,"Paused native edit must remain local");
  await control({method:"resume"});
  await app.getByText(offlineNative,{exact:true}).waitFor({timeout:45_000});
  // An already-open signed-out browser also keeps editing through a real disconnect.
  await first.setOffline(true);
  const offlineBrowser="browser-offline-"+Date.now();
  await add(offlineBrowser);
  await app.getByText(offlineBrowser,{exact:true}).waitFor();
  assert(!(await control({method:"state"})).state.value.rows.some((row:any)=>row.text===offlineBrowser),"Offline guest edit must remain local");
  await first.setOffline(false);
  await until(async()=>(await control({method:"state"})).state.value.rows.some((row:any)=>row.text===offlineBrowser),"Offline browser edit did not catch up");
  note("Native pause/edit/reconnect and signed-out browser offline edits converge without replacing local state");
  const second=await openProfile("chrome-b");contexts.push(second);const late=await second.newPage();await restrict(late,second);await late.goto(invitation);
  await late.frameLocator("iframe").getByText(native,{exact:true}).waitFor({timeout:45_000});
  const otherFrame=late.frames().find(frame=>frame.url().includes("."+SharingHost.domain))!;
  const cookie=await first.cookies(frame.url());const resourceCookie=cookie.find(cookie=>cookie.name==="__Host-slop-resource");assert(resourceCookie);
  assert.equal((await fetch(otherFrame.url()+"assets/ui.js",{headers:{Cookie:`${resourceCookie.name}=${resourceCookie.value}`}})).status,403);
  note("A late browser joins the canonical document; a sibling host cannot reuse its resource cookie");
  await page.reload();
  await page.frameLocator("iframe").getByText(offlineBrowser,{exact:true}).waitFor({timeout:45_000});
  note("Connected browser refresh retains accepted guest edits and catches up");
  // Hold all blob reads after the first failure. CaughtUp queued behind failed Records
  // must never publish a reference to a file which the owner has not installed.
  let blocked=false,release:()=>void=()=>{};const gate=new Promise<void>(resolve=>{release=resolve;});
  await first.route("**/rooms/*/blob/*",async route=>{
    if(route.request().method()!=="GET"){await route.continue();return;}
    if(!blocked){blocked=true;await route.abort("failed");return;}
    await gate;await route.continue();
  });
  const photo=join(repository,"tests/abi/owner-svelte/media/swatch.png");
  const attached=await control({method:"attach",path:photo});
  const name="delivered-photo.png";
  await edit({intents:[{type:"insert",path:["files"],value:{hash:attached.attachment,name,mimeType:"image/png",bytes:Bun.file(photo).size}}]});
  try {
    await until(async()=>blocked,"Incoming file read was not exercised");
    await until(async()=>(await page.locator("#sync").textContent())!=="Synced","Failed receive incorrectly reported Synced");
    assert.equal(await app.getByAltText(name,{exact:true}).count(),0,"Failed receive overtook the file barrier");
  } finally {release();}
  await app.getByAltText(name,{exact:true}).waitFor({timeout:45_000});
  await until(async()=>app.getByAltText(name,{exact:true}).evaluate(img=>(img as HTMLImageElement).naturalWidth>0),"Delivered photo could not render");
  await first.unroute("**/rooms/*/blob/*");
  note("An interrupted receive fences queued CaughtUp; reconnect installs the verified image before publishing its reference");
  await page.evaluate(async()=>{
    const root=await navigator.storage.getDirectory();const file=await root.getFileHandle("sharing-download.slop",{create:true});
    (window as any).__download=file;(window as any).showSaveFilePicker=async()=>file;
  });
  await page.getByRole("button",{name:"Download .slop",exact:true}).click();
  await until(async()=>page.getByRole("button",{name:"Download .slop",exact:true}).isEnabled(),"Download did not finish");
  const bytes=await page.evaluate(async()=>Array.from(new Uint8Array(await (await (window as any).__download.getFile()).arrayBuffer())));
  const download=join(root,"browser-download.slop");await writeFile(download,Buffer.from(bytes));
  const db=new Database(download,{readonly:true});
  try{assert.equal((db.query("SELECT uuid FROM document").get() as {uuid:string}).uuid,ready.document);}finally{db.close();}
  const opened=await execute({method:"get",documentPath:download});assert(opened.ok);
  const inventory=await execute({method:"attachments.list",documentPath:download});assert(inventory.ok&&inventory.state.some(item=>item.id===attached.attachment));
  note("A browser download preserves canonical UUID, edits and the new attachment and opens in the native engine");
  const reopen=page.url();await first.close();contexts=contexts.filter(context=>context!==first);
  const restored=await chromium.launchPersistentContext(join(root,"chrome-a"),{channel:"chrome",headless:true});contexts.push(restored);
  let resumedBlobPuts=0;
  restored.on("request",request=>{if(request.method()==="PUT"&&/\/rooms\/[^/]+\/blob\//.test(new URL(request.url()).pathname))resumedBlobPuts++;});
  const reopened=await restored.newPage();await restrict(reopened,restored);await reopened.goto(reopen);
  await reopened.frameLocator("iframe").getByText(label,{exact:true}).waitFor({timeout:45_000});
  note("Connected process reopen retains the browser copy and its edits");
  const resumed="resumed-"+Date.now();
  const resumedApp=reopened.frameLocator("iframe");
  await resumedApp.getByRole("textbox",{name:"New row",exact:true}).fill(resumed);
  await resumedApp.getByRole("button",{name:"Add row",exact:true}).click();
  await until(async()=>(await control({method:"state"})).state.value.rows.some((row:any)=>row.text===resumed),"Reopened browser edit did not reach native owner");
  await until(async()=>(await reopened.locator("#sync").textContent())==="Synced","Reopened browser did not finish syncing");
  assert.equal(resumedBlobPuts,0,"Reopened browser re-uploaded an existing attachment");
  note("Process reopen and a new edit reuse remote attachment inventory without blob PUTs");
  if(invitationFile){
    note("Ready for Stop: stop this disposable room in its signed-in native owner app");
    await until(async()=>(await reopened.locator("#sync").textContent())==="Sharing ended","Native owner has not stopped the disposable room",300_000);
  }else await control({method:"stop"});
  stopped=true;
  await until(async()=>(await reopened.locator("#sync").textContent())==="Sharing ended","Room stop did not reach the browser");
  assert.equal((await fetch(frame.url()+"assets/ui.js",{headers:{Cookie:`${resourceCookie.name}=${resourceCookie.value}`}})).status,410);
  let stoppedSessions=0,stoppedSockets=0;
  reopened.on("request",request=>{if(new URL(request.url()).pathname.startsWith("/_resources/"))stoppedSessions++;});
  reopened.on("websocket",()=>{stoppedSockets++;});
  await reopened.reload();
  await reopened.frameLocator("iframe").getByText(resumed,{exact:true}).waitFor({timeout:45_000});
  assert.equal(await reopened.locator("#sync").textContent(),"Sharing ended");
  await reopened.frameLocator("iframe").getByRole("textbox",{name:"New row",exact:true}).fill("Local after stop");
  await reopened.frameLocator("iframe").getByRole("button",{name:"Add row",exact:true}).click();
  await reopened.frameLocator("iframe").getByText("Local after stop",{exact:true}).waitFor();
  await reopened.evaluate(async()=>{
    const storage=await navigator.storage.getDirectory();const file=await storage.getFileHandle("ended-download.slop",{create:true});
    (window as any).__download=file;(window as any).showSaveFilePicker=async()=>file;
  });
  await reopened.getByRole("button",{name:"Download .slop",exact:true}).click();
  await until(async()=>reopened.getByRole("button",{name:"Download .slop",exact:true}).isEnabled(),"Ended copy download did not finish");
  const endedBytes=await reopened.evaluate(async()=>Array.from(new Uint8Array(await (await (window as any).__download.getFile()).arrayBuffer())));
  const endedDownload=join(root,"ended-download.slop");await writeFile(endedDownload,Buffer.from(endedBytes));
  const endedCopy=await execute({method:"get",documentPath:endedDownload});assert(endedCopy.ok);
  assert((endedCopy.state.value as {rows:{text:string}[]}).rows.some(row=>row.text==="Local after stop"));
  assert.equal(stoppedSessions,0);assert.equal(stoppedSockets,0);
  note("Stopping revokes private reads and sync; a reloaded local copy stays editable and downloadable without remote sessions");
  const refusedContext=await openProfile("chrome-ended");contexts.push(refusedContext);
  const refused=await refusedContext.newPage();await refused.goto(invitation);
  await refused.locator('#status[role="alert"]').waitFor();
  assert((await refused.locator("#status").textContent())?.includes("410"));
  assert(await refused.getByRole("button",{name:"Download .slop",exact:true}).isDisabled());
  assert.equal(await refused.locator("iframe").count(),0);
  note("A fresh browser still refuses an ended invitation");
  await writeFile(report,JSON.stringify({passed:true,checks,document:ready.document,copy,chrome:restored.browser()?.version()},null,2));
  console.log("Sharing checks passed; report: "+report);
} catch(error) {
  const message=String(error).replace(/https:\/\/share\.hitslop\.com\/join#[a-f0-9]+\.[a-f0-9]+/g,"[private invitation]");
  await writeFile(report,JSON.stringify({passed:false,checks,error:message},null,2));
  console.error(message);process.exitCode=1;
} finally {
  for(const context of contexts)await context.close().catch(()=>{});
  if(!stopped&&!invitationFile&&child.exitCode===null)await control({method:"stop"}).catch(()=>{});
  if(child.exitCode===null)await control({method:"quit"}).catch(()=>{});
  child.stdin.end();child.kill();
}

import { checkContainer } from "./copies";
import { BrowserHost } from "../schema/constants";
import type { BrowserRequest, BrowserEvent } from "../wire/browser.generated";

// WASM is loaded only on the trusted storage origin, inside this dedicated worker.
const runtimeURL = new URL("./core/hitslop_core_wasm.js", location.href).href;
const initialized = (async () => {
  const runtime = await import(/* @vite-ignore */ runtimeURL);
  await runtime.default();
  return runtime;
})();
let copy: any;
let opened = false;
let copyID = "";
let timer: ReturnType<typeof setTimeout> | undefined;
const send = (event: BrowserEvent, bytes?: Uint8Array) =>
  postMessage(bytes ? { ...event, bytes } : event, bytes ? [bytes.buffer] : []);
function poll(bytes?: Uint8Array) {
  clearTimeout(timer);
  const delay = copy.poll(performance.now(), Date.now());
  for (const event of JSON.parse(copy.events()) as BrowserEvent[]) send(event, event.type === "resource" ? bytes : undefined);
  if (delay >= 0) timer = setTimeout(poll, Math.max(1, delay));
}
async function open(request: Extract<BrowserRequest, { type: "open" }>) {
  if (copy) throw new Error("Worker already owns a copy");
  if (!("createSyncAccessHandle" in FileSystemFileHandle.prototype)) throw new Error("This browser cannot save local copies");
  copyID = request.copy;
  const runtime = await initialized;
  await checkContainer(copyID, !!request.source);
  try { copy = await runtime.installCopy(copyID); }
  catch (error) { throw `Browser storage is busy: ${String(error)}`; }
  if (request.source) {
    const url = `/__import/${encodeURIComponent(request.source)}`;
    const head = await fetch(url, { method: "HEAD" });
    if (!head.ok) throw new Error("Import expired. Run slop open --browser again.");
    const size = Number(head.headers.get("Content-Length"));
    if (!Number.isSafeInteger(size) || size > BrowserHost.fileBytes || size < 100) throw new Error("Invalid import size");
    copy.importBegin(size);
    for (let offset = 0; offset < size; offset += BrowserHost.chunkBytes) {
      const end = Math.min(size, offset + BrowserHost.chunkBytes);
      const response = await fetch(url, { headers: { Range: `bytes=${offset}-${end - 1}` } });
      if (response.status !== 206 || response.headers.get("Content-Range") !== `bytes ${offset}-${end - 1}/${size}`)
        throw new Error("Snapshot transfer was interrupted");
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.length !== end - offset) throw new Error("Incomplete snapshot transfer");
      copy.importWrite(bytes);
    }
    copy.importFinish();
  }
  copy.open();
  opened = true;
  poll();
  if (request.source) await fetch(`/__import/${encodeURIComponent(request.source)}`, { method: "DELETE" });
}
async function exportFile(id: string) {
  const root = await navigator.storage.getDirectory();
  const exports = await root.getDirectoryHandle("exports", { create: true });
  const folder = await exports.getDirectoryHandle(copyID, { create: true });
  const name = `${crypto.randomUUID()}.slop`;
  const file = await folder.getFileHandle(name, { create: true });
  const handle = await (file as any).createSyncAccessHandle();
  try {
    handle.truncate(0);
    let at = 0;
    copy.export((bytes: Uint8Array) => {
      const written = handle.write(bytes, { at });
      if (written !== bytes.byteLength) throw new Error("Incomplete export write");
      at += written;
    });
    handle.flush();
  } catch (error) {
    handle.close();
    await folder.removeEntry(name).catch(() => {});
    throw error;
  }
  handle.close();
  send({ type: "exported", id, file: name });
}
// Serial async control operations; authored page work never interleaves an import/export.
let queue = Promise.resolve();
onmessage = ({ data: input }: MessageEvent<unknown>) => {
  queue = queue.then(async () => {
    let data: BrowserRequest | undefined;
    let canPoll = false;
    let bytes: Uint8Array | undefined;
    try {
      const runtime = await initialized;
      data = JSON.parse(runtime.decodeBrowserRequest(JSON.stringify(input))) as BrowserRequest;
      clearTimeout(timer);
      if (copy && data.type !== "open") await copy.recoverStorage();
      canPoll = opened;
      if (data.type === "open") { await open(data); return; }
      if (data.type === "export") await exportFile(data.id);
      else bytes = copy.request(JSON.stringify(data)) ?? undefined;
    } catch (error) {
      const id = data && "id" in data ? data.id : input && typeof input === "object" && "id" in input && typeof input.id === "string" && input.id.length <= 128 ? input.id : null;
      send({ type: "error", id, error: String(error) });
    }
    if (canPoll) poll(bytes);
  });
};

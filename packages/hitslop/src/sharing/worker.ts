import { config, frameOrigin } from "./config";
import { checkContainer } from "../browser/copies";
import { BrowserHost } from "../schema/constants";
import type { BrowserRequest, BrowserEvent } from "../wire/browser.generated";
import type {
  ConnectionPlan,
  Resource,
  ResourceGrant,
  SharingControl,
  SharingEvent,
} from "../wire/sharing.generated";
import { SharingHost } from "../wire/constants.generated";

/** The part of the WASM module this worker calls. wasm-bindgen declares the module in
 * `generated/browser/core/hitslop_core_wasm.d.ts`, but that is a build output, absent wherever
 * types are checked without building WASM, so the surface is restated here. The browser tests
 * run the real module through this worker. */
interface BrowserCopy {
  events(): string;
  openJoined(invitation: string, origin: string): void;
  syncAttach(): void;
  syncDetach(): void;
  syncFrame(bytes: Uint8Array): void;
  nextSyncFrame(): Uint8Array | undefined;
  syncStatus(): string;
  resourceRange(attachment: boolean, key: string, offset: number, length: number): Uint8Array;
  verifyResources(json: string): string;
  attachmentInventory(): string;
  missingAttachments(json: string): string;
  installAttachment(item: string, bytes: Uint8Array, monotonic: number, unix: number): void;
  export(write: (bytes: Uint8Array) => void): void;
  importBegin(size: number): void;
  importFinish(): void;
  importWrite(bytes: Uint8Array): void;
  open(): void;
  /** A negative delay means there is no pending timer. */
  poll(monotonicMs: number, unixMs: number): number;
  recoverStorage(): Promise<void>;
  request(json: string): Uint8Array | undefined;
}
interface BrowserRuntime {
  default(): Promise<unknown>;
  decodeBrowserRequest(json: string): string;
  installCopy(copy: string): Promise<BrowserCopy>;
  invitationRequest(json: string, origin: string): string;
  socketBudget(): number;
  frameOperation(bytes: Uint8Array): string;
}

// WASM is loaded only on the trusted storage origin, inside this dedicated worker.
const runtimeURL = new URL("./core/hitslop_core_wasm.js", location.href).href;
const initialized = (async () => {
  const runtime = (await import(/* @vite-ignore */ runtimeURL)) as BrowserRuntime;
  await runtime.default();
  return runtime;
})();
let copy: BrowserCopy | undefined;
let invitation = "";
let socket: WebSocket | undefined;
let generation = 0,
  backoff = 250;
let lastStatus = "";
let sending = 0;
let connection: ConnectionPlan;
let networkAbort: AbortController | undefined;
let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
let ended = false;
let resourcesReady = false;
const uploaded = new Set<string>();
const sharingEvent = (event: SharingEvent) => postMessage(event);
class TransportError extends Error {
  constructor(readonly status: number) {
    super("File transport refused: " + status);
  }
}
function currentEpoch(epoch: number) {
  if (epoch !== generation) throw new DOMException("Connection replaced", "AbortError");
}
async function checkedFetch(url: string, init: RequestInit = {}, epoch = generation) {
  currentEpoch(epoch);
  const signal = networkAbort?.signal;
  const response = await fetch(url, {
    ...init,
    signal: signal
      ? AbortSignal.any([signal, AbortSignal.timeout(30_000)])
      : AbortSignal.timeout(30_000),
  });
  currentEpoch(epoch);
  if (!response.ok) throw new TransportError(response.status);
  return response;
}
async function boundedBody(response: Response, limit: number, epoch: number) {
  if (!response.body) throw new Error("Missing response body");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      currentEpoch(epoch);
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > limit) throw new Error("File exceeds its declared size");
      chunks.push(value);
    }
  } catch (error) {
    await reader.cancel().catch(() => {});
    throw error;
  }
  currentEpoch(epoch);
  const bytes = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.length;
  }
  return bytes;
}
async function metadata(response: Response, epoch: number) {
  return new TextDecoder().decode(await boundedBody(response, SharingHost.controlBytes, epoch));
}
async function resourceSession(epoch: number) {
  const manifest = await checkedFetch(
    connection.url.replace(/seed$/, "resources"),
    { headers: connection.headers },
    epoch,
  );
  const app = copy!.verifyResources(await metadata(manifest, epoch));
  const grant: ResourceGrant = { app };
  await checkedFetch(
    frameOrigin(copyID) + "/_resources/" + new URL(connection.url).pathname.split("/")[2],
    {
      method: "POST",
      credentials: "include",
      headers: { ...connection.headers, "Content-Type": "application/json" },
      body: JSON.stringify(grant),
    },
    epoch,
  );
  if (!resourcesReady) {
    resourcesReady = true;
    sharingEvent({ type: "resourcesReady" });
  }
}
async function uploadFiles(epoch: number) {
  sharingEvent({ type: "sync", status: "Syncing files" });
  sharingEvent({ type: "files", status: "Uploading files" });
  for (const item of JSON.parse(copy!.attachmentInventory()) as Resource[]) {
    currentEpoch(epoch);
    if (uploaded.has(item.hash)) continue;
    const chunks: Uint8Array[] = [];
    for (let at = 0; at < item.size; at += BrowserHost.chunkBytes)
      chunks.push(
        copy!.resourceRange(true, item.hash, at, Math.min(BrowserHost.chunkBytes, item.size - at)),
      );
    await checkedFetch(
      connection.url.replace(/seed$/, "blob/" + item.hash),
      {
        method: "PUT",
        headers: { ...connection.headers, "Content-Type": item.media_type },
        body: new Blob(chunks.map((chunk) => new Uint8Array(chunk).buffer)),
      },
      epoch,
    );
    currentEpoch(epoch);
    uploaded.add(item.hash);
  }
}
async function receiveFiles(epoch: number) {
  sharingEvent({ type: "sync", status: "Receiving files" });
  sharingEvent({ type: "files", status: "Receiving files" });
  const list = await checkedFetch(
    connection.url.replace(/seed$/, "blobs"),
    { headers: connection.headers },
    epoch,
  );
  for (const item of JSON.parse(
    copy!.missingAttachments(await metadata(list, epoch)),
  ) as Resource[]) {
    const response = await checkedFetch(
      connection.url.replace(/seed$/, "blob/" + item.hash),
      { headers: connection.headers },
      epoch,
    );
    const bytes = await boundedBody(response, item.size, epoch);
    if (bytes.length !== item.size) throw new Error("Incomplete attachment");
    currentEpoch(epoch);
    copy!.installAttachment(JSON.stringify(item), bytes, performance.now(), Date.now());
    uploaded.add(item.hash);
  }
  sharingEvent({ type: "files", status: "Files ready" });
}
function resourceFailure(error: unknown, epoch = generation) {
  if (epoch !== generation) return;
  sharingEvent({ type: "files", status: "File transfer pending: " + String(error) });
  // Fence the generation before QueuedCaughtUp can follow a failed file download.
  ++generation;
  networkAbort?.abort();
  const target = socket;
  socket = undefined;
  target?.close(4011, "File transfer interrupted");
  copy?.syncDetach();
  if (opened) poll();
  if (error instanceof TransportError && error.status === 410) {
    ended = true;
    sharingEvent({ type: "sync", status: "Sharing ended" });
    return;
  }
  sharingEvent({ type: "sync", status: "Offline; reconnecting" });
  clearTimeout(reconnectTimer);
  if (!ended)
    reconnectTimer = setTimeout(() => {
      void connectSocket().catch((error) => resourceFailure(error));
    }, backoff);
  backoff = Math.min(8000, backoff * 2);
}
async function connectSocket() {
  if (ended) return;
  networkAbort?.abort();
  networkAbort = new AbortController();
  const epoch = ++generation;
  await resourceSession(epoch);
  currentEpoch(epoch);
  const ws = new WebSocket(connection.socket, connection.protocols);
  socket = ws;
  ws.binaryType = "arraybuffer";
  // Each connection owns its queue and counters. An abandoned HTTP read never blocks
  // the next connection or decrements its budget.
  let incoming = Promise.resolve(),
    incomingBytes = 0,
    incomingFrames = 0;
  ws.onopen = () => {
    if (epoch !== generation) {
      ws.close();
      return;
    }
    backoff = 250;
    copy!.syncAttach();
    poll();
  };
  ws.onmessage = ({ data }) => {
    if (epoch !== generation) return;
    if (!(data instanceof ArrayBuffer)) {
      resourceFailure(new Error("Binary frame required"), epoch);
      return;
    }
    incomingBytes += data.byteLength;
    incomingFrames++;
    if (incomingBytes > SharingHost.queueBytes || incomingFrames > SharingHost.queueFrames) {
      resourceFailure(new Error("Incoming queue budget exceeded"), epoch);
      return;
    }
    incoming = incoming
      .then(async () => {
        try {
          currentEpoch(epoch);
          const runtime = await initialized;
          currentEpoch(epoch);
          const bytes = new Uint8Array(data);
          const operation = runtime.frameOperation(bytes);
          if (operation === "ended") {
            resourceFailure(new TransportError(410), epoch);
            return;
          }
          if (operation === "download") await receiveFiles(epoch);
          currentEpoch(epoch);
          copy!.syncFrame(bytes);
          poll();
        } finally {
          incomingBytes -= data.byteLength;
          incomingFrames--;
        }
      })
      .catch((error) => resourceFailure(error, epoch));
  };
  ws.onerror = () => {};
  ws.onclose = () => {
    if (epoch !== generation || ended) return;
    resourceFailure(new Error("Connection interrupted"), epoch);
  };
}
function flushSocket() {
  if (!copy || socket?.readyState !== WebSocket.OPEN || sending === generation) return;
  const state = copy.syncStatus();
  if (state.startsWith("Paused")) {
    ended = true;
    socket.close(1000, "Sharing paused");
  }
  if (state !== lastStatus) {
    lastStatus = state;
    sharingEvent({ type: "sync", status: state });
  }
  let frame: Uint8Array | undefined;
  try {
    frame = copy.nextSyncFrame();
  } catch (error) {
    resourceFailure(error);
    return;
  }
  if (!frame) return;
  const bytes = frame,
    epoch = generation,
    target = socket;
  sending = epoch;
  void (async () => {
    const runtime = await initialized;
    currentEpoch(epoch);
    if (runtime.frameOperation(bytes) === "upload") await uploadFiles(epoch);
    currentEpoch(epoch);
    if (target.readyState !== WebSocket.OPEN) return;
    if (target.bufferedAmount + bytes.length > SharingHost.queueBytes)
      throw new Error("Outgoing queue budget exceeded");
    target.send(new Uint8Array(bytes));
    sharingEvent({ type: "files", status: "Files ready" });
  })()
    .catch((error) => resourceFailure(error, epoch))
    .finally(() => {
      if (sending === epoch) sending = 0;
      flushSocket();
    });
}
let opened = false;
let copyID = "";
let timer: ReturnType<typeof setTimeout> | undefined;
const send = (event: BrowserEvent, bytes?: Uint8Array) =>
  postMessage(bytes ? { ...event, bytes } : event, bytes ? [bytes.buffer] : []);
function poll(bytes?: Uint8Array) {
  clearTimeout(timer);
  if (!copy) return;
  const delay = copy.poll(performance.now(), Date.now());
  for (const event of JSON.parse(copy.events()) as BrowserEvent[])
    send(event, event.type === "resource" ? bytes : undefined);
  flushSocket();
  if (delay >= 0) timer = setTimeout(poll, Math.max(1, delay));
}
async function open(request: Extract<BrowserRequest, { type: "open" }>) {
  if (copy) throw new Error("Worker already owns a copy");
  if (!("createSyncAccessHandle" in FileSystemFileHandle.prototype))
    throw new Error("This browser cannot save local copies");
  copyID = request.copy;
  const runtime = await initialized;
  await checkContainer(copyID, !!request.source);
  let installed: BrowserCopy;
  try {
    installed = await runtime.installCopy(copyID);
  } catch (error) {
    throw `Browser storage is busy: ${String(error)}`;
  }
  copy = installed;
  connection = JSON.parse(runtime.invitationRequest(invitation, config.relay));
  if (request.source) {
    const response = await checkedFetch(connection.url, {
      headers: connection.headers,
      credentials: "omit",
    });
    if (!response.ok || !response.body)
      throw new Error("Seed download refused: " + response.status);
    const size = connection.size;
    if (size > BrowserHost.fileBytes) throw new Error("Seed too large");
    const estimate = await navigator.storage.estimate();
    if (estimate.quota !== undefined && estimate.quota - (estimate.usage ?? 0) < size * 2)
      throw new Error("Not enough browser storage");
    installed.importBegin(size);
    const reader = response.body.getReader();
    let received = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value.length;
      if (received > size) {
        await reader.cancel();
        throw new Error("Seed exceeds declared size");
      }
      for (let offset = 0; offset < value.length; offset += BrowserHost.chunkBytes)
        installed.importWrite(value.subarray(offset, offset + BrowserHost.chunkBytes));
    }
    if (received !== size) throw new Error("Incomplete seed");
    installed.importFinish();
  }
  installed.openJoined(invitation, config.relay);
  opened = true;
  poll();
  setInterval(() => {
    const epoch = generation;
    if (!ended) void resourceSession(epoch).catch((error) => resourceFailure(error, epoch));
  }, SharingHost.resourceRenewMs);
  void connectSocket().catch((error) => resourceFailure(error));
}
async function exportFile(id: string) {
  if (!copy) throw new Error("Copy is not open");
  const root = await navigator.storage.getDirectory();
  const exports = await root.getDirectoryHandle("exports", { create: true });
  const folder = await exports.getDirectoryHandle(copyID, { create: true });
  const name = `${crypto.randomUUID()}.slop`;
  const file = await folder.getFileHandle(name, { create: true });
  const handle = await (file as any).createSyncAccessHandle();
  try {
    handle.truncate(0);
    let at = 0;
    copy.export((bytes) => {
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
/** What escapes a request's own handling (a poll that throws, an event that cannot be posted)
 * leaves the owner in an unknown state. Raised as an uncaught error it reaches the host as
 * `Worker.onerror`, which fails every waiting request; swallowed by the queue it would leave
 * every later message unanswered, with no signal at all. */
const crash = (error: unknown) =>
  queueMicrotask(() => {
    throw error;
  });
// Serial async control operations; authored page work never interleaves an import/export.
let queue = Promise.resolve();
onmessage = ({ data: input }: MessageEvent<unknown>) => {
  queue = queue.then(async () => {
    let data: BrowserRequest | undefined;
    let canPoll = false;
    let bytes: Uint8Array | undefined;
    try {
      const runtime = await initialized;
      const control = input as Partial<SharingControl> | null;
      if (control?.type === "joinCredential") {
        const candidate = control.credential;
        if (copy || typeof candidate !== "string" || candidate.length > 4096)
          throw new Error("Invalid sharing link");
        runtime.invitationRequest(candidate, config.relay);
        invitation = candidate;
        return;
      }
      data = JSON.parse(runtime.decodeBrowserRequest(JSON.stringify(input))) as BrowserRequest;
      clearTimeout(timer);
      if (copy && data.type !== "open") await copy.recoverStorage();
      canPoll = opened;
      if (data.type === "open") {
        await open(data);
        return;
      }
      if (data.type === "export") await exportFile(data.id);
      else {
        if (!copy) throw new Error("Copy is not open");
        bytes = copy.request(JSON.stringify(data)) ?? undefined;
      }
    } catch (error) {
      const id =
        data && "id" in data
          ? data.id
          : input &&
              typeof input === "object" &&
              "id" in input &&
              typeof input.id === "string" &&
              input.id.length <= 128
            ? input.id
            : null;
      send({ type: "error", id, error: String(error) });
    }
    if (canPoll) poll(bytes);
  });
  queue = queue.catch(crash);
};

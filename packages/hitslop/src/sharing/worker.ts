import { config, frameOrigin } from "./config";
import {
  WorkerOwner,
  type BrowserCopy as LocalCopy,
  type BrowserRuntime as LocalRuntime,
} from "../browser/worker-owner";
import { BrowserHost } from "../schema/constants";
import type {
  ConnectionPlan,
  Resource,
  ResourceGrant,
  SharingControl,
  SharingEvent,
} from "../wire/sharing.generated";
import { SharingHost } from "../wire/constants.generated";

interface BrowserCopy extends LocalCopy {
  openJoined(invitation: string, origin: string, info: string): void;
  openSavedJoined(invitation: string, origin: string): void;
  syncAttach(): void;
  syncDetach(): void;
  syncFrame(bytes: Uint8Array): void;
  nextSyncFrame(): Uint8Array | undefined;
  syncStatus(): string;
  resourceRange(attachment: boolean, key: string, offset: number, length: number): Uint8Array;
  verifyResources(json: string): string;
  attachmentInventory(): string;
  missingAttachments(json: string): string;
  missingUploads(json: string): string;
  installAttachment(item: string, bytes: Uint8Array, monotonic: number, unix: number): void;
}
interface BrowserRuntime extends LocalRuntime<BrowserCopy> {
  invitationRequest(link: string, origin: string, info: string): string;
  frameOperation(bytes: Uint8Array): string;
}
let invitation = "";
let socket: WebSocket | undefined;
let generation = 0,
  backoff = 250;
let lastStatus = "";
let sending = 0;
let receiving = 0;
let connection: ConnectionPlan;
let networkAbort: AbortController | undefined;
let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
let ended = false;
let resourcesReady = false;
const remoteAttachments = new Map<string, Resource>();
const sharingEvent = (event: SharingEvent) => postMessage(event);
function reportSync(status: string) {
  if (status === lastStatus) return;
  lastStatus = status;
  sharingEvent({ type: "sync", status });
}
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
  const app = owner.copy!.verifyResources(await metadata(manifest, epoch));
  const grant: ResourceGrant = { app };
  await checkedFetch(
    frameOrigin(owner.copyID) + "/_resources/" + new URL(connection.url).pathname.split("/").at(-2),
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
  reportSync("Syncing files");
  sharingEvent({ type: "files", status: "Uploading files" });
  for (const item of JSON.parse(
    owner.copy!.missingUploads(JSON.stringify([...remoteAttachments.values()])),
  ) as Resource[]) {
    currentEpoch(epoch);
    const chunks: Uint8Array[] = [];
    for (let at = 0; at < item.size; at += BrowserHost.chunkBytes)
      chunks.push(
        owner.copy!.resourceRange(
          true,
          item.hash,
          at,
          Math.min(BrowserHost.chunkBytes, item.size - at),
        ),
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
    remoteAttachments.set(item.hash, item);
  }
}
async function receiveFiles(epoch: number) {
  receiving = epoch;
  try {
    await receiveInventory(epoch);
  } finally {
    if (receiving === epoch) receiving = 0;
  }
}
async function receiveInventory(epoch: number) {
  reportSync("Receiving files");
  sharingEvent({ type: "files", status: "Receiving files" });
  const list = await checkedFetch(
    connection.url.replace(/seed$/, "blobs"),
    { headers: connection.headers },
    epoch,
  );
  const inventory = await metadata(list, epoch);
  const missing = JSON.parse(owner.copy!.missingAttachments(inventory)) as Resource[];
  // Rust validated the complete inventory, including metadata for existing hashes.
  for (const item of JSON.parse(inventory) as Resource[]) remoteAttachments.set(item.hash, item);
  for (const item of missing) {
    const response = await checkedFetch(
      connection.url.replace(/seed$/, "blob/" + item.hash),
      { headers: connection.headers },
      epoch,
    );
    const bytes = await boundedBody(response, item.size, epoch);
    if (bytes.length !== item.size) throw new Error("Incomplete attachment");
    currentEpoch(epoch);
    owner.copy!.installAttachment(JSON.stringify(item), bytes, performance.now(), Date.now());
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
  owner.copy?.syncDetach();
  if (owner.opened) owner.poll();
  if (error instanceof TransportError && error.status === 410) {
    ended = true;
    reportSync("Sharing ended");
    return;
  }
  reportSync("Offline; reconnecting");
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
  remoteAttachments.clear();
  await resourceSession(epoch);
  await receiveFiles(epoch);
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
    owner.copy!.syncAttach();
    owner.poll();
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
          const runtime = (await owner.initialized) as BrowserRuntime;
          currentEpoch(epoch);
          const bytes = new Uint8Array(data);
          const operation = runtime.frameOperation(bytes);
          if (operation === "ended") {
            resourceFailure(new TransportError(410), epoch);
            return;
          }
          if (operation === "download") await receiveFiles(epoch);
          currentEpoch(epoch);
          owner.copy!.syncFrame(bytes);
          owner.poll();
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
  if (!owner.copy || socket?.readyState !== WebSocket.OPEN || sending === generation) return;
  const state = owner.copy.syncStatus();
  if (state.startsWith("Paused")) {
    ended = true;
    socket.close(1000, "Sharing paused");
  }
  if (receiving !== generation) reportSync(state);
  let frame: Uint8Array | undefined;
  try {
    frame = owner.copy.nextSyncFrame();
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
    const runtime = (await owner.initialized) as BrowserRuntime;
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
let roomInfo = "";
const owner = new WorkerOwner<BrowserCopy>({
  async control(input) {
    const control = input as Partial<SharingControl> | null;
    if (control?.type !== "joinCredential") return false;
    const candidate = control.credential;
    if (
      owner.copy ||
      typeof candidate !== "string" ||
      !candidate.startsWith(`${config.host}/join#`) ||
      !/^[0-9a-f]{32}\.[0-9a-f]{64}$/.test(candidate.slice(`${config.host}/join#`.length))
    )
      throw new Error("Sharing link is no longer valid");
    invitation = candidate;
    const [room, capability] = candidate.split("#")[1]!.split(".");
    let response: Response;
    try {
      response = await checkedFetch(`${config.relay}/v1/rooms/${room}`, {
        headers: { Authorization: `Bearer ${capability}` },
        credentials: "omit",
      });
    } catch (error) {
      if (!(error instanceof TransportError) || error.status !== 410) throw error;
      ended = true;
      return true;
    }
    roomInfo = await metadata(response, generation);
    const runtime = (await owner.initialized) as BrowserRuntime;
    connection = JSON.parse(runtime.invitationRequest(invitation, config.relay, roomInfo));
    return true;
  },
  async importCopy(installed, request) {
    if (request.source) {
      if (ended) throw new TransportError(410);
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
  },
  openCopy(installed) {
    if (ended) {
      installed.openSavedJoined(invitation, config.relay);
      reportSync("Sharing ended");
    } else installed.openJoined(invitation, config.relay, roomInfo);
  },
  afterPoll: flushSocket,
  afterOpen() {
    if (ended) {
      sharingEvent({ type: "resourcesReady" });
      return;
    }
    setInterval(() => {
      const epoch = generation;
      if (!ended) void resourceSession(epoch).catch((error) => resourceFailure(error, epoch));
    }, SharingHost.resourceRenewMs);
    void connectSocket().catch((error) => resourceFailure(error));
  },
});
onmessage = owner.handle;

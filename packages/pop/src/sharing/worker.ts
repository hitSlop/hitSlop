import { config, frameOrigin } from "./config";
import {
  WorkerOwner,
  type BrowserCopy as LocalCopy,
  type BrowserRuntime as LocalRuntime,
} from "../browser/worker-owner";
import {
  checkContainer,
  listCopies,
  lockName,
  putCopy,
  updateCopy,
  removeCopy,
  reserveRecovery,
} from "../browser/copies";
import { BrowserHost } from "../schema/constants";
import type {
  ConnectionPlan,
  Failure,
  FullState,
  MaintenanceAction,
  Step,
  Pause,
  RoomInfo,
  Resource,
  ResourceGrant,
  PublishEpoch,
  EpochState,
  SharingControl,
  SharingEvent,
  SyncPhase,
  SyncState,
} from "../wire/sharing.generated";
import { SharingHost } from "../wire/constants.generated";

interface BrowserCopy extends LocalCopy {
  openLocal(): void;
  leaveRoom(): void;
  supportsEpoch(): boolean;
  confirmEpoch(receipt: string): void;
  hasEpoch(epoch: string): boolean;
  prepareEpoch(candidate: BrowserCopy, expected: string): string;
  exportEpoch(write: (bytes: Uint8Array) => void): void;
  epochAttachment(id: string): Uint8Array;
  currentVersion(): string;
  documentIdentity(): string;
  confirmedVersion(): string | undefined;
  validateEpoch(metadata: string, epoch: string): void;
  adoptEpoch(candidate: BrowserCopy, recovery: BrowserCopy, confirmed: string): boolean;
  releaseStorage(): void;
  abortImport(): void;
  openJoined(invitation: string, origin: string, info: string): void;
  openSavedJoined(invitation: string, origin: string): void;
  syncAttach(): void;
  syncDetach(): void;
  syncFrame(bytes: Uint8Array): void;
  nextSyncFrame(): Uint8Array | undefined;
  syncState(): string | undefined;
  pauseSync(pause: string): void;
  syncBase(
    monotonic: bigint,
    unix: bigint,
  ): { bytes(): Uint8Array; frontier(): string; root(): string; free(): void } | undefined;
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
  classifyFailure(status: number, code: string | undefined, url: string): string;
  maintenanceAction(full: string, step: string, random: number): string;
}
let invitation = "";
let localRecovery = false;
let refreshing = false;
let ending = false;
let confirmedVersion = "";
let socket: WebSocket | undefined;
let generation = 0,
  backoff: number = SharingHost.backoffMinMs;
let lastStatus = "";
let sending = 0;
let receiving = 0;
let connection: ConnectionPlan;
let networkAbort: AbortController | undefined;
let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
let ended = false;
let resourcesReady = false;
let compacting = 0;
let compactAfter = 0;
let checkedRoomAt = 0;
let retriedFull = false;
/** Room-full waits for compaction, which this worker runs, so only the other pauses stop it. */
const stops = (pause: Pause | undefined) =>
  pause !== undefined &&
  pause.kind !== "roomFull" &&
  !(pause.kind === "filesFull" && owner.copy?.supportsEpoch());
function copyState(): SyncState | undefined {
  const state = owner.copy?.syncState();
  return state ? (JSON.parse(state) as SyncState) : undefined;
}
const phaseText: Record<SyncPhase, string> = {
  offline: "Offline",
  syncing: "Syncing",
  synced: "Synced",
  paused: "Paused",
  ended: "Sharing ended",
};
function reportState(state: SyncState | undefined) {
  if (!state) return;
  reportSync(
    state.pause?.reason ?? phaseText[state.phase],
    state.pause?.kind === "roomFull" ? "syncing" : state.phase,
  );
}
function pauseSync(pause: Pause) {
  owner.copy?.pauseSync(JSON.stringify(pause));
  owner.poll();
}
async function compactRoom(epoch: number, full: boolean) {
  if (compacting || Date.now() < compactAfter) return;
  compacting = epoch;
  const filesFull = copyState()?.pause?.kind === "filesFull" && owner.copy!.supportsEpoch();
  try {
    const info = async () =>
      JSON.parse(
        await metadata(
          await checkedFetch(
            connection.url.replace(/\/seed$/, ""),
            { headers: connection.headers },
            epoch,
          ),
          epoch,
        ),
      ) as RoomInfo;
    const runtime = (await owner.initialized) as BrowserRuntime;
    const decide = (step: Step, random = 0): MaintenanceAction => {
      const fullState: FullState = filesFull
        ? retriedFull
          ? "filesRetried"
          : "filesWaiting"
        : full
          ? retriedFull
            ? "retried"
            : "waiting"
          : "available";
      return JSON.parse(
        runtime.maintenanceAction(JSON.stringify(fullState), JSON.stringify(step), random),
      );
    };
    const first = await info();
    checkedRoomAt = Date.now();
    let action = decide(
      { step: "probe", compact: first.compact ?? false },
      crypto.getRandomValues(new Uint16Array(1))[0]!,
    );
    if (action.action === "outgrown") {
      pauseSync({ kind: "outgrown", reason: "This document has outgrown sharing" });
      return;
    }
    if (action.action !== "jitter") return;
    const delay = action.milliseconds;
    await new Promise((resolve) => setTimeout(resolve, delay));
    currentEpoch(epoch);
    const latest = await info();
    action = decide({ step: "recheck", compact: latest.compact ?? false });
    if (action.action === "reconnect") {
      retriedFull = true;
      reconnectAfterCompaction(epoch);
      return;
    }
    if (action.action !== "readBase") return;
    if (filesFull) {
      const canonical = JSON.parse(
        await metadata(
          await checkedFetch(
            connection.url.replace(/seed$/, "epoch"),
            {
              headers: connection.headers,
            },
            epoch,
          ),
          epoch,
        ),
      ) as EpochState;
      await publishFreshEpoch(canonical.seedHash, JSON.stringify(canonical.heads));
      return;
    }
    const base = owner.copy!.syncBase(BigInt(Math.floor(performance.now())), BigInt(Date.now()));
    if (!base) {
      // Not eligible yet; each retry re-reads RoomInfo inside the guest metadata budget.
      action = decide({ step: "baseUnavailable" });
      if (action.action === "retryBase") compactAfter = Date.now() + action.milliseconds;
      return;
    }
    if (owner.copy!.supportsEpoch()) {
      const heads = base.frontier();
      base.free();
      await publishFreshEpoch(latest.seedHash!, heads);
      return;
    }
    try {
      const bytes = new Uint8Array(base.bytes());
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      currentEpoch(epoch);
      await checkedFetch(
        connection.url.replace(/seed$/, "compact"),
        {
          method: "POST",
          headers: {
            ...connection.headers,
            "Content-Type": "application/octet-stream",
            "X-Sharing-SHA256": Array.from(digest, (b) => b.toString(16).padStart(2, "0")).join(""),
            "X-Sharing-Version": base.frontier(),
            "X-Sharing-Root": base.root(),
          },
          body: bytes,
        },
        epoch,
      );
    } catch (error) {
      // A 413 here is an `outgrown` pause, which the outer catch applies.
      if (!(error instanceof TransportError) || error.status !== 409) throw error;
    } finally {
      base.free();
    }
    if (decide({ step: "compacted" }).action === "reconnect") {
      retriedFull = true;
      reconnectAfterCompaction(epoch);
    }
  } catch (error) {
    if (epoch !== generation) return;
    checkedRoomAt = 0;
    if (error instanceof TransportError && error.pause) {
      pauseSync(error.pause);
      return;
    }
    console.warn("Room compaction failed; retrying", error);
    compactAfter = Date.now() + backoff;
    backoff = Math.min(SharingHost.backoffMaxMs, backoff * 2);
    if (error instanceof TransportError && (error.status === 410 || error.status === 403))
      resourceFailure(error, epoch);
  } finally {
    if (compacting === epoch) compacting = 0;
  }
}
const remoteAttachments = new Map<string, Resource>();
const sharingEvent = (event: SharingEvent) => postMessage(event);
function reportSync(status: string, phase: SyncPhase) {
  const key = `${phase}:${status}`;
  if (key === lastStatus) return;
  lastStatus = key;
  sharingEvent({ type: "sync", status, phase });
}
class TransportError extends Error {
  constructor(
    readonly status: number,
    readonly pause?: Pause,
  ) {
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
  if (!response.ok) {
    let body: { code: string; message: string } | undefined;
    try {
      body = JSON.parse(await metadata(response, epoch));
    } catch (error) {
      // A missing error body still carries the HTTP status.
      console.warn("Sharing error reply unreadable (" + response.status + ")", error);
    }
    const runtime = (await owner.initialized) as BrowserRuntime;
    const failure = JSON.parse(
      runtime.classifyFailure(response.status, body?.code, url),
    ) as Failure;
    const { outcome } = failure;
    const pause: Pause | undefined =
      outcome === "refused" || outcome === "needsUpdate"
        ? { kind: outcome, reason: body?.message || failure.message }
        : outcome === "outgrown" || outcome === "filesFull"
          ? { kind: outcome, reason: failure.message }
          : undefined;
    throw new TransportError(response.status, pause);
  }
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
    await reader.cancel().catch((cancel) => console.warn("Sharing response not cancelled", cancel));
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
  reportSync("Syncing files", "syncing");
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
  reportSync("Receiving files", "syncing");
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
function detachConnection(code: number, reason: string) {
  // Fence the generation before any queued work can follow the detached connection.
  ++generation;
  networkAbort?.abort();
  const target = socket;
  socket = undefined;
  target?.close(code, reason);
  owner.copy?.syncDetach();
  if (owner.opened) owner.poll();
}
function reconnectAfterCompaction(epoch: number) {
  if (epoch !== generation || ended) return;
  detachConnection(1000, "Room compacted");
  reportSync("Syncing", "syncing");
  clearTimeout(reconnectTimer);
  reconnectTimer = setTimeout(() => {
    void connectSocket().catch((error) => resourceFailure(error));
  }, backoff);
}
function resourceFailure(error: unknown, epoch = generation) {
  if (epoch !== generation || refreshing) return;
  if (error instanceof TransportError && (error.status === 410 || error.status === 403)) {
    ended = true;
    void settleEndedCopy();
    return;
  }
  if (copyState()?.pause?.kind === "missingHistory") {
    void refreshEpoch();
    return;
  }
  if (error instanceof TransportError && error.pause && owner.copy) pauseSync(error.pause);
  sharingEvent({ type: "files", status: "File transfer pending: " + String(error) });
  const state = copyState();
  if (state?.pause?.kind === "filesFull" && owner.copy?.supportsEpoch()) {
    void compactRoom(epoch, true);
    return;
  }
  if (stops(state?.pause)) {
    ended = true;
    reportState(state);
  }
  detachConnection(4011, "File transfer interrupted");
  if (ended) return;
  reportSync("Offline; reconnecting", "offline");
  clearTimeout(reconnectTimer);
  if (!ended)
    reconnectTimer = setTimeout(() => {
      void connectSocket().catch((error) => resourceFailure(error));
    }, backoff);
  backoff = Math.min(SharingHost.backoffMaxMs, backoff * 2);
}
async function connectSocket() {
  if (ended) return;
  networkAbort?.abort();
  networkAbort = new AbortController();
  const epoch = ++generation;
  checkedRoomAt = 0;
  compactAfter = 0;
  remoteAttachments.clear();
  await owner.copy!.recoverStorage();
  const canonical = await metadata(
    await checkedFetch(
      connection.url.replace(/seed$/, "epoch"),
      {
        headers: connection.headers,
      },
      epoch,
    ),
    epoch,
  );
  if (!owner.copy!.hasEpoch(canonical)) {
    await refreshEpoch();
    return;
  }
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
    backoff = SharingHost.backoffMinMs;
    owner.copy!.syncAttach();
    owner.poll();
  };
  ws.onmessage = ({ data }) => {
    if (epoch !== generation) return;
    if (!(data instanceof ArrayBuffer)) return;
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
          let operation: string;
          try {
            operation = runtime.frameOperation(bytes);
          } catch (error) {
            pauseSync({ kind: "refused", reason: String(error) });
            throw error;
          }
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
  // Browsers give no detail here and always follow with `close`, which retries.
  ws.onerror = () => console.warn("Sharing socket error");
  ws.onclose = () => {
    if (epoch !== generation || ended) return;
    resourceFailure(new Error("Connection interrupted"), epoch);
  };
}
function flushSocket() {
  if (refreshing || localRecovery) return;
  if (!owner.copy || socket?.readyState !== WebSocket.OPEN || sending === generation) return;
  const state = copyState();
  const pause = state?.pause;
  if (pause?.kind === "missingHistory") {
    void refreshEpoch();
    return;
  }
  if (stops(pause)) {
    ended = true;
    socket.close(1000, "Sharing paused");
    reportState(state);
    return;
  }
  if (state?.phase === "synced") {
    const confirmation = owner.copy.confirmedVersion();
    if (confirmation !== undefined) {
      retriedFull = false;
      confirmedVersion = confirmation;
    }
  }
  if (
    pause?.kind === "roomFull" ||
    pause?.kind === "filesFull" ||
    Date.now() - checkedRoomAt >= SharingHost.recheckMs
  )
    void compactRoom(generation, pause?.kind === "roomFull" || pause?.kind === "filesFull");
  if (receiving !== generation) reportState(state);
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
/** Guests keep their existing copy when sharing ends. No final fetch or UUID
 * rewrite occurs; failed saves retain the open owner and its accepted local edits. */
async function settleEndedCopy() {
  if (ending || !owner.opened || !owner.copy) return;
  ending = true;
  ended = true;
  reportSync("Sharing ended; keeping this browser copy", "ended");
  detachConnection(1000, "Sharing ended");
  clearTimeout(reconnectTimer);
  try {
    await owner.serialize(async () => {
      owner.copy!.leaveRoom();
      await updateCopy(owner.copyID, { local: true });
      localRecovery = true;
      owner.poll();
    });
    reportSync("Sharing ended; this copy is saved in your browser", "ended");
  } catch (error) {
    reportSync("Sharing ended; this copy still needs to save: " + String(error), "ended");
    owner.poll();
  } finally {
    ending = false;
  }
}

/** Same eligibility policy as legacy /compact, but layout 2 publishes a bounded
 * latest SQLite seed. The relay CAS is the authority change; local adoption follows it. */
async function publishFreshEpoch(seedHash: string, heads: string) {
  if (refreshing || ended || !owner.copy) return;
  refreshing = true;
  detachConnection(1000, "Publishing shared snapshot");
  clearTimeout(reconnectTimer);
  networkAbort = new AbortController();
  const epoch = generation,
    scratch = crypto.randomUUID();
  let reconnect = true;
  try {
    await owner.serialize(async () => {
      const runtime = (await owner.initialized) as BrowserRuntime;
      await putCopy({
        id: scratch,
        title: "Preparing shared snapshot…",
        created: Date.now(),
        state: "importing",
      });
      await navigator.locks.request(lockName(scratch), async () => {
        await checkContainer(scratch, true);
        const candidate = await runtime.installCopy(scratch);
        const endpoint = connection.url.replace(/seed$/, "");
        const cleanupHeaders = connection.headers;
        let stagedHash: string | undefined;
        try {
          const prepared = JSON.parse(
            owner.copy!.prepareEpoch(
              candidate,
              JSON.stringify({ seedHash, heads: JSON.parse(heads) }),
            ),
          ) as {
            sourceVersion: string;
            size: number;
            publish: PublishEpoch;
          };
          const root = await navigator.storage.getDirectory();
          const folder = await (
            await root.getDirectoryHandle("exports", { create: true })
          ).getDirectoryHandle(scratch, { create: true });
          const file = await folder.getFileHandle("epoch.pop", { create: true });
          const handle = await (file as any).createSyncAccessHandle();
          try {
            handle.truncate(0);
            let offset = 0;
            candidate.exportEpoch((bytes) => {
              if (handle.write(bytes, { at: offset }) !== bytes.byteLength)
                throw new Error("Incomplete epoch export");
              offset += bytes.byteLength;
            });
            if (offset !== prepared.size) throw new Error("Epoch export size changed");
            handle.flush();
          } finally {
            handle.close();
          }
          stagedHash = prepared.publish.seedHash;
          await checkedFetch(
            endpoint + "epochs/" + prepared.publish.seedHash,
            {
              method: "PUT",
              headers: { ...connection.headers, "Content-Type": "application/octet-stream" },
              body: await file.getFile(),
            },
            epoch,
          );
          // Candidate inventory contains only referenced blobs. Existing immutable app
          // resources belong to the same embedded app and are checked again at commit.
          const inventory = JSON.parse(
            await metadata(
              await checkedFetch(
                endpoint + "blobs",
                {
                  headers: connection.headers,
                },
                epoch,
              ),
              epoch,
            ),
          ) as Resource[];
          const present = new Set(inventory.map((item) => item.hash));
          for (const item of prepared.publish.attachments) {
            if (present.has(item.hash)) continue;
            await checkedFetch(
              endpoint + "epochs/" + prepared.publish.seedHash + "/blobs/" + item.hash,
              {
                method: "PUT",
                headers: { ...connection.headers, "Content-Type": item.media_type },
                body: new Uint8Array(candidate.epochAttachment(item.key)),
              },
              epoch,
            );
          }
          const receipt = await checkedFetch(
            endpoint + "epoch",
            {
              method: "POST",
              headers: { ...connection.headers, "Content-Type": "application/json" },
              body: JSON.stringify(prepared.publish),
            },
            epoch,
          );
          candidate.confirmEpoch(await metadata(receipt, epoch));
          await adoptCandidate(
            runtime,
            candidate,
            prepared.publish.seedHash,
            prepared.sourceVersion,
            epoch,
          );
          confirmedVersion = "";
          retriedFull = true;
          owner.poll();
        } finally {
          try {
            candidate.abortImport();
            candidate.releaseStorage();
          } finally {
            // A lost publication response is safe: DELETE refuses the current hash.
            // Cleanup has its own timeout rather than the abandoned socket's signal.
            if (stagedHash)
              await fetch(endpoint + "epochs/" + stagedHash, {
                method: "DELETE",
                headers: cleanupHeaders,
                signal: AbortSignal.timeout(5000),
              }).catch(() => {});
          }
        }
      });
    });
  } catch (error) {
    if (
      (error instanceof TransportError && error.pause?.kind === "outgrown") ||
      (error instanceof Error && "code" in error && error.code === "outgrown")
    ) {
      reconnect = false;
      reportSync("This document has outgrown sharing", "paused");
    } else if (!(error instanceof TransportError && error.status === 409)) {
      // A lost commit acknowledgment is resolved by the next epoch handshake; the
      // local owner is unchanged unless its durable adoption already succeeded.
      reportSync("Shared maintenance interrupted: " + String(error), "offline");
    }
  } finally {
    refreshing = false;
    await removeCopy(scratch).catch(() => {});
  }
  if (reconnect && !ended) await connectSocket().catch((error) => resourceFailure(error));
}

/** Called inside the serialized owner queue after its sync connection is detached. */
async function adoptCandidate(
  runtime: BrowserRuntime,
  candidate: BrowserCopy,
  seedHash: string,
  expected: string,
  epoch: number,
) {
  const source = (await listCopies()).find((copy) => copy.id === owner.copyID);
  const version = owner.copy!.currentVersion();
  const recovery = await reserveRecovery(
    owner.copyID,
    owner.copy!.documentIdentity(),
    source?.epoch ?? "unknown",
    version,
  );
  let unused = false;
  try {
    await navigator.locks.request(lockName(recovery.id), { ifAvailable: true }, async (lock) => {
      if (!lock) throw new Error("The recovery copy is open in another tab");
      await checkContainer(recovery.id, true);
      const installed = await runtime.installCopy(recovery.id);
      try {
        currentEpoch(epoch);
        const recovered = owner.copy!.adoptEpoch(candidate, installed, expected);
        // A failed catalog acknowledgment retains the discoverable reservation.
        if (recovered) await updateCopy(recovery.id, { state: "ready" });
        else unused = true;
        await updateCopy(owner.copyID, { epoch: seedHash });
      } finally {
        installed.releaseStorage();
      }
    });
  } finally {
    if (unused) await removeCopy(recovery.id).catch(() => {});
  }
}

/** Download into a separate OPFS pool. Old state remains editable until this
 * operation joins the owner queue; no relay snapshot is merged across epochs. */
async function refreshEpoch() {
  if (refreshing || ended || !owner.copy) return;
  refreshing = true;
  // Fence remote imports before the first await and before reserving a source version.
  detachConnection(1000, "Refreshing shared snapshot");
  clearTimeout(reconnectTimer);
  networkAbort = new AbortController();
  const epoch = generation;
  const scratch = crypto.randomUUID();
  try {
    await owner.serialize(async () => {
      const runtime = (await owner.initialized) as BrowserRuntime;
      const base = connection.url.replace(/seed$/, "");
      const info = await metadata(
        await checkedFetch(
          base.slice(0, -1),
          {
            headers: connection.headers,
            credentials: "omit",
          },
          epoch,
        ),
        epoch,
      );
      const canonical = await metadata(
        await checkedFetch(
          base + "epoch",
          {
            headers: connection.headers,
            credentials: "omit",
          },
          epoch,
        ),
        epoch,
      );
      const plan = JSON.parse(
        runtime.invitationRequest(invitation, config.relay, info),
      ) as ConnectionPlan;
      const remote = JSON.parse(canonical) as { seedHash: string };
      await putCopy({
        id: scratch,
        title: "Refreshing shared snapshot…",
        created: Date.now(),
        state: "importing",
      });
      await navigator.locks.request(lockName(scratch), async () => {
        await checkContainer(scratch, true);
        const candidate = await runtime.installCopy(scratch);
        try {
          const response = await checkedFetch(
            plan.url + "?hash=" + encodeURIComponent(remote.seedHash),
            {
              headers: plan.headers,
              credentials: "omit",
            },
            epoch,
          );
          if (!response.body || plan.size < 100 || plan.size > BrowserHost.fileBytes)
            throw new Error("Invalid epoch seed");
          candidate.importBegin(plan.size);
          const reader = response.body.getReader();
          let received = 0;
          try {
            while (true) {
              const { done, value } = await reader.read();
              currentEpoch(epoch);
              if (done) break;
              received += value.length;
              if (received > plan.size) throw new Error("Epoch seed exceeds declared size");
              for (let offset = 0; offset < value.length; offset += BrowserHost.chunkBytes)
                candidate.importWrite(value.subarray(offset, offset + BrowserHost.chunkBytes));
            }
            if (received !== plan.size) throw new Error("Incomplete epoch seed");
            candidate.importFinish();
          } finally {
            await reader.cancel().catch(() => {});
          }
          candidate.validateEpoch(info, canonical);
          await adoptCandidate(runtime, candidate, remote.seedHash, confirmedVersion, epoch);
          roomInfo = info;
          connection = plan;
          confirmedVersion = "";
          owner.poll();
        } finally {
          candidate.abortImport();
          candidate.releaseStorage();
        }
      });
    });
    refreshing = false;
    await connectSocket();
  } catch (error) {
    // Refusal, quota and uncertain writes keep the old copy and its recovery reservation.
    // No automatic retry may overwrite either one while authority is uncertain.
    if (error instanceof TransportError && (error.status === 410 || error.status === 403)) {
      ended = true;
      void settleEndedCopy();
    } else reportSync("Shared refresh paused: " + String(error), "paused");
  } finally {
    refreshing = false;
    await removeCopy(scratch).catch(() => {});
  }
}
let roomInfo = "";
const owner = new WorkerOwner<BrowserCopy>({
  async control(input) {
    const control = input as Partial<SharingControl> | null;
    if (control?.type !== "joinCredential") return false;
    const candidate = control.credential;
    if (candidate === "" && !owner.copy) {
      localRecovery = true;
      return true;
    }
    if (
      owner.copy ||
      typeof candidate !== "string" ||
      !candidate.startsWith(`${config.host}/join#`) ||
      !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.[0-9a-f]{64}$/.test(
        candidate.slice(`${config.host}/join#`.length),
      )
    )
      throw new Error("Sharing link is no longer valid");
    invitation = candidate;
    const [room, capability] = candidate.split("#")[1]!.split(".");
    let response: Response;
    try {
      response = await checkedFetch(`${config.relay}/v1/documents/${room}`, {
        headers: { Authorization: `Bearer ${capability}` },
        credentials: "omit",
      });
    } catch (error) {
      if (!(error instanceof TransportError) || (error.status !== 410 && error.status !== 403))
        throw error;
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
    if (localRecovery) {
      installed.openLocal();
      return;
    }
    if (ended) {
      installed.openSavedJoined(invitation, config.relay);
      reportSync("Sharing ended; keeping this browser copy", "ended");
    } else installed.openJoined(invitation, config.relay, roomInfo);
  },
  afterPoll: flushSocket,
  afterOpen() {
    if (localRecovery) {
      reportSync("Local browser copy", "ended");
      sharingEvent({ type: "resourcesReady" });
      return;
    }
    if (ended) {
      sharingEvent({ type: "resourcesReady" });
      void settleEndedCopy();
      return;
    }
    setInterval(() => {
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send("ping");
        flushSocket();
      }
    }, SharingHost.pingMs);
    setInterval(() => {
      const epoch = generation;
      if (!ended) void resourceSession(epoch).catch((error) => resourceFailure(error, epoch));
    }, SharingHost.resourceRenewMs);
    void connectSocket().catch((error) => resourceFailure(error));
  },
});
onmessage = owner.handle;

import { GuestIdentity, GuestIdentityAttention } from "./identity";
import { config, frameOrigin } from "./config";
import {
  WorkerOwner,
  type BrowserCopy as LocalCopy,
  type BrowserRuntime as LocalRuntime,
} from "../browser/worker-owner";
import { updateCopy } from "../browser/copies";
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
  SharingControl,
  SharingEvent,
  SyncPhase,
  SyncState,
  BudgetDecision,
} from "../wire/sharing.generated";
import { SharingHost } from "../wire/constants.generated";

interface BrowserCopy extends LocalCopy {
  openLocal(): void;
  leaveRoom(): void;
  confirmedVersion(): string | undefined;
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
  ):
    | {
        bytes(): Uint8Array;
        frontier(): string;
        tooLarge(): boolean;
        free(): void;
      }
    | undefined;
  resourceRange(attachment: boolean, key: string, offset: number, length: number): Uint8Array;
  verifyResources(json: string): string;
  attachmentInventory(): string;
  missingAttachments(json: string): string;
  missingUploads(json: string): string;
  installAttachment(item: string, bytes: Uint8Array, monotonic: number, unix: number): void;
}
interface SocketBudget {
  admit(monotonic: bigint, bytes: Uint8Array): string;
  acknowledge(monotonic: bigint, bytes: Uint8Array): void;
  free(): void;
}
interface SharingSupervisor {
  connect(): boolean;
  live(): boolean;
  retry(): number | undefined;
  stop(): void;
  free(): void;
}
interface BrowserRuntime extends LocalRuntime<BrowserCopy> {
  SocketPushBudget: new (monotonic: bigint) => SocketBudget;
  SharingSupervisor: new () => SharingSupervisor;
  invitationRequest(link: string, origin: string, info: string): string;
  frameOperation(bytes: Uint8Array): string;
  classifyFailure(status: number, code: string | undefined, url: string): string;
  maintenanceAction(full: string, step: string, random: number): string;
}
const identity = new GuestIdentity();
let identityUnavailable = false;
let identityFailure: Error | undefined;
let invitation = "";
let localRecovery = false;
let ending = false;
let socket: WebSocket | undefined;
let pushBudget: SocketBudget | undefined;
let pendingPush: Uint8Array | undefined;
let pushWakeAt = 0;
let pushTimer: ReturnType<typeof setTimeout> | undefined;
const monotonic = () => BigInt(Math.floor(performance.now()));
function clearPushBudget() {
  clearTimeout(pushTimer);
  pendingPush = undefined;
  pushWakeAt = 0;
  pushBudget?.free();
  pushBudget = undefined;
}
let generation = 0;
let connectionRetry: SharingSupervisor | undefined;
let maintenanceRetry: SharingSupervisor | undefined;
function supervisors(runtime: BrowserRuntime) {
  connectionRetry ??= new runtime.SharingSupervisor();
  maintenanceRetry ??= new runtime.SharingSupervisor();
}
function stopSupervisors() {
  connectionRetry?.stop();
  maintenanceRetry?.stop();
}
function scheduleReconnect() {
  const milliseconds = connectionRetry?.retry();
  if (milliseconds === undefined) return;
  const attempt = generation;
  clearTimeout(reconnectTimer);
  reconnectTimer = setTimeout(() => {
    if (attempt !== generation || ended) return;
    void connectSocket().catch((error) => resourceFailure(error));
  }, milliseconds);
}
let lastStatus = "";
let sending = 0;
let receiving = 0;
let connection: ConnectionPlan;
/** A route of the joined document. Rust checked `connection.document` against its invitation. */
const documentURL = (route = "") => `${config.relay}/v1/documents/${connection.document}${route}`;
let networkAbort: AbortController | undefined;
let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
let ended = false;
let resourcesReady = false;
let compacting = 0;
let compactAfter = 0;
let checkedRoomAt = 0;
let retriedFull = false;
/** Room-full waits for compaction, which this worker runs, so only the other pauses stop it. */
const stops = (pause: Pause | undefined) => pause !== undefined && pause.kind !== "roomFull";
const outgrown: Pause = { kind: "outgrown", reason: "This document has outgrown sharing" };
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
function maintenanceDecision(
  runtime: BrowserRuntime,
  step: Step,
  random = 0,
  full = false,
): MaintenanceAction {
  const kind = copyState()?.pause?.kind;
  const state: FullState =
    full || kind === "roomFull" ? (retriedFull ? "retried" : "waiting") : "available";
  return JSON.parse(runtime.maintenanceAction(JSON.stringify(state), JSON.stringify(step), random));
}
async function compactRoom(attempt: number, full: boolean) {
  if (compacting || Date.now() < compactAfter) return;
  compacting = attempt;
  try {
    const info = async () =>
      JSON.parse(
        await metadata(await checkedFetch(documentURL(), {}, attempt), attempt),
      ) as RoomInfo;
    const runtime = (await owner.initialized) as BrowserRuntime;
    supervisors(runtime);
    if (!maintenanceRetry!.connect()) return;
    const decide = (step: Step, random = 0) => maintenanceDecision(runtime, step, random, full);
    const first = await info();
    checkedRoomAt = Date.now();
    let action = decide(
      { step: "probe", compact: first.compact },
      crypto.getRandomValues(new Uint16Array(1))[0]!,
    );
    if (action.action === "outgrown") {
      pauseSync(outgrown);
      return;
    }
    if (action.action !== "jitter") return;
    const delay = action.milliseconds;
    const signal = networkAbort?.signal;
    await new Promise<void>((resolve) => {
      if (!signal || signal.aborted) return resolve();
      const timer = setTimeout(resolve, delay);
      signal.addEventListener("abort", () => (clearTimeout(timer), resolve()), { once: true });
    });
    assertCurrent(attempt);
    const latest = await info();
    action = decide({ step: "recheck", compact: latest.compact });
    if (action.action === "reconnect") {
      retriedFull = true;
      reconnectAfterCompaction(attempt);
      return;
    }
    if (action.action !== "readBase") return;
    const base = owner.copy!.syncBase(BigInt(Math.floor(performance.now())), BigInt(Date.now()));
    if (!base) {
      // Not eligible yet; each retry re-reads RoomInfo inside the guest metadata budget.
      action = decide({ step: "baseUnavailable" });
      if (action.action === "retryBase") compactAfter = Date.now() + action.milliseconds;
      return;
    }
    if (base.tooLarge()) {
      base.free();
      if (decide({ step: "baseTooLarge" }).action === "outgrown") pauseSync(outgrown);
      return;
    }
    try {
      const bytes = new Uint8Array(base.bytes());
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      assertCurrent(attempt);
      await checkedFetch(
        documentURL("/compact"),
        {
          method: "POST",
          headers: {
            "Content-Type": "application/octet-stream",
            "X-Sharing-SHA256": Array.from(digest, (b) => b.toString(16).padStart(2, "0")).join(""),
            "X-Sharing-Version": base.frontier(),
          },
          body: bytes,
        },
        attempt,
      );
    } catch (error) {
      // A 413 here is an `outgrown` pause, which the outer catch applies. Another
      // writer's fold conflicts: reconnect to catch up, as the native client does.
      if (!(error instanceof TransportError) || error.status !== 409) throw error;
      reconnectAfterCompaction(attempt);
      return;
    } finally {
      base.free();
    }
    if (decide({ step: "compacted" }).action === "reconnect") {
      retriedFull = true;
      reconnectAfterCompaction(attempt);
    }
  } catch (error) {
    if (attempt !== generation) return;
    checkedRoomAt = 0;
    if (error instanceof TransportError && error.pause) {
      pauseSync(error.pause);
      return;
    }
    console.warn("Room compaction failed; retrying", error);
    const milliseconds = maintenanceRetry?.retry();
    if (milliseconds !== undefined) compactAfter = Date.now() + milliseconds;
    if (error instanceof TransportError && (error.status === 410 || error.status === 403))
      resourceFailure(error, attempt);
  } finally {
    if (compacting === attempt) compacting = 0;
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
function assertCurrent(attempt: number) {
  if (attempt !== generation) throw new DOMException("Connection replaced", "AbortError");
}
async function checkedFetch(url: string, init: RequestInit = {}, attempt = generation) {
  assertCurrent(attempt);
  const signal = networkAbort?.signal;
  const headers = new Headers(init.headers);
  try {
    headers.set("Authorization", `Bearer ${await identity.token()}`);
    identityUnavailable = false;
    identityFailure = undefined;
  } catch (error) {
    identityUnavailable = true;
    identityFailure =
      error instanceof Error ? error : new Error("Guest session is unavailable; retry later");
    if (error instanceof GuestIdentityAttention) {
      sharingEvent({ type: "identityAttention", message: error.message });
    }
    throw error;
  }
  assertCurrent(attempt);
  const response = await fetch(url, {
    ...init,
    headers,
    signal: signal
      ? AbortSignal.any([signal, AbortSignal.timeout(30_000)])
      : AbortSignal.timeout(30_000),
  });
  assertCurrent(attempt);
  if (!response.ok) {
    let body: { code: string; message: string } | undefined;
    try {
      body = JSON.parse(await metadata(response, attempt));
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
async function boundedBody(response: Response, limit: number, attempt: number) {
  if (!response.body) throw new Error("Missing response body");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      assertCurrent(attempt);
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
  assertCurrent(attempt);
  const bytes = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.length;
  }
  return bytes;
}
async function metadata(response: Response, attempt: number) {
  return new TextDecoder().decode(await boundedBody(response, SharingHost.controlBytes, attempt));
}
async function resourceSession(attempt: number) {
  const manifest = await checkedFetch(documentURL("/resources"), {}, attempt);
  const app = owner.copy!.verifyResources(await metadata(manifest, attempt));
  const grant: ResourceGrant = { app };
  await checkedFetch(
    frameOrigin(owner.copyID) + "/_resources/" + connection.document,
    {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(grant),
    },
    attempt,
  );
  if (!resourcesReady) {
    resourcesReady = true;
    sharingEvent({ type: "resourcesReady" });
  }
}
async function uploadFiles(attempt: number) {
  reportSync("Syncing files", "syncing");
  sharingEvent({ type: "files", status: "Uploading files" });
  for (const item of JSON.parse(
    owner.copy!.missingUploads(JSON.stringify([...remoteAttachments.values()])),
  ) as Resource[]) {
    assertCurrent(attempt);
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
      documentURL("/blob/" + item.hash),
      {
        method: "PUT",
        headers: { "Content-Type": item.media_type },
        body: new Blob(chunks.map((chunk) => new Uint8Array(chunk).buffer)),
      },
      attempt,
    );
    assertCurrent(attempt);
    remoteAttachments.set(item.hash, item);
  }
}
async function receiveFiles(attempt: number) {
  receiving = attempt;
  try {
    await receiveInventory(attempt);
  } finally {
    if (receiving === attempt) receiving = 0;
  }
}
async function receiveInventory(attempt: number) {
  reportSync("Receiving files", "syncing");
  sharingEvent({ type: "files", status: "Receiving files" });
  const list = await checkedFetch(documentURL("/blobs"), {}, attempt);
  const inventory = await metadata(list, attempt);
  const missing = JSON.parse(owner.copy!.missingAttachments(inventory)) as Resource[];
  // Rust validated the complete inventory, including metadata for existing hashes.
  for (const item of JSON.parse(inventory) as Resource[]) remoteAttachments.set(item.hash, item);
  for (const item of missing) {
    const response = await checkedFetch(documentURL("/blob/" + item.hash), {}, attempt);
    const bytes = await boundedBody(response, item.size, attempt);
    if (bytes.length !== item.size) throw new Error("Incomplete attachment");
    assertCurrent(attempt);
    owner.copy!.installAttachment(JSON.stringify(item), bytes, performance.now(), Date.now());
  }
  sharingEvent({ type: "files", status: "Files ready" });
}
function detachConnection(code: number, reason: string) {
  // Fence the generation before any queued work can follow the detached connection.
  ++generation;
  clearPushBudget();
  networkAbort?.abort();
  const target = socket;
  socket = undefined;
  target?.close(code, reason);
  owner.copy?.syncDetach();
  if (owner.opened) owner.poll();
}
function reconnectAfterCompaction(attempt: number) {
  if (attempt !== generation || ended) return;
  detachConnection(1000, "Room compacted");
  reportSync("Syncing", "syncing");
  scheduleReconnect();
}
function resourceFailure(error: unknown, attempt = generation) {
  if (attempt !== generation) return;
  if (error instanceof GuestIdentityAttention) {
    identityUnavailable = true;
    detachConnection(4011, "Guest session needs attention");
    clearTimeout(reconnectTimer);
    stopSupervisors();
    sharingEvent({ type: "identityAttention", message: error.message });
    reportSync("Guest session needs attention; this browser copy is kept", "paused");
    return;
  }
  if (error instanceof TransportError && (error.status === 410 || error.status === 403)) {
    ended = true;
    void settleEndedCopy();
    return;
  }
  if (error instanceof TransportError && error.pause && owner.copy) pauseSync(error.pause);
  sharingEvent({ type: "files", status: "File transfer pending: " + String(error) });
  const state = copyState();
  if (stops(state?.pause)) {
    ended = true;
    stopSupervisors();
    reportState(state);
  }
  detachConnection(4011, "File transfer interrupted");
  if (ended) return;
  reportSync("Offline; reconnecting", "offline");
  scheduleReconnect();
}
async function connectSocket() {
  if (ended) return;
  const runtime = (await owner.initialized) as BrowserRuntime;
  supervisors(runtime);
  if (ended || !connectionRetry!.connect()) return;
  clearPushBudget();
  networkAbort?.abort();
  networkAbort = new AbortController();
  const attempt = ++generation;
  checkedRoomAt = 0;
  compactAfter = 0;
  remoteAttachments.clear();
  let token: string;
  try {
    await owner.copy!.recoverStorage();
    await resourceSession(attempt);
    await receiveFiles(attempt);
    assertCurrent(attempt);
    token = await identity.token();
    assertCurrent(attempt);
  } catch (error) {
    // Attribute the failure to this attempt: a replaced attempt's abort must not run
    // recovery (backoff, detach, events) against the newer one.
    resourceFailure(error, attempt);
    return;
  }
  pushBudget = new runtime.SocketPushBudget(monotonic());
  const ws = new WebSocket(documentURL("/sync").replace(/^https:/, "wss:"), [
    SharingHost.socketProtocol,
    `jwt.${token}`,
  ]);
  socket = ws;
  ws.binaryType = "arraybuffer";
  // Each connection owns its queue and counters. An abandoned HTTP read never blocks
  // the next connection or decrements its budget.
  let incoming = Promise.resolve(),
    incomingBytes = 0,
    incomingFrames = 0;
  ws.onopen = () => {
    if (attempt !== generation) {
      ws.close();
      return;
    }
    if (!connectionRetry!.live()) {
      ws.close(1000, "Sharing stopped");
      return;
    }
    maintenanceRetry!.live();
    owner.copy!.syncAttach();
    owner.poll();
  };
  ws.onmessage = ({ data }) => {
    if (attempt !== generation) return;
    if (!(data instanceof ArrayBuffer)) return;
    incomingBytes += data.byteLength;
    incomingFrames++;
    if (incomingBytes > SharingHost.queueBytes || incomingFrames > SharingHost.queueFrames) {
      resourceFailure(new Error("Incoming queue budget exceeded"), attempt);
      return;
    }
    incoming = incoming
      .then(async () => {
        try {
          assertCurrent(attempt);
          const runtime = (await owner.initialized) as BrowserRuntime;
          assertCurrent(attempt);
          const bytes = new Uint8Array(data);
          let operation: string;
          try {
            operation = runtime.frameOperation(bytes);
          } catch (error) {
            pauseSync({ kind: "refused", reason: String(error) });
            throw error;
          }
          if (operation === "ended") {
            resourceFailure(new TransportError(410), attempt);
            return;
          }
          if (operation === "download") await receiveFiles(attempt);
          assertCurrent(attempt);
          if (operation === "acknowledge") pushBudget?.acknowledge(monotonic(), bytes);
          owner.copy!.syncFrame(bytes);
          owner.poll();
        } finally {
          incomingBytes -= data.byteLength;
          incomingFrames--;
        }
      })
      .catch((error) => resourceFailure(error, attempt));
  };
  // Browsers give no detail here and always follow with `close`, which retries.
  ws.onerror = () => console.warn("Sharing socket error");
  ws.onclose = () => {
    if (attempt !== generation || ended) return;
    resourceFailure(new Error("Connection interrupted"), attempt);
  };
}
function flushSocket() {
  if (localRecovery) return;
  if (!owner.copy || socket?.readyState !== WebSocket.OPEN || sending === generation) return;
  const state = copyState();
  const pause = state?.pause;
  if (stops(pause)) {
    ended = true;
    stopSupervisors();
    detachConnection(1000, "Sharing paused");
    reportState(state);
    return;
  }
  if (state?.phase === "synced" && owner.copy.confirmedVersion() !== undefined) retriedFull = false;
  if (pause?.kind === "roomFull" || Date.now() - checkedRoomAt >= SharingHost.recheckMs)
    void compactRoom(generation, pause?.kind === "roomFull");
  if (receiving !== generation) reportState(state);
  let frame: Uint8Array | undefined;
  try {
    frame = owner.copy.nextSyncFrame();
    if (!frame && pendingPush && performance.now() >= pushWakeAt) frame = pendingPush;
  } catch (error) {
    resourceFailure(error);
    return;
  }
  if (!frame) return;
  const bytes = frame,
    attempt = generation,
    target = socket;
  sending = attempt;
  void (async () => {
    const runtime = (await owner.initialized) as BrowserRuntime;
    assertCurrent(attempt);
    if (runtime.frameOperation(bytes) === "upload") {
      const decision = JSON.parse(pushBudget!.admit(monotonic(), bytes)) as BudgetDecision;
      if (decision.action === "wait") {
        pendingPush = bytes;
        pushWakeAt = performance.now() + decision.milliseconds;
        clearTimeout(pushTimer);
        pushTimer = setTimeout(() => {
          pushWakeAt = 0;
          flushSocket();
        }, decision.milliseconds);
        reportSync("Syncing; waiting for the next upload window", "syncing");
        return;
      }
      pendingPush = undefined;
      pushWakeAt = 0;
      clearTimeout(pushTimer);
      if (decision.action === "tooLarge") {
        pauseSync({ kind: "outgrown", reason: "This change is too large to share" });
        return;
      }
      await uploadFiles(attempt);
    }
    assertCurrent(attempt);
    if (target.readyState !== WebSocket.OPEN) return;
    if (target.bufferedAmount + bytes.length > SharingHost.queueBytes)
      throw new Error("Outgoing queue budget exceeded");
    target.send(new Uint8Array(bytes));
    sharingEvent({ type: "files", status: "Files ready" });
  })()
    .catch((error) => resourceFailure(error, attempt))
    .finally(() => {
      if (sending === attempt) sending = 0;
      flushSocket();
    });
}
/** Guests keep their existing copy when sharing ends. No final fetch or UUID
 * rewrite occurs; failed saves retain the open owner and its accepted local edits. */
async function settleEndedCopy() {
  if (ending || !owner.opened || !owner.copy) return;
  ending = true;
  ended = true;
  stopSupervisors();
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

let roomInfo = "";
const owner = new WorkerOwner<BrowserCopy>({
  async control(input) {
    const control = input as Partial<SharingControl> | null;
    if (control?.type === "openLocal") {
      if (owner.copy) throw new Error("This copy is already open");
      localRecovery = true;
      return true;
    }
    if (control?.type !== "joinInvitation") return false;
    // A shared copy without its invitation (cleared site data) is refused, never opened locally.
    const candidate = control.invitation;
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
      await checkedFetch(`${config.relay}/v1/documents/${room}/members`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ capability }),
        credentials: "omit",
      });
      response = await checkedFetch(`${config.relay}/v1/documents/${room}`, {
        credentials: "omit",
      });
    } catch (error) {
      if (identityFailure) return true;
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
      if (identityUnavailable) throw identityFailure!;
      if (ended) throw new TransportError(410);
      const response = await checkedFetch(documentURL("/seed"), { credentials: "omit" });
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
    if (identityUnavailable) {
      installed.openSavedJoined(invitation, config.relay);
      return;
    }
    if (ended) {
      installed.openSavedJoined(invitation, config.relay);
      reportSync("Sharing ended; keeping this browser copy", "ended");
    } else installed.openJoined(invitation, config.relay, roomInfo);
  },
  afterPoll: flushSocket,
  afterOpen() {
    if (identityUnavailable) {
      reportSync("Guest session needs attention; this browser copy is kept", "paused");
      sharingEvent({ type: "resourcesReady" });
      return;
    }
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
      const attempt = generation;
      if (!ended && !identityUnavailable)
        void resourceSession(attempt).catch((error) => resourceFailure(error, attempt));
    }, SharingHost.resourceRenewMs);
    void connectSocket().catch((error) => resourceFailure(error));
  },
});
onmessage = owner.handle;

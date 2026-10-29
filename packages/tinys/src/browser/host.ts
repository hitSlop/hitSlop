import { config } from "../sharing/config";
import { resourceDelivery } from "./resource-delivery";
import type { SharingEvent, SharingControl } from "../wire/sharing.generated";
import { BrowserHost } from "../schema/constants";
import type { BrowserRequest } from "../wire/browser.generated";
import type { Capability, WindowDefinition } from "../wire/app.generated";
import type { PageFailure } from "../wire/page";
import { cleanup, listCopies, lockName, putCopy, removeCopy } from "./copies";
import { JoinGate } from "./join";
import { silhouetteClip } from "../schema/presentation";
import { capabilityFeatures } from "../schema/policy";
import {
  PendingRequests,
  copyDeadline,
  type CallBody,
  type CallReply,
  type CallType,
  type Deadline,
  type FrameMessage,
  type HostReply,
  type WorkerEvent,
} from "./requests";

const status = document.querySelector<HTMLElement>("#status")!;
const title = document.querySelector<HTMLElement>("#title")!;
const stage = document.querySelector<HTMLElement>("#stage")!;
const download = document.querySelector<HTMLButtonElement>("#download")!;
const retry = document.querySelector<HTMLButtonElement>("#retry")!;
const fullscreen = document.querySelector<HTMLButtonElement>("#fullscreen")!;
const awake = document.querySelector<HTMLButtonElement>("#awake")!;
const hosted = resourceDelivery.kind === "network";
const join = new JoinGate();
const params = new URLSearchParams(location.search);
let id = params.get("copy");
let source = hosted
  ? params.get("fresh") === "1"
    ? "0".repeat(64)
    : null
  : new URLSearchParams(location.hash.slice(1)).get("import");
const fail = (error: unknown) => {
  status.textContent = String(error);
  status.setAttribute("role", "alert");
};
let worker: Worker | undefined;
let page: MessagePort | undefined;
let frame: HTMLIFrameElement | undefined;
let surface: HTMLDivElement | undefined;
let windowSpec: WindowDefinition | undefined;
/** Everything this tab waits on: the worker's replies and the page's answers to `host` messages.
 * Once it has failed, the worker is gone for good: nothing more is sent to it or believed. */
const requests = new PendingRequests<WorkerEvent | HostReply>();
const send = (message: BrowserRequest) => worker!.postMessage(message);
const toFrame = (message: FrameMessage) => page?.postMessage(message);
const copyTimeout: Deadline = {
  after: copyDeadline,
  message: "The copy did not respond; keep this tab open and retry",
};
/** Asks the worker and waits for its paired reply. A copy that ran out the deadline is still
 * usable, so the person can retry; pass `null` where the caller holds its own deadline. */
async function request<T extends CallType>(
  message: { type: T } & CallBody<T>,
  timeout: Deadline | null = copyTimeout,
): Promise<CallReply<T>> {
  const reply = await requests.start((id) => send({ ...message, id } as BrowserRequest), {
    timeout: timeout ?? undefined,
  });
  // The worker answers each request with the event the wire pairs with it (`answeredBy`), but
  // the table matches on the ID alone.
  return reply as CallReply<T>;
}
async function drain() {
  if (requests.failure) throw requests.failure;
  const target = page;
  if (!target) throw new Error("Document is still opening");
  await requests.start(
    (id) =>
      target.postMessage({ type: "host", id, request: { method: "flush" } } satisfies FrameMessage),
    { timeout: { after: 30_000, message: "The app did not finish saving" } },
  );
}
/** The worker has failed for good, so what it was asked is unanswerable and none of it may be
 * repeated. Requests this tab holds fail now and later ones on arrival; the page frame, which
 * holds the page's own, is told to do the same. The worker itself is left to the tab:
 * terminating it could drop accepted edits it has not saved yet. */
function stop(error: Error) {
  if (requests.failure) return;
  requests.fail(error);
  download.disabled = true;
  fail(error.message);
  tellFrame();
}
/** The frame's last message, then the page connection is closed: nothing more crosses it. */
function tellFrame() {
  if (!requests.failure || !page) return;
  toFrame({ type: "fatal", error: requests.failure.message });
  page.close();
  page = undefined;
}
/** One Service Worker fetch, relayed to the worker and answered on the asset channel. The frame
 * and the Service Worker each bound it at 30 s, so the host adds no deadline of its own. */
type ResourceFetch = { id: string; route: "app" | "attachment"; key: string } & (
  | { type: "resourcePlan"; range: string | null }
  | { type: "resource"; offset: number | null; length: number | null }
);
async function relayResource(port: MessagePort, fetched: ResourceFetch) {
  try {
    const reply =
      fetched.type === "resourcePlan"
        ? await request(
            { type: "resourcePlan", route: fetched.route, key: fetched.key, range: fetched.range },
            null,
          )
        : await request(
            {
              type: "resource",
              route: fetched.route,
              key: fetched.key,
              offset: fetched.offset,
              length: fetched.length,
            },
            null,
          );
    port.postMessage({ ...reply, id: fetched.id }, reply.bytes ? [reply.bytes.buffer] : []);
  } catch (error) {
    port.postMessage({ type: "error", id: fetched.id, error: (error as Error).message });
  }
}
function fit() {
  if (!surface || !windowSpec) return;
  const { frame } = windowSpec;
  const fixed = frame.kind === "skin" || !frame.frame.resizable;
  const ratio = frame.kind === "standard" && frame.frame.lockAspect;
  let width = stage.clientWidth,
    height = stage.clientHeight;
  if (ratio) {
    const scale = Math.min(width / windowSpec.width, height / windowSpec.height);
    width = windowSpec.width * scale;
    height = windowSpec.height * scale;
  }
  if (fixed) {
    const scale = Math.min(width / windowSpec.width, height / windowSpec.height);
    surface.style.width = `${windowSpec.width}px`;
    surface.style.height = `${windowSpec.height}px`;
    surface.style.transform = `scale(${scale})`;
  } else {
    surface.style.width = `${width}px`;
    surface.style.height = `${height}px`;
    surface.style.transform = "";
  }
}
function mount(spec: WindowDefinition, capabilities: readonly Capability[]) {
  windowSpec = spec;
  fullscreen.hidden = !spec.fullscreenable;
  frame = document.createElement("iframe");
  frame.title = title.textContent || "Tiny";
  const origin = resourceDelivery.frameOrigin(id!);
  frame.src = `${origin}/${join.ended ? "?local=1" : ""}`;
  frame.setAttribute(
    "sandbox",
    "allow-scripts allow-same-origin allow-forms allow-downloads allow-popups",
  );
  // Devices are delegated to the copy's origin only when the app declared them.
  frame.allow = ["fullscreen", ...capabilityFeatures(capabilities)].join("; ");
  const connect = (event: MessageEvent) => {
    if (
      event.origin !== origin ||
      event.source !== frame!.contentWindow ||
      event.data?.type !== "frameReady"
    )
      return;
    page?.close();
    const pages = new MessageChannel();
    const assets = hosted && !join.ended ? undefined : new MessageChannel();
    page = pages.port1;
    // The frame is the page's, so its messages are checked, not trusted.
    page.onmessage = ({ data }) => {
      if (requests.failure) return;
      if (data.type === "page" && typeof data.id === "string" && typeof data.request === "string")
        send({ type: "page", id: data.id, request: data.request });
      if (data.type === "hostReply") {
        if (data.error) requests.settle(data.id, undefined, new Error(data.error));
        else requests.settle(data.id, data);
      }
    };
    if (assets)
      assets.port1.onmessage = ({ data }) => {
        if (
          !["resource", "resourcePlan"].includes(data.type) ||
          !["app", "attachment"].includes(data.route) ||
          typeof data.key !== "string"
        )
          return;
        void relayResource(assets!.port1, data);
      };
    frame!.contentWindow!.postMessage(
      { type: "connect" },
      origin,
      assets ? [pages.port2, assets.port2] : [pages.port2],
    );
    // A frame that connects after the worker failed learns it at once.
    tellFrame();
  };
  addEventListener("message", connect);
  surface = document.createElement("div");
  surface.style.cssText = "flex:none;overflow:hidden;transform-origin:center;position:relative";
  frame.style.cssText = "width:100%;height:100%;display:block";
  surface.append(frame);
  stage.replaceChildren(surface);
  const windowFrame = spec.frame;
  if (windowFrame.kind === "skin") {
    const target = surface;
    void (async () => {
      const key = windowFrame.skin;
      const info = await request({ type: "resource", route: "app", key, offset: 0, length: 0 });
      const chunks: Uint8Array<ArrayBuffer>[] = [];
      for (let offset = 0; offset < info.size; offset += BrowserHost.chunkBytes) {
        const result = await request({
          type: "resource",
          route: "app",
          key,
          offset,
          length: Math.min(BrowserHost.chunkBytes, info.size - offset),
        });
        if (!result.bytes) throw new Error("Incomplete skin transfer");
        chunks.push(result.bytes);
      }
      const url = URL.createObjectURL(new Blob(chunks, { type: info.media_type }));
      target.style.background = `url("${url}") 0 0/100% 100% no-repeat`;
      target.style.mask = `url("${url}") 0 0/100% 100% no-repeat`;
    })().catch(fail);
  } else {
    const { background, shape, radius } = windowFrame.frame;
    surface.style.background =
      background === "transparent"
        ? "transparent"
        : background === "glass"
          ? "#ffffff55"
          : "Canvas";
    if (background === "glass") surface.style.backdropFilter = "blur(24px) saturate(1.8)";
    if (shape.kind === "radii") surface.style.clipPath = `inset(0 round ${radius})`;
    else {
      stage.insertAdjacentHTML("beforeend", silhouetteClip(shape));
      surface.style.clipPath = "url(#window-shape)";
    }
  }
  new ResizeObserver(fit).observe(stage);
  fit();
}

async function own() {
  if (!id || !/^[0-9a-f-]{36}$/.test(id)) throw new Error("Invalid browser copy ID");
  await navigator.locks.request(lockName(id), { ifAvailable: true }, async (lock) => {
    if (!lock)
      throw new Error("This copy is open in another tab. Close that tab, then reload here.");
    const entries = await listCopies();
    const existing = entries.find((entry) => entry.id === id);
    if (source) {
      if (existing) throw new Error("This import already exists. Open it from Browser copies.");
      if (!hosted) {
        const estimate = await navigator.storage.estimate();
        const head = await fetch(`/__import/${encodeURIComponent(source)}`, { method: "HEAD" });
        if (!head.ok) throw new Error("Import expired. Run tiny open --browser again.");
        const bytes = Number(head.headers.get("Content-Length"));
        if (estimate.quota !== undefined && estimate.quota - (estimate.usage ?? 0) < bytes * 2)
          throw new Error("There is not enough browser storage for this copy");
      }
      await putCopy({ id: id!, title: "Importing…", created: Date.now(), state: "importing" });
    } else if (!existing || existing.state !== "ready")
      throw new Error("This browser copy is missing. Open the original file again.");
    // Keep the Web Lock until the worker dies and this tab releases its handles.
    await new Promise<void>((_, reject) => {
      const launch = (attempt: number) => {
        worker = new Worker("/browser/worker.js", { type: "module" });
        let opened = false;
        // The worker is gone, or can no longer be trusted: every request waiting on it is lost.
        worker.onerror = () =>
          stop(
            new Error("The document worker stopped. Reload to recover its last confirmed save."),
          );
        worker.onmessageerror = () =>
          stop(
            new Error(
              "The document worker sent a message this tab could not read. Reload to recover its last confirmed save.",
            ),
          );
        worker.onmessage = async ({ data }: MessageEvent<WorkerEvent | SharingEvent>) => {
          if (requests.failure) return;
          if (data.type === "resourcesReady") {
            join.handle(data);
            return;
          }
          if (data.type === "files") {
            document.querySelector("#files")!.textContent = data.status;
            return;
          }
          if (data.type === "sync") {
            document.querySelector("#sync")!.textContent = data.status;
            join.handle(data);
            return;
          }
          if (data.type === "ready") {
            opened = true;
            title.textContent = data.title;
            try {
              await putCopy({
                id: id!,
                title: data.title,
                created: existing?.created ?? Date.now(),
                state: "ready",
              });
            } catch (error) {
              worker?.terminate();
              reject(error);
              return;
            }
            // The worker may have failed while the catalog was written.
            if (requests.failure) return;
            history.replaceState(null, "", `/?copy=${id}`);
            download.disabled = false;
            if (hosted) await join.resources;
            if (hosted && source) await join.catchup; // Initial join catches up before authored startup effects run.
            if (requests.failure) return;
            if (join.stopped !== undefined) {
              // The copy is intact and saved here; it just cannot open until sharing is dealt with.
              fail(
                `Sharing stopped: ${join.stopped.replace(/[.…]+$/, "")}. This copy is saved in this browser: download it, or go back to Browser copies.`,
              );
              return;
            }
            mount(data.window, data.capabilities);
            download.disabled = false;
          } else if (data.type === "save") {
            status.textContent =
              data.error || (data.status === "saved" ? "Saved in this browser" : "Saving…");
            retry.hidden = data.status !== "failed";
          } else if (data.type === "publication") toFrame(data);
          else if (data.type === "error" && !data.id) {
            if (!opened) {
              worker?.terminate();
              if (data.error.startsWith("Browser storage is busy:") && attempt < 4) {
                setTimeout(() => launch(attempt + 1), 50);
                return;
              }
              reject(new Error(data.error));
            } else fail(data.error);
          } else if ("id" in data && data.id) {
            const settled =
              data.type === "error"
                ? requests.settle(data.id, undefined, new Error(data.error))
                : requests.settle(data.id, data);
            // Not this tab's request: a page request's reply or refusal, matched by the frame.
            if (!settled && data.type === "reply")
              toFrame({ type: "reply", id: data.id, json: data.json });
            if (!settled && data.type === "error")
              toFrame({ type: "error", id: data.id, error: data.error });
          }
        };
        if (hosted)
          worker!.postMessage({
            type: "joinCredential",
            credential: localStorage.getItem("sharing.invitation." + id) ?? "",
          } satisfies SharingControl);
        send({ type: "open", copy: id!, source });
      };
      launch(0);
    });
  });
}
const copiesLink = document.querySelector<HTMLAnchorElement>("header a")!;
copiesLink.onclick = async (event) => {
  if (
    !page ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  )
    return;
  event.preventDefault();
  try {
    await drain();
    location.assign(copiesLink.href);
  } catch (error) {
    fail(error);
  }
};
download.onclick = async () => {
  download.disabled = true;
  let staged: { folder: FileSystemDirectoryHandle; name: string } | undefined;
  let writable: FileSystemWritableFileStream | undefined;
  try {
    if (!("showSaveFilePicker" in window))
      throw new Error("Saving a .tiny needs desktop Chrome's Save As support");
    // The picker must run during the click's user activation, before any async drain.
    const destination = await (window as any).showSaveFilePicker({
      suggestedName: `${title.textContent || "Document"}.tiny`,
      types: [{ description: "tinys document", accept: { "application/octet-stream": [".tiny"] } }],
    });
    if (page) await drain();
    const result = await request({ type: "export" });
    const root = await navigator.storage.getDirectory();
    const exports = await root.getDirectoryHandle("exports");
    const folder = await exports.getDirectoryHandle(id!);
    staged = { folder, name: result.file };
    const file = await (await folder.getFileHandle(result.file)).getFile();
    writable = await destination.createWritable();
    await file.stream().pipeTo(writable!);
    writable = undefined;
  } catch (error) {
    await writable?.abort().catch(() => {});
    if ((error as DOMException).name !== "AbortError") fail(error);
  } finally {
    await staged?.folder.removeEntry(staged.name).catch(fail);
    download.disabled = requests.failure !== undefined;
  }
};
retry.onclick = () => {
  void request({ type: "flush" })
    .then((reply) => {
      const outcome = JSON.parse(reply.json) as { ok: true } | PageFailure;
      if (!outcome.ok) throw new Error(outcome.error);
    })
    .catch(fail);
};
fullscreen.onclick = () => {
  void (document.fullscreenElement ? document.exitFullscreen() : stage.requestFullscreen()).catch(
    fail,
  );
};
let keepAwake = false;
let wake: WakeLockSentinel | undefined;
let wakeGeneration = 0;
let acquiring: Promise<void> | undefined;
function renderWake() {
  awake.textContent = !keepAwake
    ? "Keep awake: off"
    : wake && !wake.released
      ? "Keep awake: on"
      : "Keep awake: paused";
}
async function updateWake() {
  const generation = ++wakeGeneration;
  if (!keepAwake || document.visibilityState !== "visible") {
    const old = wake;
    wake = undefined;
    renderWake();
    await old?.release();
    return;
  }
  if (wake && !wake.released) {
    renderWake();
    return;
  }
  if (acquiring) {
    await acquiring;
    if (generation === wakeGeneration) await updateWake();
    return;
  }
  if (!navigator.wakeLock) throw new Error("Keep awake is unavailable in this browser");
  renderWake();
  acquiring = (async () => {
    const sentinel = await navigator.wakeLock.request("screen");
    if (generation !== wakeGeneration || !keepAwake || document.visibilityState !== "visible") {
      await sentinel.release();
      return;
    }
    wake = sentinel;
    sentinel.addEventListener("release", () => {
      if (wake === sentinel) wake = undefined;
      renderWake();
    });
    renderWake();
  })();
  try {
    await acquiring;
  } finally {
    acquiring = undefined;
    renderWake();
  }
}
awake.onclick = () => {
  keepAwake = !keepAwake;
  renderWake();
  void updateWake().catch((error) => {
    renderWake();
    fail(error);
  });
};
document.addEventListener("visibilitychange", () => {
  void updateWake().catch((error) => {
    renderWake();
    fail(error);
  });
  if (document.hidden && page) void drain().catch(() => {});
});
addEventListener("pagehide", () => {
  if (page) void drain().catch(() => {});
});

async function main() {
  if (!navigator.storage?.getDirectory || !navigator.locks || (!hosted && !navigator.serviceWorker))
    throw new Error("Browser editing needs local file storage and Web Locks. Use desktop Chrome.");
  await cleanup();
  const fragment = location.hash;
  if (hosted && location.pathname === "/join") {
    if (!/^#[0-9a-f]{32}\.[0-9a-f]{64}$/.test(fragment))
      throw new Error("Sharing link is no longer valid");
    const invitation = config.host + "/join" + fragment;
    const existing = (await listCopies()).find(
      (copy) =>
        copy.state === "ready" &&
        localStorage.getItem("sharing.invitation." + copy.id) === invitation,
    );
    id = existing?.id ?? crypto.randomUUID();
    source = existing ? null : "0".repeat(64);
    localStorage.setItem("sharing.invitation." + id, invitation);
    history.replaceState(null, "", `/?copy=${id}${source ? "&fresh=1" : ""}`);
  }
  if (id) return own();
  document.querySelector("#actions")!.remove();
  stage.classList.add("copies");
  title.textContent = "Browser copies";
  status.textContent = hosted
    ? "Live collaboration · copies save in this browser"
    : "Open a file with tiny open --browser file.tiny";
  const form = document.createElement("form");
  const input = document.createElement("input");
  input.type = "url";
  input.required = true;
  input.placeholder = "Paste a sharing link";
  input.setAttribute("aria-label", "Sharing link");
  const join = document.createElement("button");
  join.type = "submit";
  join.textContent = "Open shared tiny";
  form.append(input, join);
  form.onsubmit = (event) => {
    event.preventDefault();
    try {
      const link = new URL(input.value);
      if (
        link.href !== `${config.host}/join${link.hash}` ||
        !/^#[0-9a-f]{32}\.[0-9a-f]{64}$/.test(link.hash)
      )
        throw new Error("Paste a tinys sharing link");
      location.assign(link.href);
    } catch (error) {
      fail(error);
    }
  };
  if (hosted) stage.append(form);
  for (const entry of await listCopies()) {
    const row = document.createElement("div");
    const link = document.createElement("a");
    link.href = `/?copy=${entry.id}`;
    link.textContent = entry.title;
    const remove = document.createElement("button");
    remove.textContent = "Delete";
    remove.onclick = () => {
      void removeCopy(entry.id)
        .then(() => {
          localStorage.removeItem("sharing.invitation." + entry.id);
          row.remove();
        })
        .catch(fail);
    };
    const details = document.createElement("small");
    details.textContent =
      entry.state === "missing"
        ? "Browser data is missing"
        : new Date(entry.created).toLocaleString();
    row.append(link, details, remove);
    stage.append(row);
  }
}
void main().catch(fail);

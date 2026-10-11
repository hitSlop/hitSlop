import { BrowserHost } from "../schema/constants";
import type { BrowserEvent, BrowserRequest } from "../wire/browser.generated";
import type { Capability, WindowDefinition } from "../wire/app.generated";
import { cleanup, listCopies, lockName, putCopy, removeCopy } from "./copies";
import { silhouettePath } from "../schema/presentation";
import { capabilityFeatures } from "../schema/policy";
import { PendingRequests } from "./requests";

const status = document.querySelector<HTMLElement>("#status")!;
const title = document.querySelector<HTMLElement>("#title")!;
const stage = document.querySelector<HTMLElement>("#stage")!;
const download = document.querySelector<HTMLButtonElement>("#download")!;
const retry = document.querySelector<HTMLButtonElement>("#retry")!;
const fullscreen = document.querySelector<HTMLButtonElement>("#fullscreen")!;
const awake = document.querySelector<HTMLButtonElement>("#awake")!;
const params = new URLSearchParams(location.search);
const id = params.get("copy");
const source = new URLSearchParams(location.hash.slice(1)).get("import");
const fail = (error: unknown) => {
  status.textContent = String(error);
  status.setAttribute("role", "alert");
};
let worker: Worker | undefined;
let page: MessagePort | undefined;
let frame: HTMLIFrameElement | undefined;
let surface: HTMLDivElement | undefined;
let windowSpec: WindowDefinition | undefined;
const requests = new PendingRequests();
const send = (message: BrowserRequest) => worker!.postMessage(message);
type WithoutId<T> = T extends { id: string } ? Omit<T, "id"> : never;
function request(message: WithoutId<BrowserRequest>): Promise<any> {
  return requests.start((id) => send({ ...message, id } as BrowserRequest), {
    after: 60_000,
    message: "The copy did not respond; keep this tab open and retry",
  });
}
async function drain() {
  if (!page) throw new Error("Document is still opening");
  await requests.start(
    (id) => page!.postMessage({ type: "host", id, request: { method: "flush" } }),
    {
      after: 30_000,
      message: "The app did not finish saving",
    },
  );
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
  frame.title = title.textContent || "Slop";
  const origin = `http://${id}.localhost:${BrowserHost.port}`;
  frame.src = `${origin}/`;
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
    const assets = new MessageChannel();
    page = pages.port1;
    page.onmessage = ({ data }) => {
      if (data.type === "page" && typeof data.id === "string" && typeof data.request === "string")
        send({ type: "page", id: data.id, request: data.request });
      if (data.type === "hostReply") {
        requests.settle(data.id, data, data.error ? new Error(data.error) : undefined);
      }
    };
    assets.port1.onmessage = ({ data }) => {
      if (
        !["resource", "resourcePlan"].includes(data.type) ||
        !["app", "attachment"].includes(data.route) ||
        typeof data.key !== "string"
      )
        return;
      void requests
        .start((id) =>
          send(
            data.type === "resourcePlan"
              ? { type: "resourcePlan", id, route: data.route, key: data.key, range: data.range }
              : {
                  type: "resource",
                  id,
                  route: data.route,
                  key: data.key,
                  offset: data.offset,
                  length: data.length,
                },
          ),
        )
        .then(
          (result) =>
            assets.port1.postMessage(
              { ...result, id: data.id },
              result.bytes ? [result.bytes.buffer] : [],
            ),
          (error) => assets.port1.postMessage({ type: "error", id: data.id, error: error.message }),
        );
    };
    frame!.contentWindow!.postMessage({ type: "connect" }, origin, [pages.port2, assets.port2]);
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
      const ns = "http://www.w3.org/2000/svg";
      const svg = document.createElementNS(ns, "svg"),
        clip = document.createElementNS(ns, "clipPath"),
        path = document.createElementNS(ns, "path");
      svg.setAttribute("width", "0");
      svg.setAttribute("height", "0");
      svg.style.position = "absolute";
      clip.id = "window-shape";
      clip.setAttribute("clipPathUnits", "objectBoundingBox");
      const { viewBoxWidth: w, viewBoxHeight: h } = shape;
      path.setAttribute("d", silhouettePath(shape.segments));
      path.setAttribute("transform", `scale(${1 / w},${1 / h})`);
      path.setAttribute("clip-rule", shape.evenOdd ? "evenodd" : "nonzero");
      clip.append(path);
      svg.append(clip);
      stage.append(svg);
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
      const estimate = await navigator.storage.estimate();
      const head = await fetch(`/__import/${encodeURIComponent(source)}`, { method: "HEAD" });
      if (!head.ok) throw new Error("Import expired. Run slop open --browser again.");
      const bytes = Number(head.headers.get("Content-Length"));
      if (estimate.quota !== undefined && estimate.quota - (estimate.usage ?? 0) < bytes * 2)
        throw new Error("There is not enough browser storage for this copy");
      await putCopy({ id, title: "Importing…", created: Date.now(), state: "importing" });
    } else if (!existing || existing.state !== "ready")
      throw new Error("This browser copy is missing. Open the original file again.");
    // Keep the Web Lock until the worker dies and this tab releases its handles.
    await new Promise<void>((_, reject) => {
      const launch = (attempt: number) => {
        worker = new Worker("/browser/worker.js", { type: "module" });
        let opened = false;
        worker.onerror = () => {
          const error = new Error(
            "The document worker stopped. Reload to recover its last confirmed save.",
          );
          requests.fail(error);
          download.disabled = true;
          fail(error.message);
        };
        worker.onmessage = async ({
          data,
        }: MessageEvent<BrowserEvent & { bytes?: Uint8Array }>) => {
          if (data.type === "ready") {
            opened = true;
            title.textContent = data.title;
            try {
              await putCopy({
                id,
                title: data.title,
                created: existing?.created ?? Date.now(),
                state: "ready",
              });
            } catch (error) {
              worker?.terminate();
              reject(error);
              return;
            }
            history.replaceState(null, "", `/?copy=${id}`);
            mount(data.window, data.capabilities);
            download.disabled = false;
          } else if (data.type === "save") {
            status.textContent =
              data.error || (data.status === "saved" ? "Saved in this browser" : "Saving…");
            retry.hidden = data.status !== "failed";
          } else if (data.type === "publication") page?.postMessage(data);
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
            if (
              !requests.settle(
                data.id,
                data,
                data.type === "error" ? new Error(data.error) : undefined,
              )
            )
              page?.postMessage(data);
          }
        };
        send({ type: "open", copy: id, source });
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
      throw new Error("Saving a .slop needs desktop Chrome's Save As support");
    // The picker must run during the click's user activation, before any async drain.
    const destination = await (window as any).showSaveFilePicker({
      suggestedName: `${title.textContent || "Document"}.slop`,
      types: [
        { description: "hitSlop document", accept: { "application/octet-stream": [".slop"] } },
      ],
    });
    await drain();
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
    download.disabled = false;
  }
};
retry.onclick = () => {
  void request({ type: "flush" })
    .then((data) => {
      if (!JSON.parse(data.json).ok) throw new Error(JSON.parse(data.json).error);
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
  if (!navigator.storage?.getDirectory || !navigator.locks || !navigator.serviceWorker)
    throw new Error(
      "Browser editing needs OPFS, Web Locks and Service Workers. Use desktop Chrome.",
    );
  await cleanup();
  if (id) return own();
  document.querySelector("#actions")!.remove();
  stage.classList.add("copies");
  title.textContent = "Browser copies";
  status.textContent = "Open a file with slop open --browser file.slop";
  for (const entry of await listCopies()) {
    const row = document.createElement("div");
    const link = document.createElement("a");
    link.href = `/?copy=${entry.id}`;
    link.textContent = entry.title;
    const remove = document.createElement("button");
    remove.textContent = "Delete";
    remove.onclick = () => {
      void removeCopy(entry.id)
        .then(() => row.remove())
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
